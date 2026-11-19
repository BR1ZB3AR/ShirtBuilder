//! App state, editing actions and the overall layout (same arrangement as the web app:
//! top bar · sewing-pattern pane | 3D pane · sidebar).

use crate::pattern_view::PatternView;
use crate::style::{self, P};
use egui::{Key, KeyboardShortcut, Modifiers};
use sb_core::doc::{load_doc, remap_layers, save_doc, Doc, Layer, LayerType, ShapeKind, Side};
use sb_core::history::History;
use sb_core::pattern::{garment, GarmentId, PieceId, PieceKind, Unit};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

const DOC_KEY: &str = "shirtbuilder.doc.v1";
const PREFS_KEY: &str = "shirtbuilder.ui.v1";

/// the words a new text layer starts with
pub const DEFAULT_TEXT: &str = "LET'S BUILD IT";

/// The design and preferences saved by the last session (or fresh ones). Once, a design still on
/// the old default colours (green shirt, white text) moves to the new ones (white shirt, black
/// text); colours chosen later are never touched again.
pub fn restore_saved(doc_json: Option<&str>, prefs_json: Option<&str>) -> (Doc, Prefs) {
    let mut doc = doc_json.and_then(|j| load_doc(j).ok()).unwrap_or_default();
    let mut prefs: Prefs = prefs_json.and_then(|j| serde_json::from_str(j).ok()).unwrap_or_default();
    if !prefs.colour_defaults_v2 {
        sb_core::doc::update_old_default_colours(&mut doc);
        prefs.colour_defaults_v2 = true;
    }
    (doc, prefs)
}
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    Pattern,
    Split,
    ThreeD,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tab {
    Design,
    Scene,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    Hand,
    Image,
    Text,
    Shape,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View3d {
    Front,
    Angle,
    Side,
    Back,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Quality {
    Auto,
    High,
    Balanced,
    Fast,
}

/// per-device preferences (not part of the design)
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub mode: Mode,
    pub tab: Tab,
    pub unit: Unit,
    pub seam_allowance: bool,
    pub grid: bool,
    pub quality: Quality,
    pub show_fps: bool,
    pub seen_version: String,
    /// theme used in "one theme" mode
    pub theme: String,
    /// follow the system's light / dark setting
    pub theme_system: bool,
    pub theme_light: String,
    pub theme_dark: String,
    /// custom accent overriding the theme's
    pub accent: Option<String>,
    /// the saved design was checked once for the old green / white-text defaults (see
    /// `sb_core::doc::update_old_default_colours`)
    pub colour_defaults_v2: bool,
    /// the .sb file being edited when the app last closed
    pub last_file: Option<String>,
}
impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            mode: Mode::Split,
            tab: Tab::Design,
            unit: Unit::Cm,
            seam_allowance: true,
            grid: false,
            quality: Quality::Auto,
            show_fps: false,
            seen_version: String::new(),
            theme: "forest-dark".into(),
            theme_system: false,
            theme_light: "daylight".into(),
            theme_dark: "forest-dark".into(),
            accent: None,
            colour_defaults_v2: false,
            last_file: None,
        }
    }
}

#[derive(Default)]
pub struct Popups {
    pub garment_top: bool,
    pub garment_float: bool,
    pub export: bool,
    pub theme: bool,
    pub changelog: bool,
    pub help: bool,
    /// theme gallery filters: Some(true) = dark only, Some(false) = light only
    pub theme_mode: Option<bool>,
    /// 0 standard, 1 liquid glass, 2 cyberpunk, 3 high contrast
    pub theme_kind: Option<u8>,
}

