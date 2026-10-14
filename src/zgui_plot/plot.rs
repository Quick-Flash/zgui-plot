use super::render::{self, Distribution};
use super::theme::tick_text as tick_style;
use super::{PlotState, theme};
use crate::plot_data::{HistogramCurve, PlotData};
fn legend_theme() -> crate::LegendTheme {
    theme::LEGEND
}
use crate::LegendMarker;
use crate::interaction::{Axes, HitZones};
use crate::labels::{Axis, AxisLabels, TickLabel, TickLabelLayout, XLabelRow, tick_labels};
use crate::zgui_legend::{LegendItemProps, toggle_member};
use crate::zgui_pointer::PlotPointer;
use crate::zgui_renderer::{push_grid, push_selection, push_stroke, redraw_on_change, show_ranges};
use crate::zgui_surface::{PlotHover, PlotSurfaceProps};
use crate::zgui_svg::{
    PlotSnapshot, PlotSnapshotSource, register_snapshot, scenes_to_svg_pair,
    snapshot_canvas_handles,
};
use crate::{GridOptions, LabelOptions, PlotDescription, PlotGeometry, PlotPoint};
use zgui::reactive::{Callable, UnsyncCallback};
use zgui::{canvas::zgui_color::Color, elements::canvas::CanvasHandle, prelude::*};

const BACKGROUND: &str = "#12181c";
const LABELS: TickLabelLayout = TickLabelLayout {
    x_label: (64.0, 16.0),
    y_label: (56.0, 16.0),
    inset: 4.0,
    x_row: XLabelRow::Inside,
};

