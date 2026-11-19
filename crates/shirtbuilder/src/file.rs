//! Design files (`.sb`): New, Open, Save and Save as — which file is being edited, whether the
//! design has unsaved changes, and asking before New / Open would throw those away.
//!
//! A `.sb` file is the whole design (pieces, artwork with its images embedded, fabric, scene) in
//! JSON with a small header (`sb_core::doc::save_sb`). Project `.json` files from the web app and
//! earlier versions still open; they come in as a new, not yet saved design.

use crate::app::{ShirtBuilderApp, VERSION};
use crate::style::P;
use crate::widgets::{button, BtnKind};
use egui::{Layout, RichText};
use sb_core::doc::{default_doc, load_design, save_sb, Doc, SB_EXT};
use std::path::{Path, PathBuf};

/// What happens once "Save changes?" has been answered.
#[derive(Clone, Debug)]
pub(crate) enum After {
    New,
    /// pick a file in the system dialog
    Open,
    OpenPath(PathBuf),
    /// a file that arrived as bytes only (web drop): name, contents
    OpenText(String, String),
}

/// The file behind the design on screen.
pub(crate) struct DocFile {
    /// the `.sb` file being edited; `None` until the design is first saved
    pub path: Option<PathBuf>,
    /// the design as last saved, opened or started: unsaved changes are differences from it
    pub baseline: Doc,
    /// New / Open waiting for the "Save changes?" answer
    pub ask: Option<After>,
    /// window title last sent
    title: String,
}

impl DocFile {
    /// On start-up: the file edited last time (remembered in the preferences) if it is still
    /// there; the restored design counts as unsaved where it differs from that file.
    pub fn restore(last_file: Option<&str>, doc: &Doc) -> DocFile {
        let from_disk = last_file.map(PathBuf::from).and_then(|p| {
            #[cfg(not(target_arch = "wasm32"))]
            {
                let text = std::fs::read_to_string(&p).ok()?;
                load_design(&text).ok().map(|d| (p, d))
            }
            #[cfg(target_arch = "wasm32")]
            {
                let _ = p;
                None
            }
        });
        match from_disk {
            Some((p, d)) => DocFile { path: Some(p), baseline: d, ask: None, title: String::new() },
            None => DocFile { path: None, baseline: default_doc(doc.garment), ask: None, title: String::new() },
        }
    }

    /// "Summer tee.sb", or "Untitled" before the first save
    pub fn name(&self) -> String {
        self.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Untitled".into())
    }
}

/// `path` with the `.sb` extension (added when the user typed a name without it)
fn with_sb_ext(path: PathBuf) -> PathBuf {
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case(SB_EXT)) {
        path
    } else {
        let mut s = path.into_os_string();
        s.push(format!(".{SB_EXT}"));
        PathBuf::from(s)
    }
}

/// write `text` to `path` without ever leaving a half-written file there
#[cfg(not(target_arch = "wasm32"))]
fn write_atomically(path: &Path, text: &str) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".part");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

impl ShirtBuilderApp {
    /// the design differs from the file it came from (or, never saved, from a fresh design)
    pub fn has_unsaved_changes(&self) -> bool {
        self.doc != self.file.baseline
    }

    /// name of the design file being edited ("Untitled" before the first save)
    pub fn file_name(&self) -> String {
        self.file.name()
    }

    // ------------------------------------------------------------------ actions (buttons / keys)

    /// New: a fresh design for the current garment (asks first if there are unsaved changes)
    pub fn request_new(&mut self, now: f64) {
        self.when_saved_or_discarded(After::New, now);
    }

    /// Open…: pick a `.sb` (or web-app `.json`) file (asks first if there are unsaved changes)
    pub(crate) fn request_open(&mut self, now: f64) {
        self.when_saved_or_discarded(After::Open, now);
    }

    /// a design file dropped onto the window or passed on the command line
    pub fn request_open_path(&mut self, path: PathBuf, now: f64) {
        self.when_saved_or_discarded(After::OpenPath(path), now);
    }

    pub(crate) fn request_open_text(&mut self, name: String, text: String, now: f64) {
        self.when_saved_or_discarded(After::OpenText(name, text), now);
    }

    fn when_saved_or_discarded(&mut self, after: After, now: f64) {
        if self.has_unsaved_changes() {
            self.file.ask = Some(after);
        } else {
            self.proceed(after, now);
        }
    }

    fn proceed(&mut self, after: After, now: f64) {
        match after {
            After::New => self.new_design(now),
            After::Open => self.open_dialog(now),
            After::OpenPath(p) => {
                self.open_file(&p);
            }
            After::OpenText(name, text) => {
                self.open_design_text(&name, &text, None, now);
            }
        }
    }

    fn new_design(&mut self, now: f64) {
        let fresh = default_doc(self.doc.garment);
        let before = std::mem::replace(&mut self.doc, fresh.clone());
        self.history.record(&before, None, now);
        self.selected = None;
        self.file.path = None;
        self.file.baseline = fresh;
        self.prefs.last_file = None;
        self.say("New design — Ctrl+Z brings the previous one back", now);
    }

    /// Save: into the file being edited, or ask where on the first save. True when saved.
    pub fn save_design(&mut self, now: f64) -> bool {
        match self.file.path.clone() {
            Some(p) => self.write_design(p, now),
            None => self.save_design_as(now),
        }
    }