pub struct ShirtBuilderApp {
    pub(crate) doc: Doc,
    pub(crate) history: History<Doc>,
    pub(crate) view: PatternView,
    pub(crate) side: Side,
    pub(crate) prefs: Prefs,
    pub(crate) tool: Tool,
    pub(crate) selected: Option<String>,
    pub(crate) view3d: View3d,
    pub(crate) popups: Popups,
    pub(crate) status: Option<(String, f64)>,
    /// graphics card in use / found / chosen
    pub(crate) gpu: crate::gpu::GpuState,
    /// files read asynchronously (web) waiting to be opened (name, bytes)
    pending_files: Arc<Mutex<Vec<(String, Result<Vec<u8>, String>)>>>,
    /// the 3D preview (also renders the print atlases the pattern view shows)
    pub(crate) viewer: crate::scene3d::Viewer3d,
    /// messages from finished background exports
    pub(crate) export_done: Arc<Mutex<Vec<String>>>,
    /// eyedropper: waiting for a click (false) / for the screenshot of that click (true)
    pub(crate) picking: Option<(bool, egui::Pos2)>,
    pick_shot: Arc<Mutex<Option<Arc<egui::ColorImage>>>>,
    /// (theme id, accent) currently applied to the UI
    applied_theme: Option<(String, Option<String>)>,
    /// how compact the top bar was drawn last frame
    pub(crate) top_density: crate::topbar::Density,
    /// the .sb file behind the design, and its unsaved changes
    pub(crate) file: crate::file::DocFile,
}

