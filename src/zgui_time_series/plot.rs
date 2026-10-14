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
    #[prop(attrs)] attrs: Attrs,
) -> impl IntoView {
    let data = Signal::derive_local(move || PlotData::Time(series.get()));
    view! { DataPlot(series = data, state = state, title = title, visible = visible, draw_series = draw_series, format = format, snapshot = snapshot, {..attrs}) }
}
