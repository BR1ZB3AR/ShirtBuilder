//! Dragging artwork on the sewing pattern with a real mouse gesture: the body moves it, a corner
//! scales it, the knob rotates it. The press point decides what is dragged — egui reports a drag
//! only after the pointer has travelled a few pixels, by when it may have left a small handle.

use egui::{vec2, Event, Modifiers, PointerButton, Pos2};
use egui_kittest::kittest::Queryable as _;
use egui_kittest::Harness;
use shirtbuilder::ShirtBuilderApp;

fn app() -> Harness<'static, ShirtBuilderApp> {
    let mut h = Harness::builder().with_size([1440.0, 900.0]).build_eframe(|cc| ShirtBuilderApp::new(cc));
    h.state_mut().set_layout("pattern");
    h.run_steps(8);
    // select the chest print from the Layers list
    h.get_by_label_contains("BUILD IT.").click();
    h.run_steps(4);
    assert!(h.state().selected_layer_for_tests().is_some(), "nothing selected");
    h
}

/// press at `from`, move to `to` in small steps like a hand does, release
fn drag(h: &mut Harness<'_, ShirtBuilderApp>, from: Pos2, to: Pos2) {
    h.event(Event::PointerMoved(from));
    h.step();
    h.event(Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.step();
    for i in 1..=12 {
        let p = from + (to - from) * (i as f32 / 12.0);
        h.event(Event::PointerMoved(p));
        h.step();
    }
    h.event(Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(4);
}

#[test]
fn dragging_the_body_moves_it() {
    let mut h = app();
    let before = h.state().selected_layer_for_tests().unwrap().clone();
    let (c, _, _) = h.state().selected_handles_for_tests().unwrap();
    drag(&mut h, c, c + vec2(60.0, 45.0));
    let after = h.state().selected_layer_for_tests().unwrap();
    assert!(after.x > before.x + 1.0 && after.y > before.y + 1.0, "didn't move: {:?} → {:?}", (before.x, before.y), (after.x, after.y));
    assert!((after.w - before.w).abs() < 1e-9, "moving changed the size");
    // it follows the pointer exactly — no jump when the drag starts
    let (c2, _, _) = h.state().selected_handles_for_tests().unwrap();
    let moved = c2 - c;
    assert!((moved - vec2(60.0, 45.0)).length() < 1.0, "moved {moved:?} for a (60, 45) px drag");
}

#[test]
fn dragging_a_corner_resizes_it() {
    let mut h = app();
    let before = h.state().selected_layer_for_tests().unwrap().clone();
    let (c, cs, _) = h.state().selected_handles_for_tests().unwrap();
    let corner = cs[2];
    let out = (corner - c).normalized();
    drag(&mut h, corner, corner + out * 40.0);
    let after = h.state().selected_layer_for_tests().unwrap();
    assert!(after.w > before.w * 1.1, "corner drag didn't enlarge: w {} → {}", before.w, after.w);
    assert!((after.w / after.h - before.w / before.h).abs() < 1e-6, "corner drag should keep the proportions");

    // and back down from the opposite corner
    let before = after.clone();
    let (c, cs, _) = h.state().selected_handles_for_tests().unwrap();
    let corner = cs[0];
    drag(&mut h, corner, corner + (c - corner) * 0.4);
    let after = h.state().selected_layer_for_tests().unwrap();
    assert!(after.w < before.w * 0.9, "corner drag didn't shrink: w {} → {}", before.w, after.w);
}

#[test]
fn dragging_the_knob_rotates_it() {
    let mut h = app();
    let before = h.state().selected_layer_for_tests().unwrap().rot;
    let (c, _, knob) = h.state().selected_handles_for_tests().unwrap();
    let r = (knob - c).length();
    drag(&mut h, knob, c + vec2(r, 0.0));
    let after = h.state().selected_layer_for_tests().unwrap().rot;
    assert!((after - before).abs() > 30.0, "knob drag didn't rotate: {before}° → {after}°");
}