impl ShirtBuilderApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        style::install_fonts(&cc.egui_ctx);
        egui_extras::install_image_loaders(&cc.egui_ctx);
        #[cfg(not(target_arch = "wasm32"))]
        if let Ok(d) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            sb_core::doc::seed_uids(d.as_millis() as u64);
        }
        // reopen the last design (autosaved) and preferences, or start fresh
        let (doc, mut prefs) = restore_saved(cc.storage.and_then(|s| s.get_string(DOC_KEY)).as_deref(), cc.storage.and_then(|s| s.get_string(PREFS_KEY)).as_deref());
        let file = crate::file::DocFile::restore(prefs.last_file.as_deref(), &doc);
        if file.path.is_none() {
            prefs.last_file = None; // moved or deleted since
        }
        ShirtBuilderApp {
            doc,
            history: History::default(),
            view: PatternView::default(),
            side: Side::Outside,
            prefs,
            tool: Tool::Select,
            selected: None,
            view3d: View3d::Front,
            popups: Popups::default(),
            status: None,
            gpu: crate::gpu::GpuState::new(cc.wgpu_render_state.as_ref()),
            pending_files: Default::default(),
            viewer: crate::scene3d::Viewer3d::new(&cc.egui_ctx, cc.wgpu_render_state.clone()),
            export_done: Default::default(),
            picking: None,
            pick_shot: Default::default(),
            applied_theme: None,
            top_density: crate::topbar::Density::Full,
            file,
        }
    }

    // ------------------------------------------------------------------ editing

    /// apply an edit as one undo step (or merged with the last one for the same `key`)
    pub(crate) fn edit(&mut self, now: f64, key: Option<&str>, f: impl FnOnce(&mut Doc)) {
        let before = self.doc.clone();
        f(&mut self.doc);
        if self.doc != before {
            self.history.record(&before, key, now);
        }
    }

    pub(crate) fn switch_garment(&mut self, now: f64, to: GarmentId) {
        if to == self.doc.garment {
            return;
        }
        self.edit(now, None, |d| {
            d.layers = remap_layers(&d.layers, d.garment, to);
            d.garment = to;
        });
        let g = garment(to);
        self.say(format!("Switched to {} · {}", g.spec.title, g.spec.subtitle), now);
    }

    pub(crate) fn say(&mut self, msg: impl Into<String>, now: f64) {
        self.status = Some((msg.into(), now));
    }

    pub(crate) fn undo(&mut self) {
        if let Some(d) = self.history.undo(&self.doc) {
            self.doc = d;
            self.fix_selection();
        }
    }
    pub(crate) fn redo(&mut self) {
        if let Some(d) = self.history.redo(&self.doc) {
            self.doc = d;
            self.fix_selection();
        }
    }
    fn fix_selection(&mut self) {
        if let Some(id) = &self.selected {
            if !self.doc.layers.iter().any(|l| &l.id == id) {
                self.selected = None;
            }
        }
    }

    pub(crate) fn selected_layer(&self) -> Option<&Layer> {
        self.selected.as_ref().and_then(|id| self.doc.layers.iter().find(|l| &l.id == id))
    }

    /// the piece new artwork goes on: the selected layer's piece, else the front
    pub(crate) fn target_piece(&self) -> (PieceId, Side) {
        self.selected_layer().map(|l| (l.piece, l.side)).unwrap_or((PieceId::Front, self.side))
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn base_layer(name: &str, kind: LayerType, piece: PieceId, side: Side, x: f64, y: f64, w: f64, h: f64) -> Layer {
        Layer {
            id: sb_core::doc::uid(),
            name: name.into(),
            kind,
            piece,
            side,
            x,
            y,
            w,
            h,
            rot: 0.0,
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

    /// default spot for new artwork on a piece (the web app's `defaultSpot`)
    pub(crate) fn default_spot(&self, piece: PieceId) -> (f64, f64) {
        let g = garment(self.doc.garment);
        let p = g.piece(piece).unwrap_or(g.front());
        match p.kind {
            PieceKind::Body => (p.w / 2.0, 24.0),
            PieceKind::Sleeve => (p.w / 2.0, p.h * 0.62),
            PieceKind::Collar => (p.w / 2.0, p.h * 0.65),
            PieceKind::Rib => (p.w / 2.0, p.h / 2.0),
        }
    }

    pub(crate) fn add_text(&mut self, now: f64, at: Option<(PieceId, Side, f64, f64)>) {
        let (piece, side) = at.map(|a| (a.0, a.1)).unwrap_or_else(|| self.target_piece());
        let (x, y) = at.map(|a| (a.2, a.3)).unwrap_or_else(|| self.default_spot(piece));
        let g = garment(self.doc.garment);
        let p = g.piece(piece).unwrap_or(g.front());
        let dark = sb_print::color::luminance(&self.doc.fabric.color) < 0.5;
        let mut l = Self::base_layer(DEFAULT_TEXT, LayerType::Text, piece, side, x, y, 10.0, 4.0);
        l.text = Some(DEFAULT_TEXT.into());
        l.font = Some("Bebas Neue".into());
        l.weight = Some(400.0);
        l.italic = Some(false);
        l.color = Some(if dark { "#ffffff" } else { "#000000" }.into());
        l.align = Some(sb_core::doc::Align::Center);
        l.line_height = Some(1.05);
        l.letter_spacing = Some(0.02);
        // fit the box to the text, inside the piece (web: fitWidth)
        let aspect = sb_print::text::text_aspect(&l);
        let want = match p.kind {
            PieceKind::Body => 22.0,
            PieceKind::Rib | PieceKind::Collar => p.w * 0.4,
            PieceKind::Sleeve => 12.0,
        };
        let mut w = want.min(p.w * 0.8);
        if w * aspect > p.h * 0.8 {
            w = p.h * 0.8 / aspect;
        }
        l.w = w;
        l.h = w * aspect;
        let id = l.id.clone();
        self.edit(now, None, |d| d.layers.push(l));
        self.selected = Some(id);
        self.side = side;
        self.tool = Tool::Select;
    }

    pub(crate) fn add_shape(&mut self, now: f64, at: Option<(PieceId, Side, f64, f64)>) {
        let (piece, side) = at.map(|a| (a.0, a.1)).unwrap_or_else(|| self.target_piece());
        let (x, y) = at.map(|a| (a.2, a.3)).unwrap_or_else(|| self.default_spot(piece));
        let g = garment(self.doc.garment);
        let p = g.piece(piece).unwrap_or(g.front());
        let s = 10f64.min(p.w * 0.5).min(p.h * 0.6);
        let mut l = Self::base_layer("Shape", LayerType::Shape, piece, side, x, y, s, s);
        l.shape = Some(ShapeKind::Rect);
        let dark = sb_print::color::luminance(&self.doc.fabric.color) < 0.5;
        l.fill = Some(if dark { "#f2efe6" } else { "#1b1b1b" }.into());
        l.radius = Some(0.0);
        let id = l.id.clone();
        self.edit(now, None, |d| d.layers.push(l));
        self.selected = Some(id);
        self.tool = Tool::Select;
    }

    pub(crate) fn remove_selected(&mut self, now: f64) {
        if let Some(id) = self.selected.take() {
            self.edit(now, None, |d| d.layers.retain(|l| l.id != id));
        }
    }

    /// copy a layer, nudged 2 cm, just above the original (Ctrl+D)
    pub(crate) fn duplicate_layer(&mut self, now: f64, id: &str) {
        let Some(i) = self.doc.layers.iter().position(|l| l.id == id) else { return };
        let mut copy = self.doc.layers[i].clone();
        copy.id = sb_core::doc::uid();
        copy.x += 2.0;
        copy.y += 2.0;
        let new_id = copy.id.clone();
        self.edit(now, None, |d| d.layers.insert(i + 1, copy));
        self.selected = Some(new_id);
    }

    pub(crate) fn move_layer(&mut self, now: f64, id: &str, dir: isize) {
        let id = id.to_string();
        self.edit(now, None, |d| {
            if let Some(i) = d.layers.iter().position(|l| l.id == id) {
                let j = i as isize + dir;
                if j >= 0 && (j as usize) < d.layers.len() {
                    d.layers.swap(i, j as usize);
                }
            }
        });
    }

    // ------------------------------------------------------------------ files

    /// open a project from its JSON text (as if the file had been opened); for tests and scripting
    pub fn open_json(&mut self, name: &str, json: &str) {
        self.open_text(name, json, 0.0);
    }

    /// how compact the top bar is drawn ("Full", "IconButtons", "Short" or "Tight"), for tests
    pub fn top_bar_density(&self) -> String {
        format!("{:?}", self.top_density)
    }

    /// the design on screen, for tests
    pub fn doc_for_tests(&self) -> &Doc {
        &self.doc
    }

    /// the selected layer's handles in the pattern view, for tests: (centre, corners, rotate knob)
    pub fn selected_handles_for_tests(&self) -> Option<(egui::Pos2, [egui::Pos2; 4], egui::Pos2)> {
        let l = self.selected_layer()?;
        let (cs, knob) = self.view.handles(&garment(self.doc.garment), l)?;
        let c = egui::pos2((cs[0].x + cs[2].x) / 2.0, (cs[0].y + cs[2].y) / 2.0);
        Some((c, cs, knob))
    }

    /// the selected layer, for tests
    pub fn selected_layer_for_tests(&self) -> Option<&Layer> {
        self.selected_layer()
    }

    /// place the 3D camera (tests): distance cm, polar and azimuth angles (rad), pan from the target
    pub fn set_camera_for_tests(&mut self, r: f64, phi: f64, theta: f64, pan: [f32; 3]) {
        self.viewer.set_camera_for_tests(r, phi, theta, pan);
    }

    /// 3D camera distance (cm), for tests
    pub fn camera_distance(&self) -> f64 {
        self.viewer.camera_distance()
    }

    /// switch the layout: "pattern", "split" or "3d"
    pub fn set_layout(&mut self, layout: &str) {
        self.prefs.mode = match layout {
            "pattern" => Mode::Pattern,
            "3d" => Mode::ThreeD,
            _ => Mode::Split,
        };
    }

    pub(crate) fn open_text(&mut self, name: &str, text: &str, now: f64) {
        match load_doc(text) {
            Ok(d) => {
                let before = std::mem::replace(&mut self.doc, d);
                self.history.record(&before, None, now);
                self.selected = None;
                self.say(format!("Opened {name}"), now);
            }
            Err(e) => self.say(format!("Couldn't open {name}: {e}"), now),
        }
    }

    /// Export the design as a web-app project .json (system dialog on the desktop). The design
    /// file of this app is the .sb (file.rs); this is only for opening it in the web app.
    pub(crate) fn export_web_project(&mut self, now: f64) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let default = format!("shirtbuilder-{}.json", self.doc.garment);
            if let Some(path) = rfd::FileDialog::new().add_filter("Web app project", &["json"]).set_file_name(&default).set_title("Export web app project").save_file() {
                match std::fs::write(&path, save_doc(&self.doc)) {
                    Ok(()) => self.say(format!("Exported {}", path.display()), now),
                    Err(e) => self.say(format!("Couldn't save: {e}"), now),
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        self.say("Saving from the web build comes with the export stage", now);
    }

    /// a project .json dropped onto the window (web-app files open too). Native reads it right away;
    /// the browser reads files asynchronously, so those arrive through `pending_files`.
    fn dropped_files(&mut self, ctx: &egui::Context, now: f64) {
        let files = ctx.input(|i| i.raw.dropped_files.clone());
        for f in files {
            let name = f.path().file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "file".into());
            // a design file on disk opens from its path, so saving goes back into it
            #[cfg(not(target_arch = "wasm32"))]
            if is_design_file(&name) && f.path().is_file() {
                self.request_open_path(f.path().to_path_buf(), now);
                continue;
            }
            match f.bytes() {
                Ok(bytes) => self.open_dropped(&name, bytes.to_vec(), now),
                #[cfg(target_arch = "wasm32")]
                Err(_) => {
                    let queue = self.pending_files.clone();
                    let ctx = ctx.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        let r = f.bytes_async().await.map(|b| b.to_vec()).map_err(|e| e.to_string());
                        queue.lock().unwrap().push((name, r));
                        ctx.request_repaint();
                    });
                }
                #[cfg(not(target_arch = "wasm32"))]
                Err(e) => self.say(format!("Couldn't read {name}: {e}"), now),
            }
        }
        let arrived: Vec<_> = self.pending_files.lock().unwrap().drain(..).collect();
        for (name, r) in arrived {
            match r {
                Ok(bytes) => self.open_dropped(&name, bytes, now),
                Err(e) => self.say(format!("Couldn't read {name}: {e}"), now),
            }
        }
    }

    /// a dropped project opens; a dropped picture becomes artwork on the current piece
    fn open_dropped(&mut self, name: &str, bytes: Vec<u8>, now: f64) {
        if is_design_file(name) {
            self.request_open_text(name.to_string(), String::from_utf8_lossy(&bytes).into_owned(), now);
        } else {
            self.add_image_bytes(now, name, &bytes, None);
        }
    }

    // ------------------------------------------------------------------ keyboard

    fn shortcuts(&mut self, ctx: &egui::Context, now: f64) {
        let sc = |m: Modifiers, k: Key| KeyboardShortcut::new(m, k);
        let (redo, redo2, undo, open, save_as, save, new) = ctx.input_mut(|i| {
            (
                i.consume_shortcut(&sc(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z)),
                i.consume_shortcut(&sc(Modifiers::COMMAND, Key::Y)),
                i.consume_shortcut(&sc(Modifiers::COMMAND, Key::Z)),
                i.consume_shortcut(&sc(Modifiers::COMMAND, Key::O)),
                i.consume_shortcut(&sc(Modifiers::COMMAND | Modifiers::SHIFT, Key::S)),
                i.consume_shortcut(&sc(Modifiers::COMMAND, Key::S)),
                i.consume_shortcut(&sc(Modifiers::COMMAND, Key::N)),
            )
        });
        let dup = ctx.input_mut(|i| i.consume_shortcut(&sc(Modifiers::COMMAND, Key::D)));
        if dup {
            if let Some(id) = self.selected.clone() {
                self.duplicate_layer(now, &id);
            }
        }
        if redo || redo2 {
            self.redo();
        } else if undo {
            self.undo();
        }
        if open {
            self.request_open(now);
        }
        if save_as {
            self.save_design_as(now);
        } else if save {
            self.save_design(now);
        }
        if new {
            self.request_new(now);
        }
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let pressed = |k: Key| ctx.input(|i| i.key_pressed(k) && !i.modifiers.command);
        if pressed(Key::Delete) || pressed(Key::Backspace) {
            self.remove_selected(now);
        }
        if pressed(Key::Escape) {
            self.selected = None;
            self.tool = Tool::Select;
            self.popups = Popups::default();
        }
        for (k, t) in [(Key::V, Tool::Select), (Key::H, Tool::Hand), (Key::T, Tool::Text), (Key::R, Tool::Shape), (Key::I, Tool::Image)] {
            if pressed(k) {
                self.tool = t;
            }
        }
        if pressed(Key::F) {
            self.view.request_fit();
        }
        for (k, m) in [(Key::Num1, Mode::Pattern), (Key::Num2, Mode::Split), (Key::Num3, Mode::ThreeD)] {
            if pressed(k) {
                self.prefs.mode = m;
            }
        }
        if ctx.input(|i| i.key_pressed(Key::Questionmark)) {
            self.popups.help = true;
        }
        // arrow keys nudge the selected layer (1 mm, Shift = 1 cm)
        if let Some(id) = self.selected.clone() {
            let step = if ctx.input(|i| i.modifiers.shift) { 1.0 } else { 0.1 };
            let (dx, dy) = ctx.input(|i| {
                let mut d = (0.0, 0.0);
                if i.key_pressed(Key::ArrowLeft) {
                    d.0 -= step;
                }
                if i.key_pressed(Key::ArrowRight) {
                    d.0 += step;
                }
                if i.key_pressed(Key::ArrowUp) {
                    d.1 -= step;
                }
                if i.key_pressed(Key::ArrowDown) {
                    d.1 += step;
                }
                d
            });
            if dx != 0.0 || dy != 0.0 {
                let key = format!("nudge-{id}");
                self.edit(now, Some(&key), |d| {
                    if let Some(l) = d.layers.iter_mut().find(|l| l.id == id) {
                        l.x += dx;
                        l.y += dy;
                    }
                });
            }
        }
    }
}

