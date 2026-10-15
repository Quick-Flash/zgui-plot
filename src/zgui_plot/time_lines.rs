//! Background viewport preparation shared by all native time plots.
use crate::{
    PlotGeometry,
    line_draw::{Connection, LineDraw, prepare},
    plot_data::MarkerShape,
    series_samples::{Accuracy, pan_viewport},
    time_series::{SeriesData, TimeSeriesSet},
    zgui_plot::PlotState,
    zgui_renderer::*,
};
use std::collections::BTreeMap;
use zgui::{
    canvas::{Lod, Marker, zgui_color::Color},
    prelude::*,
    reactive::RenderEffect,
};
type Prepared = Vec<(String, SeriesData, SeriesData, Connection)>;
type Plan = (Vec<(String, usize, Connection)>, u64, u64, usize, Lod);
fn compatible(completed: &Plan, latest: &Plan) -> bool {
    completed.0 == latest.0
}

pub(super) fn time_lines(
    series: Signal<TimeSeriesSet, LocalStorage>,
    state: PlotState,
    drawing: Signal<BTreeMap<String, LineDraw>, LocalStorage>,
    geometry: Memo<Option<PlotGeometry>>,
    visible: Signal<bool, LocalStorage>,
    handle: CanvasHandle,
) {
    let prepared = RwSignal::new_local(Prepared::new());
    let generation = RwSignal::new_local(0_u64);
    let running = RwSignal::new_local(false);
    let cached_plan = StoredValue::new_local(None::<Plan>);
    let plan = Memo::new(move |_| {
        if !visible.get() {
            return None;
        }
        let geometry = geometry.get()?;
        let range = state.ranges.get().x;
        let hidden = state.hidden.get();
        let emphasized = state.emphasized.get();
        let drawing = drawing.get();
        let keys = series.with(|set| {
            set.series()
                .iter()
                .filter(|line| {
                    !hidden.contains(line.id()) || emphasized.as_deref() == Some(line.id())
                })
                .map(|line| {
                    (
                        line.id().to_owned(),
                        line.data().samples().as_ptr() as usize,
                        drawing
                            .get(line.id())
                            .copied()
                            .unwrap_or_default()
                            .connection,
                    )
                })
                .collect::<Vec<_>>()
        });
        Some((
            keys,
            range.min.to_bits(),
            range.max.to_bits(),
            geometry.data_width().ceil().clamp(1.0, 8192.0) as usize,
            state.lod.get(),
        ))
    });
    let prepare_effect = RenderEffect::new(move |_| {
        let request = plan.get();
        generation.update(|value| *value += 1);
        if request.is_none() {
            prepared.set(vec![]);
        }
        if running.get_untracked() {
            return;
        }
        running.set(true);
        spawn_local(async move {
            loop {
                let Some(version) = generation.try_get_untracked() else {
                    return;
                };
                let Some(request) = plan.get_untracked() else {
                    prepared.set(vec![]);
                    running.set(false);
                    return;
                };
                let (keys, min, max, columns, lod) = request.clone();
                let old = cached_plan.get_value();
                let cached = prepared.get_untracked();
                let jobs = series.with_untracked(|set| {
                    keys.iter()
                        .filter_map(|(id, pointer, mode)| {
                            let reuse = old
                                .as_ref()
                                .is_some_and(|old| {
                                    old.1 == min
                                        && old.2 == max
                                        && old.3 == columns
                                        && old.4 == lod
                                        && old.0.contains(&(id.clone(), *pointer, *mode))
                                })
                                .then(|| cached.iter().find(|line| &line.0 == id).cloned())
                                .flatten();
                            set.series()
                                .iter()
                                .find(|line| line.id() == id)
                                .map(|line| (id.clone(), line.data().clone(), *mode, reuse))
                        })
                        .collect::<Vec<_>>()
                });
                let x = crate::Range::new(f64::from_bits(min), f64::from_bits(max));
                let data = blocking(move || {
                    jobs.into_iter()
                        .map(|(id, data, mode, reuse)| {
                            if let Some(reuse) = reuse {
                                return reuse;
                            }
                            let samples = pan_viewport(
                                &data,
                                x,
                                columns,
                                if lod == Lod::Exact {
                                    Accuracy::Exact
                                } else {
                                    Accuracy::Columns
                                },
                            );
                            let steps = prepare(&samples, mode);
                            (id, samples, steps, mode)
                        })
                        .collect()
                })
                .await;
                let Some(current) = generation.try_get_untracked() else {
                    return;
                };
                if let Some(latest) = plan.get_untracked()
                    && compatible(&request, &latest)
                {
                    cached_plan.set_value(Some(request));
                    prepared.set(data);
                }
                if current == version {
                    running.set(false);
                    return;
                }
            }
        });
    });
    on_cleanup_local(move || drop(prepare_effect));
    let draw = handle.clone();
    redraw_on_change(move || {
        let mut data = if visible.get() {
            prepared.get()
        } else {
            vec![]
        };
        let hidden = state.hidden.get();
        let present = series.with(|set| {
            set.series()
                .iter()
                .map(|line| line.id().to_owned())
                .collect::<Vec<_>>()
        });
        let settings = drawing.get();
        let emphasized = state.emphasized.get();
        // Paint hovered series last, including temporary previews of hidden lines.
        data.sort_by_key(|(id, ..)| emphasized.as_deref() == Some(&id.to_string()));
        let lod = state.lod.get();
        draw.draw(|scene| {
            scene.clear();
            for (id, original, line, _) in &data {
                let key = id.to_string();
                if (hidden.contains(&key) && emphasized.as_deref() != Some(&key))
                    || !present.contains(id)
                {
                    continue;
                }
                let mut style = state.style(&key);
                let alpha = if emphasized.as_ref().is_some_and(|hover| hover != &key) {
                    0.3
                } else {
                    1.0
                };
                if emphasized.as_deref() == Some(&key) {
                    style.width += 1.5;
                }
                let [r, g, b, a] = style.color;
                let color = Color::srgb_u8(r, g, b, (f64::from(a) * alpha) as u8);
                push_prepared_line(
                    scene,
                    line,
                    color,
                    style.width.max(0.5),
                    lod,
                    kurbo::Affine::IDENTITY,
                );
                if settings.get(id).copied().unwrap_or_default().markers
                    || original.samples().len() == 1
                {
                    let radius = style.radius.clamp(0.5, 20.0);
                    let marker = match style.marker {
                        MarkerShape::Circle => Marker::Circle { radius },
                        MarkerShape::Square => Marker::Square { half: radius },
                        MarkerShape::Triangle => {
                            Marker::Path(std::sync::Arc::new(crate::triangle_marker(radius)))
                        }
                        MarkerShape::Diamond => {
                            Marker::Path(std::sync::Arc::new(crate::diamond_marker(radius)))
                        }
                        MarkerShape::Cross => Marker::Path(std::sync::Arc::new(
                            crate::cross_marker(radius, radius / 3.0),
                        )),
                    };
                    let origin = original.origin();
                    push_points_transformed(
                        scene,
                        original.vertices(),
                        marker,
                        color,
                        kurbo::Affine::translate((origin.x, origin.y)),
                    );
                }
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn continuous_pan_keeps_completed_work_but_source_changes_reject_it() {
        let completed = (
            vec![("sample".into(), 123, Connection::Pre)],
            0,
            100,
            800,
            Lod::Columns,
        );
        let mut latest = completed.clone();
        latest.1 = 10;
        latest.2 = 110;
        latest.3 = 1200;
        assert!(compatible(&completed, &latest));
        latest.0[0].1 = 456;
        assert!(!compatible(&completed, &latest));
        latest.0[0].1 = 123;
        latest.0[0].2 = Connection::Post;
        assert!(!compatible(&completed, &latest));
        latest.0.clear();
        assert!(!compatible(&completed, &latest));
    }
}
