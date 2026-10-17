use crate::labels::Axis;
use crate::plot_data::PlotData;
use crate::time_series::TimeSeriesSet;
use crate::zgui_plot::{DataPlotProps, PlotState};
use crate::zgui_svg::PlotSnapshotSource;
use zgui::prelude::*;
use zgui::reactive::UnsyncCallback;

#[allow(clippy::too_many_arguments)] // Component arguments are named props.
#[component]
pub fn TimeSeriesPlot(
    series: Signal<TimeSeriesSet, LocalStorage>,
    state: PlotState,
    #[prop(default = Signal::derive_local(String::new))] title: Signal<String, LocalStorage>,
    #[prop(default = Signal::derive_local(|| true))] visible: Signal<bool, LocalStorage>,
    #[prop(default = Signal::derive_local(|| true))] draw_series: Signal<bool, LocalStorage>,
    #[prop(optional)] format: Option<UnsyncCallback<(Axis, f64), String>>,
    #[prop(optional)] snapshot: Option<PlotSnapshotSource>,
    /// Optional shared line editor action exposed in grid context menu.
    #[prop(optional)]
    on_line_settings: Option<UnsyncCallback<()>>,
    #[prop(default = Signal::derive_local(|| true))] line_settings_enabled: Signal<
        bool,
        LocalStorage,
    >,
    #[prop(default = Signal::stored_local(std::collections::BTreeMap::new()))] line_drawing: Signal<
        std::collections::BTreeMap<String, crate::line_draw::LineDraw>,
        LocalStorage,
    >,
    #[prop(default = true)] show_legend: bool,
    #[prop(default = true)] show_pointer: bool,
    #[prop(default = true)] show_cursor_values: bool,
    #[prop(optional)] background_layer: Option<ChildrenFn>,
    #[prop(attrs)] attrs: Attrs,
) -> impl IntoView {
    let data = Signal::derive_local(move || PlotData::Time(series.get()));
    view! { DataPlot(series = data, state = state, title = title, visible = visible, draw_series = draw_series, format = format, snapshot = snapshot, on_line_settings = on_line_settings, line_settings_enabled = line_settings_enabled, line_drawing = line_drawing, show_legend = show_legend, show_pointer = show_pointer, show_cursor_values = show_cursor_values, background_layer = move || AnyView::new(background_layer.as_ref().map(|layer| layer.view())), {..attrs}) }
}