impl ShirtBuilderApp {
    /// Eyedropper (Fabric › pipette): the next click anywhere in the window picks that pixel's
    /// colour as the fabric colour. A transparent overlay catches the click so nothing under it
    /// reacts; the colour comes from a screenshot of that frame.
    pub(crate) fn eyedropper(&mut self, ctx: &egui::Context, now: f64) {
        match self.picking {
            Some((false, _)) => {
                if ctx.input(|i| i.key_pressed(Key::Escape)) {
                    self.picking = None;
                    return;
                }
                let screen = ctx.content_rect();
                let resp = egui::Area::new(egui::Id::unique("eyedropper"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(screen.min)
                    .show(ctx, |ui| {
                        let r = ui.allocate_rect(screen, egui::Sense::click());
                        let tip = "Click anywhere to pick a fabric colour · Esc cancels";
                        let g = ui.painter().layout_no_wrap(tip.into(), egui::FontId::proportional(12.0), egui::Color32::WHITE);
                        let at = screen.center_top() + egui::vec2(0.0, 70.0);
                        let bg = egui::Rect::from_center_size(at, g.size() + egui::vec2(20.0, 10.0));
                        ui.painter().rect_filled(bg, 12.0, P.chip);
                        ui.painter().galley(bg.min + egui::vec2(10.0, 5.0), g, egui::Color32::WHITE);
                        r
                    })
                    .inner;
                ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
                if resp.clicked() {
                    if let Some(pos) = resp.interact_pointer_pos() {
                        self.picking = Some((true, pos));
                        let slot = self.pick_shot.clone();
                        let c2 = ctx.clone();
                        *slot.lock().unwrap() = None;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::ScreenshotCallback::new(move |img| {
                            *slot.lock().unwrap() = Some(img);
                            c2.request_repaint();
                        })));
                    }
                }
            }
            Some((true, pos)) => {
                let shot = self.pick_shot.lock().unwrap().take();
                if let Some(img) = shot {
                    let ppp = ctx.pixels_per_point();
                    let (x, y) = ((pos.x * ppp) as usize, (pos.y * ppp) as usize);
                    let [w, h] = img.size;
                    if x < w && y < h {
                        let c = img.pixels[y * w + x];
                        let hex = format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b());
                        self.edit(now, None, |d| d.fabric.color = hex.clone());
                        self.say(format!("Fabric colour {hex}"), now);
                    }
                    self.picking = None;
                } else {
                    ctx.request_repaint();
                }
            }
            None => {}
        }
    }
}

