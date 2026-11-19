//! Default colours: new designs are a white shirt with black text, and a saved design that still
//! has the old defaults (green shirt, white text) is moved onto the new ones once.

use sb_core::doc::{default_doc, update_old_default_colours, LayerType, OLD_DEFAULT_FABRIC};
use sb_core::pattern::GarmentId;

fn tee() -> GarmentId {
    GarmentId::normalize("tee-short-m-M")
}

fn text_colours(d: &sb_core::doc::Doc) -> Vec<String> {
    d.layers.iter().filter(|l| l.kind == LayerType::Text).map(|l| l.color.clone().unwrap_or_default()).collect()
}

#[test]
fn new_designs_are_white_with_black_text() {
    let d = default_doc(tee());
    assert_eq!(d.fabric.color, "#ffffff");
    assert_eq!(text_colours(&d), vec!["#000000"]);
}

#[test]
fn old_default_design_becomes_white_with_black_text() {
    let mut d = default_doc(tee());
    d.fabric.color = OLD_DEFAULT_FABRIC.0.into();
    d.fabric.color2 = OLD_DEFAULT_FABRIC.1.into();
    for l in d.layers.iter_mut().filter(|l| l.kind == LayerType::Text) {
        l.color = Some("#FFFFFF".into());
    }
    assert!(update_old_default_colours(&mut d));
    assert_eq!(d.fabric.color, "#ffffff");
    assert_eq!(d.fabric.color2, "#c8ccd2");
    assert_eq!(text_colours(&d), vec!["#000000"]);
}

#[test]
fn chosen_colours_are_left_alone() {
    // a dark shirt colour the user picked: nothing changes, white text on it stays
    let mut d = default_doc(tee());
    d.fabric.color = "#243a5c".into();
    for l in d.layers.iter_mut().filter(|l| l.kind == LayerType::Text) {
        l.color = Some("#ffffff".into());
    }
    let before = d.clone();
    assert!(!update_old_default_colours(&mut d));
    assert_eq!(d, before);

    // a white shirt picked by hand with the old white slogan: the slogan turns black
    let mut d = default_doc(tee());
    d.fabric.color = "#ffffff".into();
    d.fabric.color2 = OLD_DEFAULT_FABRIC.1.into();
    for l in d.layers.iter_mut().filter(|l| l.kind == LayerType::Text) {
        l.color = Some("#ffffff".into());
    }
    assert!(update_old_default_colours(&mut d));
    assert_eq!(text_colours(&d), vec!["#000000"]);
    assert_eq!(d.fabric.color2, "#c8ccd2");

    // old green shirt, but text in a colour the user picked: only the shirt changes
    let mut d = default_doc(tee());
    d.fabric.color = OLD_DEFAULT_FABRIC.0.into();
    for l in d.layers.iter_mut().filter(|l| l.kind == LayerType::Text) {
        l.color = Some("#e0a84a".into());
    }
    assert!(update_old_default_colours(&mut d));
    assert_eq!(d.fabric.color, "#ffffff");
    assert_eq!(text_colours(&d), vec!["#e0a84a"]);
}
