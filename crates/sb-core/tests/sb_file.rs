//! `.sb` design files: what is saved comes back identical, web-app / older `.json` projects still
//! open, and other files are refused.

use sb_core::doc::{default_doc, load_design, save_doc, save_sb, LayerType, SB_FORMAT};
use sb_core::pattern::GarmentId;

#[test]
fn sb_round_trips_exactly() {
    let mut d = default_doc(GarmentId::normalize("polo-long-f-L"));
    d.fabric.color = "#243a5c".into();
    for l in d.layers.iter_mut().filter(|l| l.kind == LayerType::Text) {
        l.text = Some("SUMMER\nCREW".into());
        l.color = Some("#e0a84a".into());
    }
    let text = save_sb(&d, "ShirtBuilder test");
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["format"], SB_FORMAT);
    assert_eq!(v["formatVersion"], 1);
    assert_eq!(load_design(&text).unwrap(), d);
}

#[test]
fn images_travel_inside_the_file() {
    let d = default_doc(GarmentId::normalize("tee-short-m-M"));
    let images: Vec<_> = d.layers.iter().filter_map(|l| l.src.clone()).collect();
    assert!(!images.is_empty() && images.iter().all(|s| s.starts_with("data:")), "default artwork is embedded");
    let back = load_design(&save_sb(&d, "test")).unwrap();
    assert_eq!(back.layers.iter().filter_map(|l| l.src.clone()).collect::<Vec<_>>(), images);
}

#[test]
fn project_json_still_opens() {
    let d = default_doc(GarmentId::normalize("shirt-short-m-M"));
    assert_eq!(load_design(&save_doc(&d)).unwrap(), d);
}

#[test]
fn other_files_are_refused() {
    assert!(load_design("not json at all").is_err());
    assert!(load_design(r#"{"hello":"world"}"#).is_err());
    assert!(load_design(&format!(r#"{{"format":"{SB_FORMAT}","formatVersion":1}}"#)).is_err(), "header without a design");
}