impl ShirtBuilderApp {
    /// the theme in force: the chosen one, or the light / dark slot when following the system
    pub(crate) fn active_theme_id(&self, ctx: &egui::Context) -> String {
        if self.prefs.theme_system {
            let dark = ctx.system_theme().map(|t| t == egui::Theme::Dark).unwrap_or(true);
            if dark {
                self.prefs.theme_dark.clone()
            } else {
                self.prefs.theme_light.clone()
            }
        } else {
            self.prefs.theme.clone()
        }
    }

    fn apply_theme(&mut self, ctx: &egui::Context) {
        let id = self.active_theme_id(ctx);
        let th = crate::themes::theme(&id);
        // glass, neon and high-contrast themes keep their own accent
        let accent = self.prefs.accent.clone().filter(|_| !th.hc && th.fx == style::Fx::None);
        let key = (id, accent);
        if self.applied_theme.as_ref() != Some(&key) {
            style::apply(ctx, &crate::themes::palette(th, key.1.as_deref()));
            self.applied_theme = Some(key);
        }
    }
}

impl eframe::App for ShirtBuilderApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        self.shortcuts(&ctx, now);
        self.dropped_files(&ctx, now);
        // geometry and print atlases (the pattern view shows the atlas too)
        self.viewer.sync(&self.doc);
        self.poll_exports(now);
        self.eyedropper(&ctx, now);
        self.apply_theme(&ctx);
        let pal = *P;
        let th = crate::themes::theme(&self.applied_theme.as_ref().map(|a| a.0.clone()).unwrap_or_default());
        if pal.fx != style::Fx::None {
            // glass / cyber: the glowing background shows through translucent panels
            crate::themes::paint_app_background(&ctx.layer_painter(egui::LayerId::background()), ctx.content_rect(), th, &pal);
        }

        egui::Panel::top("topbar")
            .exact_size(52.0)
            .frame(egui::Frame::NONE.fill(pal.panel).inner_margin(egui::Margin::symmetric(12, 0)).stroke(egui::Stroke::new(1.0, pal.border)))
            .show(ui, |ui| self.top_bar(ui, now));
        egui::Panel::right("sidebar")
            .exact_size(304.0)
            .resizable(false)
            .frame(egui::Frame::NONE.fill(pal.panel).stroke(egui::Stroke::new(1.0, pal.border)))
            .show(ui, |ui| self.sidebar(ui, now));
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(if pal.fx == style::Fx::None { pal.bg } else { egui::Color32::TRANSPARENT })).show(ui, |ui| {
            let full = ui.max_rect();
            match self.prefs.mode {
                Mode::Pattern => self.pattern_pane(ui, full, now),
                Mode::ThreeD => self.viewer_pane(ui, full, now),
                Mode::Split => {
                    let mid = full.center().x.round();
                    let left = egui::Rect::from_min_max(full.min, egui::pos2(mid, full.max.y));
                    let right = egui::Rect::from_min_max(egui::pos2(mid, full.min.y), full.max);
                    self.pattern_pane(ui, left, now);
                    self.viewer_pane(ui, right, now);
                    ui.painter().vline(mid, full.y_range(), egui::Stroke::new(1.0, pal.border));
                }
            }
        });
        self.windows(&ctx, now);
        self.save_changes_dialog(&ctx, now);
        self.update_window_title(&ctx);
        if crate::titlebar::ENABLED {
            crate::titlebar::window_outline(&ctx);
            crate::titlebar::resize_zones(ui);
        }
        if self.status.as_ref().is_some_and(|(_, t)| now - t < 3.0) {
            ctx.request_repaint_after(std::time::Duration::from_millis(300));
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(DOC_KEY, save_doc(&self.doc));
        if let Ok(j) = serde_json::to_string(&self.prefs) {
            storage.set_string(PREFS_KEY, j);
        }
    }
}

/// a design file ShirtBuilder opens: its own .sb, or a project .json (web app / earlier versions)
fn is_design_file(name: &str) -> bool {
    let n = name.to_lowercase();
    n.ends_with(&format!(".{}", sb_core::doc::SB_EXT)) || n.ends_with(".json")
}
