//! New / Open / Save with `.sb` design files, clicking the real buttons: saving goes back into
//! the file that was opened, New asks before unsaved changes would be lost (Save / Don't save /
//! Cancel), and Ctrl+Z brings the previous design back.

use egui::{Key, Modifiers};
use egui_kittest::kittest::Queryable as _;
use egui_kittest::Harness;
use sb_core::doc::{default_doc, load_design, save_doc, save_sb, Doc};
use sb_core::pattern::GarmentId;
use shirtbuilder::ShirtBuilderApp;
use std::path::PathBuf;

const NEW: &str = "New design (Ctrl+N)";
const SAVE: &str = "Save (Ctrl+S)";

fn dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("shirtbuilder-test-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// a navy design saved as "Summer tee.sb"
fn saved_design(tag: &str) -> (PathBuf, Doc) {
    let mut d = default_doc(GarmentId::normalize("tee-short-m-M"));
    d.fabric.color = "#243a5c".into();
    let path = dir(tag).join("Summer tee.sb");
    std::fs::write(&path, save_sb(&d, "test")).unwrap();
    (path, d)
}

fn app_with(path: &std::path::Path) -> Harness<'static, ShirtBuilderApp> {
    let mut h = Harness::builder().with_size([1440.0, 900.0]).build_eframe(|cc| ShirtBuilderApp::new(cc));
    h.run_steps(6);
    assert!(h.state_mut().open_file(path), "couldn't open {}", path.display());
    h.run_steps(4);
    h
}

/// make an unsaved change the way a user would: Artwork › Text
fn add_text(h: &mut Harness<'_, ShirtBuilderApp>) {
    h.get_by_label("Text").click();
    h.run_steps(4);
}

fn layers(h: &Harness<'_, ShirtBuilderApp>) -> usize {
    h.state().doc_for_tests().layers.len()
}

#[test]
fn save_goes_back_into_the_opened_file() {
    let (path, d) = saved_design("save");
    let mut h = app_with(&path);
    assert_eq!(h.state().file_name(), "Summer tee.sb");
    assert!(!h.state().has_unsaved_changes());

    add_text(&mut h);
    assert!(h.state().has_unsaved_changes());
    h.get_by_label(SAVE).click();
    h.run_steps(4);
    assert!(!h.state().has_unsaved_changes(), "Save didn't save");
    let back = load_design(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(back.layers.len(), d.layers.len() + 1, "the new text is in the file");
    assert_eq!(back.fabric.color, "#243a5c");
    assert_eq!(&back, h.state().doc_for_tests(), "the file holds exactly the design on screen");
}

#[test]
fn new_asks_before_unsaved_changes_are_lost() {
    let (path, d) = saved_design("new");
    let mut h = app_with(&path);
    add_text(&mut h);
    let edited = layers(&h);
    let ask = "Save changes to Summer tee.sb?";

    // Cancel: nothing happens
    h.get_by_label(NEW).click();
    h.run_steps(4);
    assert!(h.query_by_label(ask).is_some(), "New didn't ask");
    h.get_by_label("Cancel").click();
    h.run_steps(4);
    assert!(h.query_by_label(ask).is_none());
    assert_eq!(layers(&h), edited);
    assert_eq!(h.state().file_name(), "Summer tee.sb");

    // Don't save: a fresh design, the file untouched
    h.get_by_label(NEW).click();
    h.run_steps(4);
    h.get_by_label("Don't save").click();
    h.run_steps(4);
    assert_eq!(h.state().file_name(), "Untitled");
    assert!(!h.state().has_unsaved_changes());
    assert_eq!(h.state().doc_for_tests().fabric.color, "#ffffff", "new designs are white");
    assert_eq!(load_design(&std::fs::read_to_string(&path).unwrap()).unwrap(), d, "Don't save left the file alone");

    // Ctrl+Z brings the edited design back
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    h.run_steps(4);
    assert_eq!(layers(&h), edited);
}

#[test]
fn save_in_the_question_saves_then_starts_new() {
    let (path, d) = saved_design("ask-save");
    let mut h = app_with(&path);
    add_text(&mut h);
    h.get_by_label(NEW).click();
    h.run_steps(4);
    h.get_by_label("Save").click();
    h.run_steps(4);
    assert_eq!(load_design(&std::fs::read_to_string(&path).unwrap()).unwrap().layers.len(), d.layers.len() + 1, "saved first");
    assert_eq!(h.state().file_name(), "Untitled", "then started a new design");
}

#[test]
fn new_on_a_saved_design_does_not_ask() {
    let (path, _) = saved_design("clean");
    let mut h = app_with(&path);
    h.get_by_label(NEW).click();
    h.run_steps(4);
    assert!(h.query_by_label("Don't save").is_none(), "asked although nothing was unsaved");
    assert_eq!(h.state().file_name(), "Untitled");
}

#[test]
fn a_web_project_json_opens_as_a_new_design() {
    let d = default_doc(GarmentId::normalize("polo-short-f-M"));
    let path = dir("json").join("from-web.json");
    std::fs::write(&path, save_doc(&d)).unwrap();
    let h = app_with(&path);
    assert_eq!(h.state().doc_for_tests(), &d);
    assert_eq!(h.state().file_name(), "Untitled", "Save will ask where to put the .sb");
    assert!(!h.state().has_unsaved_changes());
}
