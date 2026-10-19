use super::{
    LegendHost, SeriesPane as Pane, legend_entry::LegendEntryProps, plot_editor::PlotEditorProps,
};
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn PlotLegend(
    host: LegendHost,
    #[prop(optional)] fixed_connection: Option<crate::line_draw::Connection>,
    pane: Pane,
    node_ref: NodeRef,
    #[prop(default = Signal::derive_local(|| false))] extra_visible: Signal<bool, LocalStorage>,
    #[prop(default = false)] custom_entries: bool,
    #[prop(optional)] children: Option<ChildrenFn>,
    #[prop(default = RwSignal::new_local(false))] settings_open: RwSignal<bool, LocalStorage>,
) -> impl IntoView {
    let children = StoredValue::new_local(children);
    let collapsed = RwSignal::new_local(false);
    view! {
        if move || !pane.lines.get().is_empty() || extra_visible.get() {
        column(class = "workbench-legend", node_ref = node_ref) {
            row(class = "legend-heading") {
                Button(size = ButtonSize::Xs, variant = ButtonVariant::Ghost,
                    class = "legend-collapse",
                    {..Attrs::new().a11y_from(A11yBinding::new(Role::Button)
                        .expanded(move || !collapsed.get())
                        .label(move || if collapsed.get() { "Expand legend" } else { "Collapse legend" }))},
                    on:click = move |event| { collapsed.update(|value| *value = !*value); event.stop_propagation(); }) {
                    {move || if collapsed.get() { "▸ Legend" } else { "▾ Legend" }}
                }
                PlotEditor(fixed_connection = fixed_connection, actions = host.actions, pane = pane, open = settings_open)
            }
            if move || !collapsed.get() {
            if move || !custom_entries {
            for line in move || pane.lines.get(), key = |line| (line.id, line.label()) {
                LegendEntry(host = host, pane = pane, line = line)
            }
            }
            {children.with_value(|children| children.as_ref().map(|children| children.view()))}
            }
        }
        }
    }
}
