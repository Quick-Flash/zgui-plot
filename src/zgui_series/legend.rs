use super::{
    LegendHost, SeriesPane as Pane, legend_entry::LegendEntryProps, plot_editor::PlotEditorProps,
};
use zgui::prelude::*;

#[component]
pub fn PlotLegend(
    host: LegendHost,
    pane: Pane,
    node_ref: NodeRef,
    #[prop(default = Signal::derive_local(|| false))] extra_visible: Signal<bool, LocalStorage>,
    #[prop(optional)] children: Option<ChildrenFn>,
    #[prop(default = RwSignal::new_local(false))] settings_open: RwSignal<bool, LocalStorage>,
) -> impl IntoView {
    view! {
        if move || !pane.lines.get().is_empty() || extra_visible.get() {
        column(class = "workbench-legend", node_ref = node_ref) {
            row(class = "legend-heading") {
                text { "Legend" }
                PlotEditor(actions = host.actions, pane = pane, open = settings_open)
            }
            for line in move || pane.lines.get(), key = |line| (line.id, line.label()) {
                LegendEntry(host = host, pane = pane, line = line)
            }
            {children.as_ref().map(|children| children.view())}
        }
        }
    }
}
