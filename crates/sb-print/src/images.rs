//! Image artwork: layers keep their picture as a data URL (as in the web app's project files).
//! Raster images (PNG, JPEG, WebP, GIF, BMP) are decoded once; SVGs are parsed once and drawn
//! sharp at whatever size they are printed at.

use base64::Engine as _;
use resvg::tiny_skia::{IntSize, Pixmap, Transform};
use resvg::usvg;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, OnceLock};

pub enum Decoded {
    Raster(Pixmap),
    Svg(usvg::Tree),
}

impl Decoded {
    /// natural size in px
    pub fn size(&self) -> (f64, f64) {
        match self {
            Decoded::Raster(p) => (p.width() as f64, p.height() as f64),
            Decoded::Svg(t) => (t.size().width() as f64, t.size().height() as f64),
        }
    }
}

fn svg_options() -> &'static usvg::Options<'static> {
    static OPT: OnceLock<usvg::Options<'static>> = OnceLock::new();
    OPT.get_or_init(|| {
        let mut db = usvg::fontdb::Database::new();
        db.load_font_data(crate::text::INTER_REGULAR.to_vec());
        db.load_font_data(crate::text::INTER_SEMIBOLD.to_vec());
        db.set_sans_serif_family("Inter");
        db.set_serif_family("Inter");
        let mut o = usvg::Options { font_family: "Inter".into(), ..Default::default() };
        o.fontdb = Arc::new(db);
        o
    })
}

/// `%xx` decoding (encodeURIComponent'd SVG data URLs)
fn percent_decode(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

/// raw bytes and MIME type of a data URL
pub fn data_url_bytes(src: &str) -> Result<(String, Vec<u8>), String> {
    let rest = src.strip_prefix("data:").ok_or("not a data URL")?;
    let (head, data) = rest.split_once(',').ok_or("malformed data URL")?;
    let mime = head.split(';').next().unwrap_or("").to_lowercase();
    let bytes = if head.split(';').any(|p| p.eq_ignore_ascii_case("base64")) {
        base64::engine::general_purpose::STANDARD.decode(data.trim()).map_err(|e| format!("base64: {e}"))?
    } else {
        percent_decode(data)
    };
    Ok((mime, bytes))
}

pub fn decode_bytes(mime: &str, bytes: &[u8]) -> Result<Decoded, String> {
    let looks_svg = mime.contains("svg") || bytes.iter().take(512).collect::<Vec<_>>().windows(4).any(|w| w == [&b'<', &b's', &b'v', &b'g']);
    if looks_svg {
        let tree = usvg::Tree::from_data(bytes, svg_options()).map_err(|e| format!("SVG: {e}"))?;
        return Ok(Decoded::Svg(tree));
    }
    let img = image::load_from_memory(bytes).map_err(|e| format!("image: {e}"))?.to_rgba8();
    let (w, h) = img.dimensions();
    let mut data = img.into_raw();
    // tiny-skia stores premultiplied alpha
    for px in data.chunks_exact_mut(4) {
        let a = px[3] as u32;
        if a < 255 {
            for c in &mut px[..3] {
                *c = ((*c as u32 * a + 127) / 255) as u8;
            }
        }
    }
    let size = IntSize::from_wh(w, h).ok_or("empty image")?;
    Pixmap::from_vec(data, size).map(Decoded::Raster).ok_or_else(|| "bad image size".into())
}

pub fn decode(src: &str) -> Result<Decoded, String> {
    let (mime, bytes) = data_url_bytes(src)?;
    decode_bytes(&mime, &bytes)
}

pub fn key(src: &str) -> u64 {
    let mut h = DefaultHasher::new();
    src.hash(&mut h);
    h.finish()
}

/// decoded images by data URL (decode once, draw many times)
#[derive(Default, Clone)]
pub struct ImageCache {
    map: HashMap<u64, Option<Arc<Decoded>>>,
}

impl ImageCache {
    /// decode any images of `srcs` not seen yet (call before rendering)
    pub fn prepare<'a>(&mut self, srcs: impl IntoIterator<Item = &'a str>) {
        for s in srcs {
            let k = key(s);
            if !self.map.contains_key(&k) {
                let d = decode(s);
                if let Err(e) = &d {
                    log_warn(&format!("image layer could not be decoded: {e}"));
                }
                self.map.insert(k, d.ok().map(Arc::new));
            }
        }
    }
    pub fn get(&self, src: &str) -> Option<&Arc<Decoded>> {
        self.map.get(&key(src)).and_then(|d| d.as_ref())
    }
    /// forget images no longer used by `srcs`
    pub fn retain<'a>(&mut self, srcs: impl IntoIterator<Item = &'a str>) {
        let keep: std::collections::HashSet<u64> = srcs.into_iter().map(key).collect();
        self.map.retain(|k, _| keep.contains(k));
    }
}

fn log_warn(msg: &str) {
    #[cfg(debug_assertions)]
    eprintln!("sb-print: {msg}");
    let _ = msg;
}

/// render an SVG at `w`×`h` px
pub fn rasterize_svg(tree: &usvg::Tree, w: u32, h: u32) -> Option<Pixmap> {
    let mut pm = Pixmap::new(w.max(1), h.max(1))?;
    let s = tree.size();
    let ts = Transform::from_scale(w as f32 / s.width(), h as f32 / s.height());
    resvg::render(tree, ts, &mut pm.as_mut());
    Some(pm)
}
