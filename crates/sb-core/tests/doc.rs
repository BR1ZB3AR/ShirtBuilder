//! Project files: compatible with the web app in both directions.

use sb_core::doc::{default_doc, load_doc, remap_layers, save_doc, Backdrop, LayerType, Side};
use sb_core::pattern::{garment, GarmentId, PieceId, Size, DEFAULT_GARMENT};

/// a project as the web app writes it, using an old garment id and an old scene, with missing fields
const WEB_PROJECT: &str = r##"{
 "version": 1,
 "garment": "crew",
 "layers": [
  {"id":"lmabc1","name":"Hello","type":"text","piece":"ribF","side":"outside","x":10,"y":3,"w":8,"h":1.6,"rot":0,
   "opacity":1,"visible":true,"text":"HELLO","font":"Inter","weight":700,"italic":false,"color":"#ffffff",
   "align":"center","lineHeight":1,"letterSpacing":0},
  {"id":"lmabc2","name":"Box","type":"shape","piece":"front","side":"inside","x":20,"y":30,"w":10,"h":5,"rot":12,
   "opacity":0.5,"visible":false,"shape":"ellipse","fill":"#ff0000","radius":0}
 ],
 "fabric": {"color":"#e31b23","mode":"solid"},
 "scene": {"bg":"leaves","hanger":true,"light":1.2}
}"##;

#[test]
fn opens_web_projects_and_normalises_them() {
    let d = load_doc(WEB_PROJECT).expect("loads");
    assert_eq!(d.garment, DEFAULT_GARMENT, "'crew' becomes the round-neck tee, size M");
    assert_eq!(d.garment.to_string(), "tee-short-m-M");
    // missing fabric fields take defaults, given ones are kept
    assert_eq!(d.fabric.color, "#e31b23");
    assert_eq!(d.fabric.knit, 3.0);
    // a scene saved before the realistic preview moves onto the new look once
    assert!(!d.scene.hanger);
    assert_eq!(d.scene.bg, Backdrop::Window);
    assert!(d.scene.gobo);
    assert_eq!(d.scene.light, 1.2);
    // layers survive with every field
    assert_eq!(d.layers.len(), 2);
    assert_eq!(d.layers[0].kind, LayerType::Text);
    assert_eq!(d.layers[0].piece, PieceId::RibF);
    assert_eq!(d.layers[0].text.as_deref(), Some("HELLO"));
    assert_eq!(d.layers[1].side, Side::Inside);
    assert!(!d.layers[1].visible);
}

#[test]
fn saved_files_use_the_web_field_names_and_round_trip() {
    let d = load_doc(WEB_PROJECT).unwrap();
    let json = save_doc(&d);
    for key in ["\"type\": \"text\"", "\"lineHeight\"", "\"letterSpacing\"", "\"wrinklesOn\"", "\"matchFabric\"", "\"autoRotate\"", "\"garment\": \"tee-short-m-M\""] {
        assert!(json.contains(key), "missing {key} in\n{json}");
    }
    assert!(!json.contains("\"src\""), "absent optional fields are not written");
    let again = load_doc(&json).unwrap();
    assert_eq!(again, d);
}

#[test]
fn rejects_files_that_are_not_projects() {
    assert!(load_doc("{\"hello\": 1}").is_err());
    assert!(load_doc("not json").is_err());
    assert!(load_doc("{\"version\": 2, \"layers\": []}").is_err());
}

#[test]
fn rib_art_moves_to_the_collar_and_back() {
    let tee = DEFAULT_GARMENT;
    let polo = GarmentId::normalize("polo-short-m-M");
    let d = load_doc(WEB_PROJECT).unwrap();
    let on_polo = remap_layers(&d.layers, tee, polo);
    assert_eq!(on_polo[0].piece, PieceId::Collar);
    let g = garment(polo);
    let collar = g.piece(PieceId::Collar).unwrap();
    assert!(on_polo[0].w <= collar.w * 0.8 + 1e-9 && on_polo[0].h <= collar.h * 0.7 + 1e-9);
    let back = remap_layers(&on_polo, polo, tee);
    assert_eq!(back[0].piece, PieceId::RibB);
}

#[test]
fn size_changes_keep_print_size() {
    let d = default_doc(DEFAULT_GARMENT);
    let xxl = DEFAULT_GARMENT.with_size(Size::XXL);
    let moved = remap_layers(&d.layers, DEFAULT_GARMENT, xxl);
    let slogan = d.layers.iter().position(|l| l.piece == PieceId::Front).unwrap();
    assert_eq!(moved[slogan].w, d.layers[slogan].w, "print keeps its size");
    assert!(moved[slogan].x > d.layers[slogan].x, "but re-centres on the wider panel");
}

#[test]
fn every_garment_has_a_valid_default_design() {
    for id in GarmentId::all() {
        let d = default_doc(id);
        let g = garment(id);
        for l in &d.layers {
            assert!(g.has_piece(l.piece), "{id}: layer on missing piece {:?}", l.piece);
        }
    }
}
