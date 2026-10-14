use crate::zgui_link::LinkOptions;
use zgui::prelude::*;
use zgui_ui::prelude::*;

/// Independent opt-in links for matching axis units.
#[component]
pub fn LinkControls(options: RwSignal<LinkOptions, LocalStorage>) -> impl IntoView {
    view! {
        text(class = "settings-label") { "Linked plots" }
            row(class = "plot-toolbar") {
                Toggle(size = ToggleSize::Sm, pressed = Binding::controlled(Signal::derive_local(move || options.get().x), move |value| options.update(|links| links.x = value))) { "Link X bounds" }
                Toggle(size = ToggleSize::Sm, pressed = Binding::controlled(Signal::derive_local(move || options.get().y), move |value| options.update(|links| links.y = value))) { "Link Y bounds" }
                Toggle(size = ToggleSize::Sm, pressed = Binding::controlled(Signal::derive_local(move || options.get().pointer), move |value| options.update(|links| links.pointer = value))) { "Link pointer" }
            }
            text(class = "settings-value") { "Enable on each participating plot. Use matching axis units." }

    }
}
