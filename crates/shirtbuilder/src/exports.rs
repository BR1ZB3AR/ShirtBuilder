//! Exports (print sheets, print & cut SVGs, cut lines, 3D snapshot) and image artwork files.
//! Desktop: a save dialog, then the file is rendered on a background thread and written.
//! Web: the file is rendered and downloaded by the browser.

use crate::app::ShirtBuilderApp;
use base64::Engine as _;
use sb_core::doc::{Doc, Layer, LayerType, Side};
use sb_core::pattern::{garment, PieceId, PieceKind, Unit};
use sb_print::ImageCache;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Export {
    PrintPng { side: Side, dpi: f64 },
    PrintSvg { side: Side },
    CutSvg,
    Snapshot,
    Project,
}

fn side_name(s: Side) -> &'static str {
    if s == Side::Inside {
        "inside"
    } else {
        "outside"
    }
}

/// render an export into (file name, bytes, mime type)
pub fn render(doc: &Doc, e: Export, unit: Unit, sa: bool) -> Result<(String, Vec<u8>, &'static str), String> {
    let mut imgs = ImageCache::default();
    imgs.prepare(sb_print::image_srcs(doc));
    let id = doc.garment;
    match e {
        Export::PrintPng { side, dpi } => Ok((format!("shirtbuilder-{id}-{}-{dpi:.0}dpi.png", side_name(side)), sb_print::sheets::pattern_png(doc, side, unit, sa, dpi, &imgs)?, "image/png")),
        Export::PrintSvg { side } => Ok((format!("shirtbuilder-{id}-{}.svg", side_name(side)), sb_print::sheets::print_svg(doc, side, unit, sa, 150.0, &imgs)?.into_bytes(), "image/svg+xml")),
        Export::CutSvg => Ok((format!("shirtbuilder-{id}-cutlines.svg"), sb_print::sheets::cutlines_svg(doc, unit, sa).into_bytes(), "image/svg+xml")),
        Export::Snapshot | Export::Project => Err("not a sheet export".into()),
    }
}

/// offer `bytes` to the user: save dialog (desktop) or browser download (web)
pub fn save_bytes(name: &str, bytes: &[u8], mime: &str) -> Result<Option<String>, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = mime;
        let ext = name.rsplit('.').next().unwrap_or("");
        let Some(path) = rfd::FileDialog::new().add_filter(ext.to_uppercase(), &[ext]).set_file_name(name).set_title("Save").save_file() else {
            return Ok(None);
        };
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
        Ok(Some(path.display().to_string()))
    }
    #[cfg(target_arch = "wasm32")]
    {
        web_download(name, bytes, mime)?;
        Ok(Some(name.to_string()))
    }
}

#[cfg(target_arch = "wasm32")]
fn web_download(name: &str, bytes: &[u8], mime: &str) -> Result<(), String> {
    use eframe::wasm_bindgen::JsCast as _;
    use eframe::web_sys;
    let arr = js_sys::Uint8Array::from(bytes);
    let parts = js_sys::Array::of1(&arr);
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type(mime);
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts).map_err(|e| format!("{e:?}"))?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(|e| format!("{e:?}"))?;
    let doc = web_sys::window().and_then(|w| w.document()).ok_or("no document")?;
    let a = doc.create_element("a").map_err(|e| format!("{e:?}"))?.dyn_into::<web_sys::HtmlAnchorElement>().map_err(|_| "no anchor")?;
    a.set_href(&url);
    a.set_download(name);
    a.click();
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(())
}

// ---------------------------------------------------------------- image artwork

