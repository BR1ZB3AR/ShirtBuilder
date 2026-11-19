//! The design document — compatible with the web app's project `.json` files in both directions.

use crate::pattern::{garment, Garment, GarmentId, PieceId, PieceKind, DEFAULT_GARMENT};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    #[default]
    Outside,
    Inside,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LayerType {
    Image,
    Text,
    Shape,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShapeKind {
    Rect,
    Ellipse,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layer {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: LayerType,
    pub piece: PieceId,
    pub side: Side,
    /// centre, cm, in the piece's view frame for its side
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// degrees
    pub rot: f64,
    pub opacity: f64,
    pub visible: bool,
    // text
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<Align>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_height: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub letter_spacing: Option<f64>,
    // image
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src: Option<String>,
    // shape
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<ShapeKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FabricMode {
    Solid,
    Gradient,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MaterialId {
    Cotton,
    Jersey,
    Polyester,
    Linen,
    Fleece,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Fabric {
    pub color: String,
    pub mode: FabricMode,
    pub color2: String,
    pub angle: f64,
    pub material: MaterialId,
    pub ao: bool,
    pub wrinkles_on: bool,
    pub wrinkles: f64,
    pub texture: f64,
    pub knit: f64,
    pub brightness: f64,
    pub contrast: f64,
}

impl Default for Fabric {
    fn default() -> Self {
        Fabric {
            color: "#ffffff".into(),
            mode: FabricMode::Solid,
            color2: "#c8ccd2".into(),
            angle: 90.0,
            material: MaterialId::Cotton,
            ao: true,
            wrinkles_on: true,
            wrinkles: 0.5,
            texture: 0.5,
            knit: 3.0,
            brightness: 1.0,
            contrast: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Backdrop {
    Window,
    Leaves,
    Studio,
    Gradient,
    Solid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelKind {
    Scan,
    Generated,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SceneSettings {
    pub bg: Backdrop,
    pub match_fabric: bool,
    pub bg_color: String,
    pub light: f64,
    pub light_angle: f64,
    pub env: f64,
    pub exposure: f64,
    /// true = on a hanger, false = ghost mannequin (worn shape)
    pub hanger: bool,
    /// realistic scanned model where one exists, or the generated shirt
    pub model: ModelKind,
    /// window-light pattern projected onto the shirt
    pub gobo: bool,
    pub shadows: bool,
    pub auto_rotate: bool,
}

impl Default for SceneSettings {
    fn default() -> Self {
        SceneSettings {
            bg: Backdrop::Window,
            match_fabric: true,
            bg_color: "#8fa894".into(),
            light: 1.0,
            light_angle: -30.0,
            env: 1.0,
            exposure: 1.0,
            hanger: false,
            model: ModelKind::Scan,
            gobo: true,
            shadows: true,
            auto_rotate: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Doc {
    pub version: u32,
    pub garment: GarmentId,
    pub layers: Vec<Layer>,
    #[serde(default)]
    pub fabric: Fabric,
    #[serde(default)]
    pub scene: SceneSettings,
}

static UID_SEQ: AtomicU64 = AtomicU64::new(1);
static UID_SEED: AtomicU64 = AtomicU64::new(0x5b1d);

/// seed layer ids (e.g. with the start-up time) so ids from different sessions don't collide
pub fn seed_uids(seed: u64) {
    UID_SEED.store(seed, Ordering::Relaxed);
}

fn base36(mut n: u64) -> String {
    const D: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if n == 0 {
        return "0".into();
    }
    let mut s = vec![];
    while n > 0 {
        s.push(D[(n % 36) as usize]);
        n /= 36;
    }
    s.reverse();
    String::from_utf8(s).unwrap()
}

/// new layer id, same shape as the web app's (`l` + base-36)
pub fn uid() -> String {
    format!("l{}{}", base36(UID_SEED.load(Ordering::Relaxed)), base36(UID_SEQ.fetch_add(1, Ordering::Relaxed)))
}

const LOGO_SVG_SRC: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="360" height="104" viewBox="0 0 360 104">
<rect x="0" y="0" width="360" height="104" rx="14" fill="#ffffff"/>
<g transform="translate(18,20)" fill="#111">
<path d="M22 0 L10 0 L0 10 L7 20 L12 16 L12 64 L52 64 L52 16 L57 20 L64 10 L54 0 L42 0 C40 6 36 9 32 9 C28 9 24 6 22 0 Z"/>
</g>
<text x="98" y="66" font-family="Helvetica, Arial, sans-serif" font-size="40" font-weight="700" fill="#111" letter-spacing="-0.5">Shirt<tspan fill="#5f8f6a">Builder</tspan></text>
</svg>"##;

/// JavaScript `encodeURIComponent`
fn encode_uri_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// the small "ShirtBuilder" badge used for the default sleeve print and inside neck label
pub fn logo_data_url() -> String {
    format!("data:image/svg+xml;charset=utf-8,{}", encode_uri_component(LOGO_SVG_SRC))
}

fn layer(name: &str, kind: LayerType, piece: PieceId, side: Side, x: f64, y: f64, w: f64, h: f64, rot: f64) -> Layer {
    Layer {
        id: uid(),
        name: name.into(),
        kind,
        piece,
        side,
        x,
        y,
        w,
        h,
        rot,
        opacity: 1.0,
        visible: true,
        text: None,
        font: None,
        weight: None,
        italic: None,
        color: None,
        align: None,
        line_height: None,
        letter_spacing: None,
        src: None,
        shape: None,
        fill: None,
        radius: None,
    }
}

/// The defaults before v0.0.9: green fabric with a dark-green gradient partner, white text.
pub const OLD_DEFAULT_FABRIC: (&str, &str) = ("#4f7a59", "#2b4a35");

/// Move a design made with the old defaults onto the new ones (run once per device):
/// - the old default green fabric becomes white;
/// - on a light fabric (that one, or a white / light shirt picked since), the old dark-green
///   gradient partner becomes light grey and white text becomes black — white text on a light
///   shirt can't be seen, which is how the old white slogan ended up on white shirts — as does
///   the old near-black default for new text.
/// A dark fabric and its text are left alone. Returns whether anything changed.
pub fn update_old_default_colours(doc: &mut Doc) -> bool {
    let before = doc.clone();
    let new = Fabric::default();
    if doc.fabric.color.eq_ignore_ascii_case(OLD_DEFAULT_FABRIC.0) {
        doc.fabric.color = new.color.clone();
    }
    if hex_luminance(&doc.fabric.color).is_some_and(|l| l >= 0.6) {
        if doc.fabric.color2.eq_ignore_ascii_case(OLD_DEFAULT_FABRIC.1) {
            doc.fabric.color2 = new.color2;
        }
        for l in doc.layers.iter_mut().filter(|l| l.kind == LayerType::Text) {
            // white text, or the old near-black default for new text (#141414) → black
            if l.color.as_deref().is_some_and(|c| ["#ffffff", "#fff", "#141414"].iter().any(|o| c.eq_ignore_ascii_case(o))) {
                l.color = Some("#000000".into());
            }
        }
    }
    *doc != before
}

/// relative luminance (0 black … 1 white) of a `#rrggbb` colour
fn hex_luminance(hex: &str) -> Option<f64> {
    let h = hex.trim().trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let ch = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok().map(|v| {
        let c = v as f64 / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    });
    Some(0.2126 * ch(0)? + 0.7152 * ch(2)? + 0.0722 * ch(4)?)
}

/// a fresh design: logo inside the back neck and on the left sleeve, slogan on the chest
pub fn default_doc(id: GarmentId) -> Doc {
    let g = garment(id);
    let sp = &g.spec;
    let front = g.front();
    let back = g.back();
    let sl = g.piece(PieceId::SleeveL).unwrap();
    let collared = sp.collar != crate::pattern::CollarKind::Rib;
    let (cx, cy, cw, ch) = if collared {
        // collared styles get a smaller chest print, clear of the placket (and pocket)
        (front.w / 2.0 + if sp.pocket { -9.5 } else { 9.5 }, sp.front_neck + 13.0, 11.0, 9.0)
    } else {
        (front.w / 2.0, 24.0 * (front.h / 73.0), 33.0 * (front.w / 53.0), 27.0 * (front.w / 53.0))
    };
    let mut inside_logo = layer("ShirtBuilder", LayerType::Image, PieceId::Back, Side::Inside, back.w / 2.0, 6.0, 9.0, 2.6, 0.0);
    inside_logo.src = Some(logo_data_url());
    let mut sleeve_logo = layer(
        "ShirtBuilder",
        LayerType::Image,
        PieceId::SleeveL,
        Side::Outside,
        sl.w * 0.66,
        sl.h - (5.5f64).max(sp.cuff_h + sp.sleeve_band + 3.0),
        8.0,
        2.3,
        0.0,
    );
    sleeve_logo.src = Some(logo_data_url());
    let mut slogan = layer("BUILD IT. NOT GENERATE IT.", LayerType::Text, PieceId::Front, Side::Outside, cx, cy, cw, ch, -4.0);
    slogan.text = Some("BUILD IT.\nNOT\nGENERATE IT.".into());
    slogan.font = Some("Permanent Marker".into());
    slogan.weight = Some(400.0);
    slogan.italic = Some(false);
    slogan.color = Some("#000000".into()); // black print on the default white fabric
    slogan.align = Some(Align::Center);
    slogan.line_height = Some(1.0);
    slogan.letter_spacing = Some(0.0);
    Doc { version: 1, garment: id, layers: vec![inside_logo, sleeve_logo, slogan], fabric: Fabric::default(), scene: SceneSettings::default() }
}

impl Default for Doc {
    fn default() -> Self {
        default_doc(DEFAULT_GARMENT)
    }
}

#[derive(Debug)]
pub enum LoadError {
    Json(serde_json::Error),
    NotAProject,
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Json(e) => write!(f, "not valid JSON: {e}"),
            LoadError::NotAProject => write!(f, "not a ShirtBuilder project file"),
        }
    }
}

/// Open a project file (web or Rust edition). Like the web app's `normalizeDoc`: unknown garment ids
/// become the default, missing settings take defaults, layers on pieces that don't exist are re-homed,
/// and saves from before the realistic preview move onto the ghost-mannequin / window-light scene.
pub fn load_doc(json: &str) -> Result<Doc, LoadError> {
    let mut v: serde_json::Value = serde_json::from_str(json).map_err(LoadError::Json)?;
    let obj = v.as_object_mut().ok_or(LoadError::NotAProject)?;
    if obj.get("version").and_then(|x| x.as_u64()) != Some(1) || !obj.get("layers").is_some_and(|l| l.is_array()) {
        return Err(LoadError::NotAProject);
    }
    if !obj.contains_key("garment") {
        obj.insert("garment".into(), serde_json::Value::String(DEFAULT_GARMENT.to_string()));
    }
    if let Some(scene) = obj.get_mut("scene").and_then(|s| s.as_object_mut()) {
        if !scene.contains_key("gobo") {
            scene.insert("hanger".into(), false.into());
            scene.insert("bg".into(), "window".into());
            scene.insert("gobo".into(), true.into());
        }
    }
    let mut doc: Doc = serde_json::from_value(v).map_err(LoadError::Json)?;
    doc.layers = remap_layers(&doc.layers, doc.garment, doc.garment);
    Ok(doc)
}

pub fn save_doc(doc: &Doc) -> String {
    serde_json::to_string_pretty(doc).expect("documents always serialise")
}

/// File extension of ShirtBuilder design files.
pub const SB_EXT: &str = "sb";
/// `format` value that marks a `.sb` file.
pub const SB_FORMAT: &str = "shirtbuilder-design";

/// A `.sb` design file: JSON with a small header around the design (pieces, artwork with its
/// images embedded, fabric and scene), so one file reopens exactly as it was saved.
/// `app` names the program that wrote it (for information only).
pub fn save_sb(doc: &Doc, app: &str) -> String {
    let v = serde_json::json!({
        "format": SB_FORMAT,
        "formatVersion": 1,
        "app": app,
        "design": doc,
    });
    serde_json::to_string_pretty(&v).expect("documents always serialise")
}

/// Read a design: a `.sb` file, or a bare project `.json` (web app / earlier versions).
pub fn load_design(text: &str) -> Result<Doc, LoadError> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(LoadError::Json)?;
    if v.get("format").and_then(|f| f.as_str()) == Some(SB_FORMAT) {
        let design = v.get("design").ok_or(LoadError::NotAProject)?;
        return load_doc(&design.to_string());
    }
    load_doc(text)
}

/// Carry artwork over to another garment: pieces that don't exist there are re-homed
/// (neck rib ⇄ collar), positions follow the piece proportions, sleeve art stays anchored to the
/// nearer end (cap or hem); a size change keeps the print size.
pub fn remap_layers(layers: &[Layer], from: GarmentId, to: GarmentId) -> Vec<Layer> {
    let gf = garment(from);
    let gt = garment(to);
    layers.iter().map(|l| remap_layer(l, &gf, &gt, from, to)).collect()
}

fn remap_layer(l: &Layer, gf: &Garment, gt: &Garment, from: GarmentId, to: GarmentId) -> Layer {
    use PieceId::*;
    let mut piece = l.piece;
    if !gt.has_piece(piece) {
        piece = match piece {
            Collar => RibB,
            RibF | RibB => Collar,
            _ => Front,
        };
    }
    if !gt.has_piece(piece) {
        piece = Front;
    }
    let pt = gt.piece(piece).unwrap();
    let Some(pf) = gf.piece(l.piece) else {
        return Layer { piece, x: pt.w / 2.0, y: pt.h / 2.0, ..l.clone() };
    };
    if std::ptr::eq(pf, pt) {
        return l.clone();
    }
    let sx = pt.w / pf.w;
    if pt.kind != pf.kind {
        // rib ⇄ collar: centre on the visible band, shrink to fit
        let k = 1f64.min((pt.h * 0.7) / l.h).min((pt.w * 0.8) / l.w);
        let y = if pt.kind == PieceKind::Collar { gt.spec.collar_stand + gt.spec.collar_fall / 2.0 } else { pt.h / 2.0 };
        return Layer { piece, x: l.x * sx, y, w: l.w * k, h: l.h * k, ..l.clone() };
    }
    let mut y = l.y * (pt.h / pf.h);
    if pt.kind == PieceKind::Sleeve {
        y = if l.y > pf.h / 2.0 { pt.h - (pf.h - l.y) } else { l.y };
    }
    let size_only = from.with_size(to.size) == to;
    let k = if pt.kind == PieceKind::Body && !size_only { sx } else { 1.0 };
    Layer { piece, x: l.x * sx, y, w: l.w * k, h: l.h * k, ..l.clone() }
}
