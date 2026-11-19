//! The top bar at every window width the app allows: its buttons never overlap or leave the bar
//! (they used to slide over each other below ~1360 px, so clicks hit the wrong button), and it
//! only gets as compact as the width needs.

use egui::Key;
use egui_kittest::kittest::{NodeT, Queryable as _};
use egui_kittest::Harness;
use shirtbuilder::ShirtBuilderApp;

const WIDTHS: [f32; 9] = [960.0, 1024.0, 1100.0, 1180.0, 1280.0, 1366.0, 1440.0, 1600.0, 1920.0];

fn harness(w: f32) -> Harness<'static, ShirtBuilderApp> {
    let mut h = Harness::builder().with_size([w, 700.0]).build_eframe(|cc| ShirtBuilderApp::new(cc));
    h.run_steps(8);
    h
}

/// labelled buttons inside the 52 px top bar
fn bar_buttons(h: &Harness<'_, ShirtBuilderApp>) -> Vec<(String, egui::Rect)> {
    let mut out = Vec::new();
    for n in h.root().children_recursive() {
        let a = n.accesskit_node();
        if a.role() != egui::accesskit::Role::Button {
            continue;
        }
        let (Some(label), Some(b)) = (a.label(), a.bounding_box()) else { continue };
        if b.y1 > 52.0 {
            continue;
        }
        out.push((label.to_string(), egui::Rect::from_min_max(egui::pos2(b.x0 as f32, b.y0 as f32), egui::pos2(b.x1 as f32, b.y1 as f32))));
    }
    out
}

#[test]
fn top_bar_buttons_never_overlap() {
    for w in WIDTHS {
        let h = harness(w);
        let buttons = bar_buttons(&h);
        assert!(buttons.len() >= 15, "{w}: only {} buttons found: {:?}", buttons.len(), buttons.iter().map(|b| &b.0).collect::<Vec<_>>());
        for (i, (a, ra)) in buttons.iter().enumerate() {
            assert!(ra.left() >= 0.0 && ra.right() <= w, "{w}: {a} at {ra:?} leaves the window");
            for (b, rb) in &buttons[i + 1..] {
                let overlap = ra.intersect(*rb);
                assert!(overlap.width() <= 0.5 || overlap.height() <= 0.5, "{w} px ({}): {a} {ra:?} overlaps {b} {rb:?}", h.state().top_bar_density());
            }
        }
    }
}

#[test]
fn top_bar_buttons_work_at_every_width() {
    for w in WIDTHS {
        let mut h = harness(w);
        let d = h.state().top_bar_density();
        h.get_by_label("Shortcuts & help (?)").click();
        h.run_steps(6);
        assert!(h.query_by_label("Shortcuts").is_some(), "{w} px ({d}): help button didn't open Shortcuts");
        h.key_press(Key::Escape);
        h.run_steps(4);
        h.get_by_label_contains("changelog").click();
        h.run_steps(6);
        assert!(h.query_by_label("What's new").is_some(), "{w} px ({d}): version badge didn't open the changelog");
        h.key_press(Key::Escape);
        h.run_steps(4);
        // the top bar's garment chip (the 3D pane has its own)
        h.get_all_by_label("Choose garment").find(|n| n.accesskit_node().bounding_box().is_some_and(|b| b.y1 <= 52.0)).expect("garment chip in the bar").click();
        h.run_steps(6);
        assert!(h.query_by_label("GARMENT").is_some(), "{w} px ({d}): garment picker didn't open");
    }
}

#[test]
fn top_bar_compacts_only_as_needed() {
    let order = ["Full", "IconButtons", "Short", "Tight"];
    let mut last = 3;
    for w in WIDTHS {
        let d = harness(w).state().top_bar_density();
        let i = order.iter().position(|o| *o == d).unwrap();
        assert!(i <= last, "{w} px is {d}, more compact than a narrower window");
        last = i;
    }
    assert_eq!(harness(1920.0).state().top_bar_density(), "Full");
    assert_eq!(harness(1440.0).state().top_bar_density(), "Full", "the usual window size should keep every label");
}