/// a picture file → (data URL, width px, height px); big rasters are scaled down to 2400 px
pub fn image_data_url(name: &str, bytes: &[u8]) -> Result<(String, f64, f64), String> {
    let lower = name.to_lowercase();
    let svg = lower.ends_with(".svg") || bytes.iter().take(256).collect::<Vec<_>>().windows(4).any(|w| w == [&b'<', &b's', &b'v', &b'g']);
    let b64 = |b: &[u8]| base64::engine::general_purpose::STANDARD.encode(b);
    if svg {
        let d = sb_print::images::decode_bytes("image/svg+xml", bytes)?;
        let (w, h) = d.size();
        return Ok((format!("data:image/svg+xml;base64,{}", b64(bytes)), w, h));
    }
    let img = image::load_from_memory(bytes).map_err(|e| format!("not a picture ShirtBuilder can read ({e})"))?;
    let (w, h) = (img.width() as f64, img.height() as f64);
    const MAX: f64 = 2400.0;
    let jpeg = lower.ends_with(".jpg") || lower.ends_with(".jpeg");
    let mime = |n: &str| {
        if n.ends_with(".png") {
            "image/png"
        } else if n.ends_with(".webp") {
            "image/webp"
        } else if n.ends_with(".gif") {
            "image/gif"
        } else if jpeg {
            "image/jpeg"
        } else {
            "image/png"
        }
    };
    if w.max(h) <= MAX {
        return Ok((format!("data:{};base64,{}", mime(&lower), b64(bytes)), w, h));
    }
    let k = MAX / w.max(h);
    let small = img.resize((w * k).round() as u32, (h * k).round() as u32, image::imageops::FilterType::Lanczos3);
    let mut out = std::io::Cursor::new(Vec::new());
    let fmt = if jpeg { image::ImageFormat::Jpeg } else { image::ImageFormat::Png };
    let small = if jpeg { image::DynamicImage::ImageRgb8(small.to_rgb8()) } else { small };
    small.write_to(&mut out, fmt).map_err(|e| e.to_string())?;
    Ok((format!("data:{};base64,{}", if jpeg { "image/jpeg" } else { "image/png" }, b64(&out.into_inner())), small.width() as f64, small.height() as f64))
}

impl ShirtBuilderApp {
    /// run an export (from the top bar or the Export menu)
    pub(crate) fn export(&mut self, ctx: &egui::Context, e: Export, now: f64) {
        match e {
            Export::Project => self.export_web_project(now),
            Export::Snapshot => {
                self.viewer.request_snapshot(&self.doc);
                if self.viewer.snapshot_pending {
                    self.say("Rendering 3D snapshot…", now);
                } else {
                    self.say("The 3D snapshot needs the graphics card renderer", now);
                }
            }
            _ => {
                let (doc, unit, sa) = (self.doc.clone(), self.prefs.unit, self.prefs.seam_allowance);
                let g = garment(doc.garment);
                let label = format!("{} {}", g.spec.title, doc.garment.size.key());
                #[cfg(not(target_arch = "wasm32"))]
                {
                    // ask where first (dialogs belong on the UI thread), then render in the background
                    let (name, _, _) = match e {
                        Export::PrintPng { side, dpi } => (format!("shirtbuilder-{}-{}-{dpi:.0}dpi.png", doc.garment, side_name(side)), 0, 0),
                        Export::PrintSvg { side } => (format!("shirtbuilder-{}-{}.svg", doc.garment, side_name(side)), 0, 0),
                        _ => (format!("shirtbuilder-{}-cutlines.svg", doc.garment), 0, 0),
                    };
                    let ext = name.rsplit('.').next().unwrap_or("").to_string();
                    let Some(path) = rfd::FileDialog::new().add_filter(ext.to_uppercase(), &[ext.as_str()]).set_file_name(&name).set_title("Export").save_file() else { return };
                    self.say(format!("Exporting {}…", path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()), now);
                    let done = self.export_done.clone();
                    let ctx = ctx.clone();
                    std::thread::spawn(move || {
                        let msg = match render(&doc, e, unit, sa).and_then(|(_, bytes, _)| std::fs::write(&path, bytes).map_err(|e| e.to_string())) {
                            Ok(()) => format!("Exported {} · {label}", path.display()),
                            Err(err) => format!("Export failed: {err}"),
                        };
                        done.lock().unwrap().push(msg);
                        ctx.request_repaint();
                    });
                }
                #[cfg(target_arch = "wasm32")]
                {
                    let _ = ctx;
                    match render(&doc, e, unit, sa).and_then(|(name, bytes, mime)| save_bytes(&name, &bytes, mime)) {
                        Ok(_) => self.say(format!("Exported · {label}"), now),
                        Err(err) => self.say(format!("Export failed: {err}"), now),
                    }
                }
            }
        }
    }

