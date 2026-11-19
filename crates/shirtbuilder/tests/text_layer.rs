//! Text artwork: a new text says LET'S BUILD IT, and the words are typed into the labelled
//! "Text" box at the top of the layer panel (there used to be an unlabelled name box there,
//! which looked like the text but didn't change the print).

use egui::accesskit::Role;
use egui::{Key, Modifiers};
use egui_kittest::kittest::Queryable as _;
use egui_kittest::Harness;
use sb_core::doc::LayerType;
use shirtbuilder::ShirtBuilderApp;

#[test]
fn new_text_says_lets_build_it() {
    let mut h = Harness::builder().with_size([1440.0, 900.0]).build_eframe(|cc| ShirtBuilderApp::new(cc));
    h.run_steps(6);
    h.get_by_role_and_label(Role::Button, "Text").click();
    h.run_steps(4);
    let l = h.state().selected_layer_for_tests().expect("the new text is selected");
    assert_eq!(l.kind, LayerType::Text);
    assert_eq!(l.text.as_deref(), Some("LET'S BUILD IT"));
}

#[test]
fn typing_in_the_text_box_changes_the_print() {
    let mut h = Harness::builder().with_size([1440.0, 900.0]).build_eframe(|cc| ShirtBuilderApp::new(cc));
    h.run_steps(6);
    h.get_by_role_and_label(Role::Button, "Text").click();
    h.run_steps(4);
    let before = h.state().selected_layer_for_tests().unwrap().clone();

    let field = h.get_by_role_and_label(Role::MultilineTextInput, "Text");
    field.focus();
    h.run_steps(2);
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.run_steps(1);
    h.get_by_role_and_label(Role::MultilineTextInput, "Text").type_text("SUMMER CREW 2026");
    h.run_steps(4);

    let after = h.state().selected_layer_for_tests().unwrap();
    assert_eq!(after.text.as_deref(), Some("SUMMER CREW 2026"), "the printed words follow the box");
    assert_eq!(after.name, "SUMMER CREW 2026", "the layer is named after its words");
    assert!((after.w - before.w).abs() < 1e-9, "width stays");
    assert!(after.h < before.h, "a longer line refits to a lower box");
}