    /// Save as…: always ask for a name and place. True when saved.
    pub(crate) fn save_design_as(&mut self, now: f64) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let suggested = match &self.file.path {
                Some(p) => p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                None => format!("{}.{SB_EXT}", sb_core::pattern::garment(self.doc.garment).spec.title),
            };
            let mut dialog = rfd::FileDialog::new().add_filter("ShirtBuilder design", &[SB_EXT]).set_file_name(&suggested).set_title("Save design");
            if let Some(dir) = self.file.path.as_ref().and_then(|p| p.parent()) {
                dialog = dialog.set_directory(dir);
            }
            match dialog.save_file() {
                Some(p) => self.write_design(with_sb_ext(p), now),
                None => false,
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            // the browser downloads it; there is no file to keep writing to
            let text = save_sb(&self.doc, &format!("ShirtBuilder {VERSION}"));
            match crate::exports::save_bytes(&format!("design.{SB_EXT}"), text.as_bytes(), "application/x-shirtbuilder") {
                Ok(Some(_)) => {
                    self.file.baseline = self.doc.clone();
                    self.say("Design downloaded", now);
                    true
                }
                Ok(None) => false,
                Err(e) => {
                    self.say(format!("Couldn't save: {e}"), now);
                    false
                }
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn write_design(&mut self, path: PathBuf, now: f64) -> bool {
        let text = save_sb(&self.doc, &format!("ShirtBuilder {VERSION} (Rust edition)"));
        match write_atomically(&path, &text) {
            Ok(()) => {
                self.prefs.last_file = Some(path.display().to_string());
                self.file.path = Some(path);
                self.file.baseline = self.doc.clone();
                self.say(format!("Saved {}", self.file.name()), now);
                true
            }
            Err(e) => {
                self.say(format!("Couldn't save {}: {e}", path.display()), now);
                false
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn write_design(&mut self, _path: PathBuf, now: f64) -> bool {
        self.save_design_as(now)
    }

    fn open_dialog(&mut self, now: f64) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = now;
            let mut dialog = rfd::FileDialog::new()
                .add_filter("ShirtBuilder design", &[SB_EXT])
                .add_filter("Web app project", &["json"])
                .set_title("Open design");
            if let Some(dir) = self.file.path.as_ref().and_then(|p| p.parent()) {
                dialog = dialog.set_directory(dir);
            }
            if let Some(p) = dialog.pick_file() {
                self.open_file(&p);
            }
        }
        #[cfg(target_arch = "wasm32")]
        self.say("Drop a ShirtBuilder design (.sb) onto the page to open it", now);
    }

    /// Open a design file from disk right away (no question asked): a `.sb` becomes the file
    /// being edited; a project `.json` comes in as a new, not yet saved design. The design on
    /// screen before stays one Ctrl+Z away. True when it opened.
    pub fn open_file(&mut self, path: &Path) -> bool {
        let now = 0.0;
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        #[cfg(not(target_arch = "wasm32"))]
        match std::fs::read_to_string(path) {
            Ok(text) => self.open_design_text(&name, &text, Some(path.to_path_buf()), now),
            Err(e) => {
                self.say(format!("Couldn't read {name}: {e}"), now);
                false
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = name;
            false
        }
    }

    fn open_design_text(&mut self, name: &str, text: &str, path: Option<PathBuf>, now: f64) -> bool {
        match load_design(text) {
            Ok(d) => {
                let before = std::mem::replace(&mut self.doc, d.clone());
                if before != self.doc {
                    self.history.record(&before, None, now);
                }
                self.selected = None;
                let sb = path.as_ref().filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case(SB_EXT))).cloned();
                self.prefs.last_file = sb.as_ref().map(|p| p.display().to_string());
                self.file.path = sb;
                self.file.baseline = d;
                self.say(format!("Opened {name}"), now);
                true
            }
            Err(e) => {
                self.say(format!("Couldn't open {name}: {e}"), now);
                false
            }
        }
    }

    // ------------------------------------------------------------------ UI

    /// "Save changes?" — shown while New / Open waits on it. Save, Don't save, or Cancel
    /// (also Esc or a click outside).
    pub(crate) fn save_changes_dialog(&mut self, ctx: &egui::Context, now: f64) {
        let Some(after) = self.file.ask.clone() else { return };
        let mut open = true;
        let mut cancel = false;
        let mut choice: Option<bool> = None; // Some(true) save, Some(false) don't save
        let what = match after {
            After::New => "starting a new design",
            _ => "opening another design",
        };
        let title = format!("Save changes to {}?", self.file.name());
        crate::topbar::dialog(ctx, "save-changes", &title, None, 440.0, &mut open, |ui| {
            ui.label(RichText::new(format!("Your design has changes that aren't saved yet. Save them before {what}?")).color(P.text));
            ui.add_space(4.0);
            ui.label(RichText::new("If you don't save, Ctrl+Z can still bring them back until ShirtBuilder closes.").size(11.5).color(P.muted));
            ui.add_space(12.0);
            // one button-high row (a bare right-to-left layout would fill the dialog's height)
            ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 30.0), Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if button(ui, None, "Save", BtnKind::Primary, true).clicked() {
                    choice = Some(true);
                }
                if button(ui, None, "Don't save", BtnKind::Normal, true).clicked() {
                    choice = Some(false);
                }
                if button(ui, None, "Cancel", BtnKind::Normal, true).clicked() {
                    cancel = true;
                }
            });
        });
        match choice {
            Some(save) => {
                self.file.ask = None;
                // saving can be cancelled in the file dialog: then nothing else happens either
                if !save || self.save_design(now) {
                    self.proceed(after, now);
                }
            }
            None if !open || cancel => self.file.ask = None,
            None => {}
        }
    }

    /// window title (taskbar, window switcher): "Summer tee.sb • — ShirtBuilder" (• = unsaved)
    pub(crate) fn update_window_title(&mut self, ctx: &egui::Context) {
        let title = format!("{}{} — ShirtBuilder", self.file.name(), if self.has_unsaved_changes() { " •" } else { "" });
        if title != self.file.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.file.title = title;
        }
    }
}
