//! Starting up restores the last design; one still on the old default colours (green shirt,
//! white text) comes back white with black text, once.

use sb_core::doc::{default_doc, save_doc, LayerType, OLD_DEFAULT_FABRIC};
use sb_core::pattern::GarmentId;
use shirtbuilder::restore_saved;

/// a design + preferences as saved by v0.0.8 and earlier
fn old_save() -> (String, String) {
    let mut d = default_doc(GarmentId::normalize("tee-short-m-M"));
    d.fabric.color = OLD_DEFAULT_FABRIC.0.into();
    d.fabric.color2 = OLD_DEFAULT_FABRIC.1.into();
    for l in d.layers.iter_mut().filter(|l| l.kind == LayerType::Text) {
        l.color = Some("#ffffff".into());
    }
    let prefs = r#"{"mode":"split","tab":"design","unit":"cm","seam_allowance":true,"grid":false,"quality":"auto","show_fps":false,"seen_version":"0.0.8","theme":"forest-dark","theme_system":false,"theme_light":"daylight","theme_dark":"forest-dark","accent":null}"#;
    (save_doc(&d), prefs.to_string())
}

#[test]
fn old_default_design_opens_white_with_black_text_once() {
    let (doc, prefs) = old_save();
    let (d, p) = restore_saved(Some(&doc), Some(&prefs));
    assert_eq!(d.fabric.color, "#ffffff");
    assert!(d.layers.iter().filter(|l| l.kind == LayerType::Text).all(|l| l.color.as_deref() == Some("#000000")));
    assert!(p.colour_defaults_v2);

    // after that, going back to the old green on purpose sticks
    let mut d2 = d.clone();
    d2.fabric.color = OLD_DEFAULT_FABRIC.0.into();
    let prefs2 = serde_json::to_string(&serde_json::json!({ "colour_defaults_v2": true })).unwrap();
    let (d3, _) = restore_saved(Some(&save_doc(&d2)), Some(&prefs2));
    assert_eq!(d3.fabric.color, OLD_DEFAULT_FABRIC.0);
}

#[test]
fn first_start_is_white_with_black_text() {
    let (d, _) = restore_saved(None, None);
    assert_eq!(d.fabric.color, "#ffffff");
    assert!(d.layers.iter().filter(|l| l.kind == LayerType::Text).all(|l| l.color.as_deref() == Some("#000000")));
}