/// Shared time/scatter/histogram shell. Visible series share axis units.
#[allow(clippy::too_many_arguments)] // Component arguments are named props.
#[component]
pub fn DataPlot(
    series: Signal<PlotData, LocalStorage>,
    state: PlotState,
    #[prop(default = Signal::derive_local(String::new))] title: Signal<String, LocalStorage>,
    #[prop(default = Signal::derive_local(|| true))] visible: Signal<bool, LocalStorage>,
    /// Disable built-in series geometry when consumer supplies custom canvas.
    #[prop(default = Signal::derive_local(|| true))]
    draw_series: Signal<bool, LocalStorage>,
    #[prop(optional)] format: Option<UnsyncCallback<(Axis, f64), String>>,
    #[prop(optional)] snapshot: Option<PlotSnapshotSource>,
    #[prop(attrs)] attrs: Attrs,
) -> impl IntoView {
    let surface = NodeRef::new();
    let legend = NodeRef::new();
    let size = surface.observe_content_size();
    let geometry = Memo::new(move |_| {
        let size = size.get();
        let scale = f64::from(surface.scale());
        let (width, height) = (
            f64::from(size.width.0) / scale,
            f64::from(size.height.0) / scale,
        );
        let options = state.histogram.get();
        let strip =
            if matches!(series.get(), PlotData::Histogram(_)) && (options.cdf || options.ccdf) {
                48.0
            } else {
                0.0
            };
        PlotGeometry::with_bounds(0.0, 0.0, (width - strip).max(0.0), height, width, height)
    });
    let format_value = move |axis, value| {
        format.map_or_else(
            || crate::format_number(value),
            |format| format.run((axis, value)),
        )
    };
    let ticks = Memo::new(move |_| {
        geometry.get().map_or_else(Vec::new, |geometry| {
            let mut ticks = tick_labels(
                state.ranges.get(),
                geometry,
                &LABELS,
                AxisLabels {
                    options: LabelOptions::horizontal(0.0),
                    format: move |value| format_value(Axis::X, value),
                },
                AxisLabels {
                    options: LabelOptions::vertical(
                        if matches!(series.get(), PlotData::Histogram(_)) {
                            1.0
                        } else {
                            0.0
                        },
                    ),
                    format: move |value| format_value(Axis::Y, value),
                },
            );
            let options = state.histogram.get();
            if matches!(series.get(), PlotData::Histogram(_)) && (options.cdf || options.ccdf) {
                for fraction in [0.0, 0.25, 0.5, 0.75, 1.0] {
                    ticks.push(TickLabel {
                        axis: Axis::Y,
                        value: fraction,
                        text: format!("{:.0}%", fraction * 100.0),
                        opacity: 1.0,
                        left: geometry.right + 6.0,
                        top: (geometry.y(fraction, (0.0, 1.0)) - 8.0)
                            .clamp(0.0, (geometry.height - 16.0).max(0.0)),
                        width: 40.0,
                        height: 16.0,
                    });
                }
            }
            ticks
        })
    });
    let pointer = PlotPointer::new(HitZones::default(), Axes::BOTH);
    let fit_before_drag = StoredValue::new_local(state.auto_fit.get_untracked());
    let cursor = RwSignal::new_local(None::<(f64, f64)>);
    redraw_on_change(move || {
        let current = series.get();
        let hidden = state.hidden.get();
        let bins = state.histogram.get().bins;
        if state.auto_fit.get() {
            let fitted = current.fitted_ranges(&hidden, bins);
            if state.ranges.get_untracked() != fitted {
                state.ranges.set(fitted);
            }
        }
        // Clear obsolete sample identities when replacing/removing sources.
        if state.hover.get_untracked().is_some() {
            state.hover.set(None);
        }
        if state.bin_hover.get_untracked().is_some() {
            state.bin_hover.set(None);
        }
    });

    let grid = CanvasHandle::new();
    let grid_draw = grid.clone();
    redraw_on_change(move || {
        if !visible.get() {
            return;
        }
        if let Some(geometry) = geometry.get() {
            grid_draw.draw(|scene| {
                scene.clear();
                push_grid(
                    scene,
                    crate::grid_paths(
                        state.ranges.get(),
                        geometry,
                        GridOptions {
                            y_min_step: if matches!(series.get(), PlotData::Histogram(_)) {
                                1.0
                            } else {
                                0.0
                            },
                            ..GridOptions::default()
                        },
                    ),
                    Color::srgb_u8(140, 150, 170, 90),
                );
            });
        }
    });
    let distribution = Memo::new(move |_| match series.get() {
        PlotData::Histogram(set) => Distribution::new(set.moments(&state.hidden.get())),
        _ => Distribution::default(),
    });
    let annotation_handle = CanvasHandle::new();
    let annotation_draw = annotation_handle.clone();
    redraw_on_change(move || {
        if !visible.get() {
            return;
        }
        if let Some(geometry) = geometry.get() {
            annotation_draw.draw(|scene| {
                scene.clear();
                if state.show_annotations.get() {
                    super::annotations::draw(
                        scene,
                        &state.annotations.get(),
                        state.ranges.get(),
                        geometry,
                    );
                }
            });
        }
    });
    let lines = CanvasHandle::new();
    let lines_draw = lines.clone();
    redraw_on_change(move || {
        if !visible.get() || !draw_series.get() {
            lines_draw.draw(|scene| scene.clear());
            return;
        }
        let current = series.get();
        let histogram = matches!(current, PlotData::Histogram(_));
        let geometry = if histogram { geometry.get() } else { None };
        lines_draw.draw(|scene| {
            scene.clear();
            render::draw_data(scene, &current, state, geometry, &distribution.get());
        });
    });
    let lines_view = lines.clone();
    redraw_on_change(move || {
        if !visible.get() {
            return;
        }
        if matches!(series.get(), PlotData::Histogram(_)) {
            lines_view.set_transform(kurbo::Affine::IDENTITY);
            return;
        }
        if let Some(geometry) = geometry.get() {
            show_ranges(&lines_view, geometry, state.ranges.get());
        }
    });
    let overlay = CanvasHandle::new();
    let overlay_draw = overlay.clone();
    redraw_on_change(move || {
        if !visible.get() {
            return;
        }
        let Some(geometry) = geometry.get() else {
            return;
        };
        overlay_draw.draw(|scene| {
            scene.clear();
            let target = state
                .hover
                .get()
                .map(|hit| crate::overlay::HoverTarget::Point(hit.point))
                .or_else(|| cursor.get().map(crate::overlay::HoverTarget::Cursor));
            if let Some(target) = target {
                let path =
                    crate::overlay::hover_crosshair(target, state.ranges.get(), geometry, 4.0);
                push_stroke(scene, path.clone(), Color::srgb_u8(0, 0, 0, 255), 3.0);
                push_stroke(scene, path, Color::srgb_u8(244, 247, 245, 255), 1.0);
            }
            if let Some(hit) = state.bin_hover.get() {
                let bins = crate::histogram::empty_bins(
                    state.histogram.get().bins.clamp(1, 4096),
                    state.ranges.get().x,
                );
                if let Some(index) = bins.iter().position(|bin| bin.start == hit.bin.start) {
                    let mut bins = bins;
                    bins[index].count = hit.bin.count;
                    push_stroke(
                        scene,
                        crate::histogram::bar_path_in_range(
                            &bins,
                            geometry,
                            state.ranges.get().y,
                            Some(index),
                        ),
                        zgui::canvas::zgui_color::Color::srgb_u8(244, 247, 245, 255),
                        2.0,
                    );
                }
            }
            if let Some(selection) = pointer.selection() {
                push_selection(
                    scene,
                    selection,
                    Color::srgb_u8(119, 185, 232, 40),
                    Color::srgb_u8(119, 185, 232, 255),
                );
            }
        });
    });
    if let Some(source) = snapshot {
        let (grid, annotations, lines, overlay) = (
            grid.clone(),
            annotation_handle.clone(),
            lines.clone(),
            overlay.clone(),
        );
        register_snapshot(source, move || {
            let geometry = geometry.get_untracked().ok_or("Plot is not measured")?;
            let current = series.get_untracked();
            let hidden = state.hidden.get_untracked();
            let mut description = PlotDescription::new(geometry.width, geometry.height, BACKGROUND);
            description.text = crate::tick_label_text(&ticks.get_untracked(), &tick_style());
            let plot_title = title.get_untracked();
            description.push_title(&plot_title);
            let mut entries = current
                .labels()
                .iter()
                .map(|series| {
                    legend_theme().entry(
                        current.legend_marker(),
                        series.label(),
                        state.style(series.id()).css_color(),
                        !hidden.contains(series.id()),
                    )
                })
                .collect::<Vec<_>>();
            if matches!(current, PlotData::Histogram(_)) {
                let options = state.histogram.get_untracked();
                entries.extend([
                    legend_theme().entry(LegendMarker::Line, "CDF", "#ffe0a0", options.cdf),
                    legend_theme().entry(LegendMarker::Line, "CCDF", "#77b9e8", options.ccdf),
                    legend_theme().entry(
                        LegendMarker::Line,
                        "Normal fit",
                        "#ed9186",
                        options.normal_fit,
                    ),
                ]);
            }
            description.push_legend(
                entries,
                (
                    geometry.right - 6.0,
                    if plot_title.is_empty() {
                        4.0
                    } else {
                        crate::TITLE_LEGEND_TOP
                    },
                ),
                &theme::legend_box(),
            );
            let base = snapshot_canvas_handles(&[&grid, &annotations, &lines])?;
            let overlay = snapshot_canvas_handles(&[&overlay])?;
            Ok(PlotSnapshot {
                svg: scenes_to_svg_pair(
                    &base.iter().collect::<Vec<_>>(),
                    &overlay.iter().collect::<Vec<_>>(),
                    &description,
                )?,
                node: surface,
                width: geometry.width,
                height: geometry.height,
            })
        });
    }
    let on_hover = UnsyncCallback::new(move |event| {
        let PlotHover::At(position) = event else {
            cursor.set(None);
            state.hover.set(None);
            state.bin_hover.set(None);
            return;
        };
        cursor.set(Some(position));
        let Some(geometry) = geometry.get_untracked() else {
            return;
        };
        let ranges = state.ranges.get_untracked();
        let target = PlotPoint {
            x: geometry.x_value(position.0, ranges.x),
            y: geometry.y_value(position.1, ranges.y),
        };
        let current = series.get_untracked();
        let hit = render::hover(&current, state, target, geometry);
        state.hover.set(hit);
    });
    view! {
        column(class = "time-series-host", {..attrs}) {
            PlotSurface(node_ref = surface, class = "time-series-plot", label = "Interactive data plot".into(),
                pointer = pointer, geometry = UnsyncCallback::new(move |()| geometry.get()),
                ranges = UnsyncCallback::new(move |()| state.ranges.get()),
                on_update = UnsyncCallback::new(move |update| { state.apply(update); cursor.set(None); }),
                on_reset = UnsyncCallback::new(move |()| { state.fit_data(&series.get_untracked()); cursor.set(None); pointer.cancel(); }),
                on_press = UnsyncCallback::new(move |_| { fit_before_drag.set_value(state.auto_fit.get_untracked()); false }),
                on_cancel = UnsyncCallback::new(move |initial| { state.ranges.set(initial); state.auto_fit.set(fit_before_drag.get_value()); }),
                on_hover = on_hover, ticks = ticks, title = title, legend_ref = legend,
                cursor_format = UnsyncCallback::new(move |(x, y)| if matches!(series.get(), PlotData::Histogram(_)) { format!("X: {}", format_value(Axis::X, x)) } else { format!("X: {} | Y: {}", format_value(Axis::X, x), format_value(Axis::Y, y)) }),
                background = BACKGROUND.to_owned()) {
                {zgui::elements::canvas().class("time-series-layer").scene(&grid)}
                {zgui::elements::canvas().class("time-series-layer").scene(&annotation_handle)}
                {zgui::elements::canvas().class("time-series-layer").scene(&lines)}
                {zgui::elements::canvas().class("time-series-layer").scene(&overlay)}
                column(class = "plot-legend", node_ref = legend,
                    style:right = move || geometry.get().map(|geometry| format!("{}px", geometry.width - geometry.right + 6.0))) {
                    for label in move || series.get().labels(), key = |label| (label.id().to_owned(), label.label().to_owned()) {
                        LegendItem(label = label.label().to_owned(), color = state.style(label.id()).css_color(), marker = series.get_untracked().legend_marker(),
                            color_signal = Signal::derive_local({ let id = label.id().to_owned(); move || state.style(&id).css_color() }),
                            visible = Signal::derive_local({ let id = label.id().to_owned(); move || !state.hidden.with(|hidden| hidden.contains(&id)) }),
                            on_toggle = UnsyncCallback::new({ let id = label.id().to_owned(); move |()| state.hidden.update(|hidden| toggle_member(hidden, &id)) }),
                            key = label.id().to_owned(), hovered = state.emphasized)
                    }
                    if move || matches!(series.get(), PlotData::Histogram(_)) {
                        for curve in move || [HistogramCurve::Cdf, HistogramCurve::Ccdf, HistogramCurve::NormalFit], key = |curve| *curve {
                            LegendItem(label = match curve { HistogramCurve::Cdf => "CDF", HistogramCurve::Ccdf => "CCDF", HistogramCurve::NormalFit => "Normal fit" }.into(),
                                color = match curve { HistogramCurve::Cdf => "#ffe0a0", HistogramCurve::Ccdf => "#77b9e8", HistogramCurve::NormalFit => "#ed9186" }.into(),
                                marker = LegendMarker::Line,
                                visible = Signal::derive_local(move || { let options = state.histogram.get(); match curve { HistogramCurve::Cdf => options.cdf, HistogramCurve::Ccdf => options.ccdf, HistogramCurve::NormalFit => options.normal_fit } }),
                                on_toggle = UnsyncCallback::new(move |()| state.histogram.update(|options| match curve { HistogramCurve::Cdf => options.cdf = !options.cdf, HistogramCurve::Ccdf => options.ccdf = !options.ccdf, HistogramCurve::NormalFit => options.normal_fit = !options.normal_fit })),
                                key = curve, hovered = state.curve_hover)
                        }
                    }
                }
            }
        }
    }
}
