//! Nothing in the sidebar may be wider than the sidebar: an over-wide widget pushes egui's right
//! panel open past the window edge and the central panes then slide under the sidebar.

use egui_kittest::kittest::{NodeT, Queryable as _};
use egui_kittest::Harness;
use shirtbuilder::ShirtBuilderApp;

fn overflowing(h: &Harness<'_, ShirtBuilderApp>, width: f32) -> Vec<String> {
    let mut out = Vec::new();
    for n in h.root().children_recursive() {
        let Some(b) = n.accesskit_node().bounding_box() else { continue };
        // the bottom edge (scroll area) legitimately reaches the window's corner
        if b.x1 as f32 > width + 0.5 && (b.y1 as f32) < 890.0 {
            out.push(format!("{:?} {:?} right {:.1}", n.accesskit_node().role(), n.accesskit_node().label(), b.x1));
        }
    }
    out
}

#[test]
fn sidebar_fits_with_every_layer_panel() {
    let mut h = Harness::builder().with_size([1440.0, 900.0]).build_eframe(|cc| ShirtBuilderApp::new(cc));
    h.run();
    assert!(overflowing(&h, 1440.0).is_empty(), "{:?}", overflowing(&h, 1440.0));
    for layer in ["BUILD IT. NOT GENERATE IT.", "ShirtBuilder"] {
        h.get_all_by_label(layer).next().unwrap().click();
        for _ in 0..4 {
            h.step();
        }
        let bad = overflowing(&h, 1440.0);
        assert!(bad.is_empty(), "{layer}: {bad:?}");
    }
}

#[test]
fn theme_menu_stays_open_while_filtering_and_picking() {
    let mut h = Harness::builder().with_size([1440.0, 900.0]).build_eframe(|cc| ShirtBuilderApp::new(cc));
    h.run();
    h.get_by_label("Theme").click();
    h.run();
    for chip in ["Cyberpunk", "Cyberpunk", "Light", "Light"] {
        h.get_by_label(chip).click();
        h.run();
        assert!(h.query_by_label("Accent colour").is_some(), "menu closed after clicking {chip}");
    }
    h.get_by_label("Light").click();
    h.run();
    h.get_by_label("Daylight").click();
    h.run();
    assert!(h.query_by_label("Accent colour").is_some(), "menu closed after picking a theme");
}
