#![cfg(all(feature = "workspace", feature = "raster"))]

use std::{cell::RefCell, rc::Rc};
use zgui::{
    geom::{CssPx, DevicePx, Point, Size},
    platform::SurfaceEvent,
    prelude::*,
    render::Renderer,
    vocab::{
        KeyEvent, KeyState, Modifiers, NamedKey, PhysicalKey, PointerAction, PointerButton,
        PointerEvent, Timestamp,
    },
};
use zgui_platform_headless::Harness;
use zgui_plot::{zgui_svg::ScreenshotFormat, zgui_workspace::ScreenshotMenuProps};
use zgui_testkit_scene::CaptureRenderer;
use zgui_ui_tokens::prelude::*;

fn click(harness: &mut Harness<zgui::runtime::Runtime>, class: &str, index: usize) {
    let window = &harness.app().windows()[0];
    let node = window.nodes_with_class(class)[index];
    let bounds = zgui::view::ViewHost::window_box(window.host().as_ref(), node).unwrap();
    for action in [PointerAction::Pressed, PointerAction::Released] {
        let mut event = PointerEvent::mouse(Point::new(
            CssPx(bounds.origin.x.0 + bounds.size.width.0 / 2.),
            CssPx(bounds.origin.y.0 + bounds.size.height.0 / 2.),
        ));
        event.button = Some(PointerButton::Primary);
        harness.deliver_to_first(SurfaceEvent::Pointer {
            action,
            event,
            modifiers: Modifiers::NONE,
            timestamp: Timestamp::ORIGIN,
        });
    }
    harness.settle(128);
    harness.run_for(
        std::time::Duration::from_millis(300),
        std::time::Duration::from_millis(16),
    );
    harness.settle(128);
}

#[test]
fn camera_menus_open_without_shadow_pipeline_and_export_only_on_selection() {
    let exports = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&exports);
    let handler = app()
        .with_size(600., 400.)
        .with_renderer(Box::new(|_, target| {
            let mut renderer = CaptureRenderer::new();
            renderer.configure(target);
            Ok(Box::new(renderer))
        }))
        .with_stylesheet(
            ".test-root {width:100vw;height:100vh;} .zui-button {box-shadow:none;transition:none;}",
        )
        .into_handler(move || {
            let recorded = Rc::clone(&recorded);
            let on_export = UnsyncCallback::new(move |format| recorded.borrow_mut().push(format));
            view! {ThemeProvider(scheme=ColorScheme::Dark) {row(class="test-root") {
                ScreenshotMenu(label="Screenshot all docks",on_export=on_export)
                ScreenshotMenu(label="Screenshot plot",on_export=on_export)
            }}}
        })
        .unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(600.),
        DevicePx(400.),
    )));
    harness.settle(128);
    harness.run_for(
        std::time::Duration::from_millis(300),
        std::time::Duration::from_millis(16),
    );
    harness.settle(128);
    for (button, item, format) in [(0, 0, ScreenshotFormat::Svg), (1, 1, ScreenshotFormat::Png)] {
        let before = exports.borrow().len();
        click(&mut harness, "plot-screenshot", button);
        assert_eq!(
            exports.borrow().len(),
            before,
            "Opening camera must not capture or encode"
        );
        let window = &harness.app().windows()[0];
        assert_eq!(window.nodes_with_class("zui-menu__item").len(), 2);
        assert!(
            window.scene().primitives.shadows.is_empty(),
            "Camera menu must not trigger cold shadow pipeline"
        );
        click(&mut harness, "zui-menu__item", item);
        assert_eq!(exports.borrow().last(), Some(&format));
        assert!(
            harness.app().windows()[0]
                .nodes_with_class("zui-menu__item")
                .is_empty()
        );
    }
    click(&mut harness, "plot-screenshot", 0);
    harness.deliver_to_first(SurfaceEvent::Key {
        state: KeyState::Pressed,
        event: KeyEvent::named(NamedKey::Escape, PhysicalKey::Unidentified(0)),
        modifiers: Modifiers::NONE,
        timestamp: Timestamp::ORIGIN,
    });
    harness.settle(128);
    harness.run_for(
        std::time::Duration::from_millis(300),
        std::time::Duration::from_millis(16),
    );
    harness.settle(128);
    assert!(
        harness.app().windows()[0]
            .nodes_with_class("zui-menu__item")
            .is_empty()
    );
    assert_eq!(
        &*exports.borrow(),
        &[ScreenshotFormat::Svg, ScreenshotFormat::Png]
    );
}