    /// finished background exports and 3D snapshots
    pub(crate) fn poll_exports(&mut self, now: f64) {
        let msgs: Vec<String> = self.export_done.lock().unwrap().drain(..).collect();
        for m in msgs {
            self.say(m, now);
        }
        if let Some(r) = self.viewer.take_snapshot(&self.doc) {
            let name = format!("shirtbuilder-3d-{}.png", (now * 1000.0) as u64);
            // automation: SB_SNAPSHOT_DIR saves straight into a folder, no dialog
            #[cfg(not(target_arch = "wasm32"))]
            if let Ok(dir) = std::env::var("SB_SNAPSHOT_DIR") {
                let path = std::path::Path::new(&dir).join(&name);
                let msg = match r.and_then(|png| std::fs::write(&path, png).map_err(|e| e.to_string())) {
                    Ok(()) => format!("Saved 3D snapshot {}", path.display()),
                    Err(e) => format!("3D snapshot failed: {e}"),
                };
                self.say(msg, now);
                return;
            }
            match r.and_then(|png| save_bytes(&name, &png, "image/png")) {
                Ok(Some(p)) => self.say(format!("Saved 3D snapshot {p}"), now),
                Ok(None) => {}
                Err(e) => self.say(format!("3D snapshot failed: {e}"), now),
            }
        }
    }

    /// add a picture as artwork on `piece` (at `at`, else the piece's default spot)
    pub(crate) fn add_image_bytes(&mut self, now: f64, name: &str, bytes: &[u8], target: Option<(PieceId, Side, f64, f64)>) {
        let (src, iw, ih) = match image_data_url(name, bytes) {
            Ok(v) => v,
            Err(e) => {
                self.say(format!("Could not read that image: {e}"), now);
                return;
            }
        };
        let (piece, side) = target.map(|t| (t.0, t.1)).unwrap_or_else(|| self.target_piece());
        let (x, y) = target.map(|t| (t.2, t.3)).unwrap_or_else(|| self.default_spot(piece));
        let g = garment(self.doc.garment);
        let p = g.piece(piece).unwrap_or(g.front());
        let aspect = ih / iw.max(1.0);
        let want: f64 = if p.kind == PieceKind::Body { 24.0 } else { 10.0 };
        let mut w = want.min(p.w * 0.8);
        if w * aspect > p.h * 0.8 {
            w = p.h * 0.8 / aspect;
        }
        let stem = name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name);
        let mut l = Self::base_layer(if stem.is_empty() { "Image" } else { stem }, LayerType::Image, piece, side, x, y, w, w * aspect);
        l.src = Some(src);
        let id = l.id.clone();
        self.edit(now, None, |d| d.layers.push(l));
        self.selected = Some(id);
        self.side = side;
        self.tool = crate::app::Tool::Select;
    }

    /// pick a picture with the system dialog (desktop)
    pub(crate) fn pick_image(&mut self, now: f64, target: Option<(PieceId, Side, f64, f64)>) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(path) = rfd::FileDialog::new().add_filter("Pictures", &["png", "jpg", "jpeg", "webp", "gif", "bmp", "svg"]).set_title("Add image").pick_file() {
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                match std::fs::read(&path) {
                    Ok(b) => self.add_image_bytes(now, &name, &b, target),
                    Err(e) => self.say(format!("Couldn't read {name}: {e}"), now),
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = target;
            self.say("Drop a picture onto the page to add it", now);
        }
    }

    /// replace the picture of image layer `id`, keeping its width
    pub(crate) fn replace_image(&mut self, now: f64, id: &str) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = rfd::FileDialog::new().add_filter("Pictures", &["png", "jpg", "jpeg", "webp", "gif", "bmp", "svg"]).set_title("Replace image").pick_file() {
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            match std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| image_data_url(&name, &b)) {
                Ok((src, iw, ih)) => {
                    let id = id.to_string();
                    self.edit(now, None, |d| {
                        if let Some(l) = d.layers.iter_mut().find(|l| l.id == id) {
                            l.h = l.w * ih / iw.max(1.0);
                            l.src = Some(src);
                        }
                    });
                }
                Err(e) => self.say(format!("Could not read that image: {e}"), now),
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = id;
            self.say("Drop a picture onto the page to add it", now);
        }
    }
}

/// keep a text layer's box proportional to its content after an edit (web: `refitText`)
pub fn refit_text(l: &mut Layer) {
    let b = sb_print::text::layout(l);
    l.h = l.w * b.h / b.w;
}
