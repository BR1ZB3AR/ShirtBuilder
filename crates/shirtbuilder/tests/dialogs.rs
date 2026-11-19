//! The changelog dialog must always fit the window (title and close button on screen, the list
//! scrolling inside), and close with its button, Esc, or a click outside. Earlier it grew with
//! every release until its title bar sat above the window and it could not be closed.

use egui::{pos2, vec2, Event, Key, Modifiers, MouseWheelUnit, PointerButton, TouchPhase};
use egui_kittest::kittest::{NodeT, Queryable as _};
use egui_kittest::Harness;
use shirtbuilder::ShirtBuilderApp;

const CLOSE: &str = "Close (Esc)";

fn open_changelog(h: &mut Harness<'_, ShirtBuilderApp>) {
    h.get_by_label_contains("changelog").click();
    h.run();
    assert!(h.query_by_label(CLOSE).is_some(), "changelog dialog did not open");
}

fn is_open(h: &Harness<'_, ShirtBuilderApp>) -> bool {
    h.query_by_label(CLOSE).is_some()
}

fn bounds(h: &Harness<'_, ShirtBuilderApp>, label: &str) -> egui::Rect {
    let b = h.get_by_label(label).accesskit_node().bounding_box().expect("bounding box");
    egui::Rect::from_min_max(pos2(b.x0 as f32, b.y0 as f32), pos2(b.x1 as f32, b.y1 as f32))
}

#[test]
fn changelog_fits_scrolls_and_closes() {
    for size in [[1000.0, 600.0], [1440.0, 900.0]] {
        let screen = egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(size[0], size[1]));
        let mut h = Harness::builder().with_size(size).build_eframe(|cc| ShirtBuilderApp::new(cc));
        h.run();
        open_changelog(&mut h);

        // title and close button are on screen
        for label in [CLOSE, "What's new"] {
            let r = bounds(&h, label);
            assert!(screen.contains_rect(r), "{size:?}: {label} at {r:?} is off screen");
        }

        // the oldest release starts out of view and comes into view by scrolling the list
        let oldest = "Rust edition — first release";
        let before = bounds(&h, oldest);
        assert!(before.top() > size[1], "{size:?}: expected the oldest release below the fold, at {before:?}");
        let close = bounds(&h, CLOSE);
        h.event(Event::PointerMoved(pos2(close.center().x - 200.0, size[1] / 2.0)));
        for _ in 0..30 {
            h.event(Event::MouseWheel { unit: MouseWheelUnit::Point, delta: vec2(0.0, -400.0), phase: TouchPhase::Move, source: egui::MouseWheelSource::Wheel, modifiers: Modifiers::NONE });
            h.step();
        }
        h.run();
        let after = bounds(&h, oldest);
        assert!(after.bottom() <= size[1] && after.top() >= 0.0, "{size:?}: oldest release not reachable by scrolling, at {after:?}");
        assert!(screen.contains_rect(bounds(&h, CLOSE)), "{size:?}: close button scrolled away");

        // Esc closes
        h.key_press(Key::Escape);
        h.run();
        assert!(!is_open(&h), "{size:?}: Esc did not close the dialog");

        // the close button closes
        open_changelog(&mut h);
        h.get_by_label(CLOSE).click();
        h.run();
        assert!(!is_open(&h), "{size:?}: close button did not close the dialog");

        // a click outside closes
        open_changelog(&mut h);
        let outside = pos2(8.0, size[1] - 8.0);
        h.event(Event::PointerMoved(outside));
        h.event(Event::PointerButton { pos: outside, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
        h.step();
        h.event(Event::PointerButton { pos: outside, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
        h.run();
        assert!(!is_open(&h), "{size:?}: clicking outside did not close the dialog");
    }
}

#[test]
fn shortcuts_dialog_fits_a_short_window() {
    let size = [1440.0, 600.0]; // as short as the window can be
    let screen = egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(size[0], size[1]));
    let mut h = Harness::builder().with_size(size).build_eframe(|cc| ShirtBuilderApp::new(cc));
    h.run_steps(10); // the 3D view may still be easing its camera
    h.get_by_label_contains("Shortcuts & help").click();
    h.run_steps(10);
    for label in [CLOSE, "Shortcuts"] {
        let r = bounds(&h, label);
        assert!(screen.contains_rect(r), "{label} at {r:?} is off screen");
    }
}
