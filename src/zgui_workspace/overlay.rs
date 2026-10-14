use super::DockState;
use crate::workspace::layout::DropZone;
use zgui::prelude::*;

/// Edge/center docking feedback while tab drag is active.
#[component]
pub fn DockOverlay(id: u64, dock: DockState) -> impl IntoView {
    view! {
        if move || dock.drag.get().is_some_and(|drag| drag.3) {
            column(class = "dock-drop-overlay", class:hidden = move || !dock.layout.get().is_visible(id)) {
                text(class = "dock-drop-zone", class:dock-zone-left = true, class:dock-zone-active = move || dock.group_target.get().is_none() && dock.target.get() == Some((id, DropZone::Left))) { "Left" }
                text(class = "dock-drop-zone", class:dock-zone-right = true, class:dock-zone-active = move || dock.group_target.get().is_none() && dock.target.get() == Some((id, DropZone::Right))) { "Right" }
                text(class = "dock-drop-zone", class:dock-zone-top = true, class:dock-zone-active = move || dock.group_target.get().is_none() && dock.target.get() == Some((id, DropZone::Top))) { "Top" }
                text(class = "dock-drop-zone", class:dock-zone-bottom = true, class:dock-zone-active = move || dock.group_target.get().is_none() && dock.target.get() == Some((id, DropZone::Bottom))) { "Bottom" }
                text(class = "dock-drop-zone", class:dock-zone-center = true, class:dock-zone-active = move || dock.group_target.get().is_none() && dock.target.get() == Some((id, DropZone::Center))) { "Center \u{b7} tab group" }
            }
        }

    }
}
