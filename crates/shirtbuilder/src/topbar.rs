//! Top bar (same arrangement as the web app), pop-up menus and the changelog / help windows.

use crate::app::{Mode, ShirtBuilderApp, VERSION};
use crate::exports::Export;
use sb_core::doc::Side;
use crate::style::{icon, icons, semibold, P};
use crate::widgets::{button, button_width, icon_button, icon_button_framed, seg, seg_size, BtnKind, ICON_BUTTON_FRAMED_W};
use egui::{pos2, vec2, Align, Align2, Color32, CornerRadius, FontId, Layout, Rect, RichText, Sense, Stroke, StrokeKind, Ui, UiBuilder};
use lucide_icons::Icon;
use sb_core::pattern::{garment, GarmentId, FITS, SIZES, SLEEVES, STYLES};

const LOGO: &[u8] = include_bytes!("../assets/logo-mark-64.png");

/// a pop-up anchored under `anchor`; closes when clicking elsewhere
pub(crate) fn popup(ctx: &egui::Context, id: &str, anchor: Rect, right: bool, open: &mut bool, width: f32, add: impl FnOnce(&mut Ui)) {
    if !*open {
        return;
    }
    let (pos, pivot) = if right { (anchor.right_bottom() + vec2(0.0, 6.0), Align2::RIGHT_TOP) } else { (anchor.left_bottom() + vec2(0.0, 6.0), Align2::LEFT_TOP) };
    let area = egui::Area::new(egui::Id::unique(id)).order(egui::Order::Foreground).fixed_pos(pos).pivot(pivot).show(ctx, |ui| {
        egui::Frame::NONE
            .fill(P.menu)
            .stroke(Stroke::new(1.0, P.border))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(egui::Margin::same(8))
            .shadow(egui::Shadow { offset: [0, 10], blur: 30, spread: 0, color: Color32::from_black_alpha(115) })
            .show(ui, |ui| {
                ui.set_width(width);
                add(ui);
            });
    });
    let outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !area.response.rect.contains(p) && !anchor.contains(p)));
    if outside {
        *open = false;
    }
}

/// How compact the top bar is drawn — the least compact one that fits the window wins:
/// `Full` everything labelled · `IconButtons` Theme / Export SVG / 3D snapshot become icons ·
/// `Short` + "Pattern" instead of "Sewing pattern" and no "ShirtBuilder" wordmark ·
/// `Tight` + the garment chip becomes a shirt icon and the divider before Help goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Density {
    Full,
    IconButtons,
    Short,
    Tight,
}

/// gap kept between the three groups of the bar
const GROUP_GAP: f32 = 12.0;

fn mode_options(d: Density) -> [(Mode, &'static str); 3] {
    [(Mode::Pattern, if d >= Density::Short { "Pattern" } else { "Sewing pattern" }), (Mode::Split, "Split"), (Mode::ThreeD, "3D")]
}

fn text_w(ui: &Ui, text: &str, font: FontId) -> f32 {
    ui.fonts_mut(|f| f.layout_no_wrap(text.to_string(), font, P.text).size().x)
}

/// the garment chip's two lines
fn garment_lines(id: GarmentId) -> (String, String) {
    let g = garment(id);
    (g.spec.title.to_uppercase(), g.spec.subtitle.to_uppercase())
}

/// widths of the left group, the right group and the layout switcher at density `d`
/// (mirrors the drawing code in `top_bar`)
fn bar_widths(ui: &Ui, d: Density, garment_id: GarmentId) -> (f32, f32, f32) {
    let sx = ui.spacing().item_spacing.x;
    // left: logo · [wordmark] · version badge · 10 · garment chip
    let mut left = vec![28.0];
    if d < Density::Short {
        left.push(text_w(ui, "ShirtBuilder", FontId::new(17.0, semibold())));
    }
    left.push(text_w(ui, &format!("v{VERSION}"), FontId::proportional(11.0)) + 12.0);
    let (title, sub) = garment_lines(garment_id);
    let chip = if d < Density::Tight { text_w(ui, &title, FontId::new(11.0, semibold())).max(text_w(ui, &sub, FontId::proportional(9.5))) + 12.0 } else { 32.0 };
    left.push(chip);
    let left_w = left.iter().sum::<f32>() + sx * (left.len() as f32 - 1.0) + 10.0;
    // right (6 px apart): caret · Export · snapshot · SVG · Theme · 4 · save · open · new · 4 · redo · undo · [divider] · help
    let compact = d >= Density::IconButtons;
    let framed = |ic: bool, text: &str| if compact { ICON_BUTTON_FRAMED_W } else { button_width(ui, ic, text) };
    let mut right = vec![button_width(ui, true, ""), button_width(ui, false, "Export"), framed(false, "3D snapshot"), framed(true, "Export SVG"), framed(true, "Theme"), 30.0, 30.0, 30.0, 30.0, 30.0];
    if d < Density::Tight {
        right.push(10.0);
    }
    right.push(30.0);
    let right_w = right.iter().sum::<f32>() + 6.0 * (right.len() as f32 - 1.0) + 4.0 + 4.0;
    let switch_w = seg_size(ui, &mode_options(d), false).x;
    (left_w, right_w, switch_w)
}

fn menu_label(ui: &mut Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(text.to_uppercase()).size(10.5).color(P.muted));
}

/// a menu row: icon, title, small subtitle
fn menu_row(ui: &mut Ui, ic: Icon, title: &str, sub: &str, on: bool, enabled: bool) -> bool {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, if sub.is_empty() { 30.0 } else { 40.0 }), if enabled { Sense::click() } else { Sense::hover() });
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, enabled, on, title));
    if on || (enabled && resp.hovered()) {
        ui.painter().rect_filled(rect, CornerRadius::same(7), P.panel2);
    }
    let fg = if enabled { P.text } else { P.muted };
    ui.painter().text(rect.left_center() + vec2(10.0, 0.0), Align2::LEFT_CENTER, icon(ic), FontId::new(15.0, icons()), fg);
    if sub.is_empty() {
        ui.painter().text(rect.left_center() + vec2(34.0, 0.0), Align2::LEFT_CENTER, title, FontId::proportional(13.0), fg);
    } else {
        ui.painter().text(pos2(rect.left() + 34.0, rect.top() + 7.0), Align2::LEFT_TOP, title, FontId::new(13.0, semibold()), fg);
        ui.painter().text(pos2(rect.left() + 34.0, rect.bottom() - 7.0), Align2::LEFT_BOTTOM, sub, FontId::proportional(11.0), P.muted);
    }
    enabled && resp.clicked()
}

impl ShirtBuilderApp {
    /// the garment picker contents (top bar and the floating one in the 3D pane)
    pub(crate) fn garment_menu(&mut self, ui: &mut Ui, now: f64) {
        let id = self.doc.garment;
        let mut next = id;
        menu_label(ui, "Garment");
        for (s, name, hint) in STYLES {
            if menu_row(ui, Icon::Shirt, name, hint, s == id.style, true) {
                next.style = s;
            }
        }
        menu_label(ui, "Sleeve");
        let opts: Vec<_> = SLEEVES.iter().map(|(s, n)| (*s, *n)).collect();
        seg(ui, &mut next.sleeve, &opts, true);
        menu_label(ui, "Fit");
        let opts: Vec<_> = FITS.iter().map(|(s, n)| (*s, *n)).collect();
        seg(ui, &mut next.fit, &opts, true);
        let g = garment(id);
        ui.horizontal(|ui| {
            menu_label(ui, "Size");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(format!("chest {:.0} cm · length {:.0} cm", g.spec.body_w * 2.0, g.spec.body_h)).size(11.0).color(P.muted));
            });
        });
        let opts: Vec<_> = SIZES.iter().map(|s| (*s, s.key())).collect();
        seg(ui, &mut next.size, &opts, true);
        if next != id {
            self.switch_garment(now, GarmentId { ..next });
        }
    }

    pub(crate) fn top_bar(&mut self, ui: &mut Ui, now: f64) {
        let mut full = ui.max_rect();
        let ctx = ui.ctx().clone();
        // desktop: the bar is the window's title bar (drag to move, caption buttons at the right)
        if crate::titlebar::ENABLED {
            let win = ctx.content_rect();
            let bar = Rect::from_min_max(win.left_top(), pos2(win.right(), win.top() + 51.0)); // above the bar's bottom border
            crate::titlebar::drag_area(ui, bar);
            crate::titlebar::caption_buttons(ui, bar);
            full.max.x = bar.right() - crate::titlebar::WIDTH - 10.0;
        }

        // how compact the bar has to be to fit (Density docs)
        let density = [Density::Full, Density::IconButtons, Density::Short, Density::Tight]
            .into_iter()
            .find(|&d| {
                let (l, r, s) = bar_widths(ui, d, self.doc.garment);
                l + r + s + 2.0 * GROUP_GAP <= full.width()
            })
            .unwrap_or(Density::Tight);
        self.top_density = density;

        // ---- left: logo · name · version · garment
        let mut garment_anchor = Rect::NOTHING;
        let left_end = ui.scope_builder(UiBuilder::new().max_rect(full).layout(Layout::left_to_right(Align::Center)), |ui| {
            ui.add(egui::Image::from_bytes("bytes://logo-mark.png", LOGO).fit_to_exact_size(vec2(28.0, 28.0)).corner_radius(7));
            if density < Density::Short {
                ui.label(RichText::new("ShirtBuilder").family(semibold()).size(17.0));
            }
            // version badge → changelog
            let unseen = self.prefs.seen_version != VERSION;
            let label = format!("v{VERSION}");
            let galley = ui.painter().layout_no_wrap(label, FontId::proportional(11.0), P.muted);
            let (r, resp) = ui.allocate_exact_size(galley.size() + vec2(12.0, 4.0), Sense::click());
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, format!("Version {VERSION} — changelog")));
            let hovered = resp.hovered();
            ui.painter().rect(r, CornerRadius::same(5), if hovered { P.accent_weak } else { Color32::TRANSPARENT }, Stroke::new(1.0, if hovered { P.accent } else { P.border }), StrokeKind::Inside);
            ui.painter().galley(r.min + vec2(6.0, 2.0), galley, if hovered { P.text } else { P.muted });
            if unseen {
                ui.painter().circle(r.right_top() + vec2(-1.0, 1.0), 3.5, P.accent, Stroke::new(2.0, P.panel));
            }
            if resp.on_hover_text(if unseen { "What's new — view changelog" } else { "View changelog" }).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                self.popups.changelog = true;
                self.prefs.seen_version = VERSION.to_string();
            }
            ui.add_space(10.0);
            // compact garment picker (just a shirt icon when the bar is tight)
            let (title_s, sub_s) = garment_lines(self.doc.garment);
            let resp = if density < Density::Tight {
                let title = ui.painter().layout_no_wrap(title_s, FontId::new(11.0, semibold()), P.muted);
                let sub = ui.painter().layout_no_wrap(sub_s, FontId::proportional(9.5), P.accent);
                let w = title.size().x.max(sub.size().x) + 12.0;
                let (r, resp) = ui.allocate_exact_size(vec2(w, 34.0), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(r, CornerRadius::same(8), P.panel2);
                }
                let (th, sh) = (title.size().y, sub.size().y);
                let top = r.center().y - (th + sh) / 2.0;
                ui.painter().galley(pos2(r.left() + 6.0, top), title, P.muted);
                ui.painter().galley(pos2(r.left() + 6.0, top + th), sub, P.accent);
                resp
            } else {
                let (r, resp) = ui.allocate_exact_size(vec2(32.0, 34.0), Sense::click());
                ui.painter().rect_filled(r, CornerRadius::same(8), if resp.hovered() { P.panel2 } else { Color32::TRANSPARENT });
                ui.painter().text(r.center(), Align2::CENTER_CENTER, icon(Icon::Shirt), FontId::new(17.0, icons()), P.accent);
                resp.on_hover_text(format!("{title_s}\n{sub_s}"))
            };
            garment_anchor = resp.rect;
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Choose garment"));
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                self.popups.garment_top = !self.popups.garment_top;
            }
        })
        .response
        .rect
        .right();

        // ---- right: help · undo · redo · open · save | Theme · Export SVG · 3D snapshot · Export ▾
        let mut export_anchor = Rect::NOTHING;
        let mut theme_anchor = Rect::NOTHING;
        let right_start = ui.scope_builder(UiBuilder::new().max_rect(full).layout(Layout::right_to_left(Align::Center)), |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let caret = button(ui, Some(Icon::ChevronDown), "", BtnKind::Primary, true).on_hover_text("More export options");
            let exp = button(ui, None, "Export", BtnKind::Primary, true).on_hover_text("Print sheet · outside (PNG, 100 dpi)");
            export_anchor = exp.rect.union(caret.rect);
            if caret.clicked() {
                self.popups.export = !self.popups.export;
            }
            if exp.clicked() {
                self.export(&ctx, Export::PrintPng { side: Side::Outside, dpi: 100.0 }, now);
            }
            // labelled buttons, or just their icons when the bar is narrow (named the same either way)
            let compact = density >= Density::IconButtons;
            let labelled = |ui: &mut Ui, ic: Icon, show_icon: bool, name: &str, enabled: bool| {
                if compact {
                    icon_button_framed(ui, ic, name, BtnKind::Normal, enabled)
                } else {
                    button(ui, show_icon.then_some(ic), name, BtnKind::Normal, enabled)
                }
            };
            if labelled(ui, Icon::Camera, false, "3D snapshot", self.viewer.gpu_ok).on_hover_text("PNG, 2000 px, with the backdrop").clicked() {
                self.export(&ctx, Export::Snapshot, now);
            }
            if labelled(ui, Icon::File, true, "Export SVG", true).on_hover_text("Export the sewing pattern as a true-scale SVG (artwork + cut lines)").clicked() {
                self.export(&ctx, Export::PrintSvg { side: Side::Outside }, now);
            }
            let th = labelled(ui, Icon::Palette, true, "Theme", true);
            theme_anchor = th.rect;
            if th.clicked() {
                self.popups.theme = !self.popups.theme;
            }
            ui.add_space(4.0);
            // design file: New · Open · Save (a dot on Save = unsaved changes)
            let unsaved = self.has_unsaved_changes();
            let save = icon_button(ui, Icon::Save, "Save (Ctrl+S)", true);
            if unsaved {
                ui.painter().circle(save.rect.right_top() + vec2(-6.0, 6.0), 3.5, P.accent, Stroke::new(2.0, P.panel));
            }
            let save_tip = match (unsaved, &self.file.path) {
                (true, Some(_)) => format!("Save changes to {}", self.file.name()),
                (true, None) => "Save this design as a .sb file".to_string(),
                (false, Some(_)) => format!("All changes saved in {}", self.file.name()),
                (false, None) => "Save this design as a .sb file".to_string(),
            };
            if save.on_hover_text(save_tip).clicked() {
                self.save_design(now);
            }
            if icon_button(ui, Icon::FolderOpen, "Open design (Ctrl+O)", true).on_hover_text(".sb design, or a web app project .json").clicked() {
                self.request_open(now);
            }
            if icon_button(ui, Icon::FilePlus, "New design (Ctrl+N)", true).clicked() {
                self.request_new(now);
            }
            ui.add_space(4.0);
            if icon_button(ui, Icon::Redo2, "Redo (Ctrl+Shift+Z)", self.history.can_redo()).clicked() {
                self.redo();
            }
            if icon_button(ui, Icon::Undo2, "Undo (Ctrl+Z)", self.history.can_undo()).clicked() {
                self.undo();
            }
            if density < Density::Tight {
                ui.add(egui::Separator::default().vertical().spacing(10.0));
            }
            if icon_button(ui, Icon::CircleQuestionMark, "Shortcuts & help (?)", true).clicked() {
                self.popups.help = true;
            }
        })
        .response
        .rect
        .left();

        // ---- centre: Sewing pattern · Split · 3D — centred on the bar, or in the gap between the
        // two button groups when the window is too narrow for that
        let modes = mode_options(density);
        let size = seg_size(ui, &modes, false);
        let mut cx = full.center().x;
        if cx - size.x / 2.0 < left_end + GROUP_GAP || cx + size.x / 2.0 > right_start - GROUP_GAP {
            cx = (left_end + right_start) / 2.0;
        }
        let c = Rect::from_center_size(pos2(cx, full.center().y), size);
        ui.scope_builder(UiBuilder::new().max_rect(c).layout(Layout::left_to_right(Align::Center)), |ui| {
            seg(ui, &mut self.prefs.mode, &modes, false);
        });

        // ---- pop-ups
        let mut open = self.popups.garment_top;
        popup(&ctx, "garment-top", garment_anchor, false, &mut open, 260.0, |ui| self.garment_menu(ui, now));
        self.popups.garment_top = open;

        let mut open = self.popups.export;
        let mut chosen: Option<Export> = None;
        let mut save_as = false;
        let mut sa = self.prefs.seam_allowance;
        popup(&ctx, "export-menu", export_anchor, true, &mut open, 300.0, |ui| {
            // seam & hem allowance switch
            let w = ui.available_width();
            let (r, resp) = ui.allocate_exact_size(vec2(w, 40.0), Sense::click());
            resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, sa, "Seam & hem allowances"));
            if resp.hovered() {
                ui.painter().rect_filled(r, CornerRadius::same(7), P.panel2);
            }
            ui.painter().text(pos2(r.left() + 10.0, r.top() + 7.0), Align2::LEFT_TOP, "Seam & hem allowances", FontId::new(13.0, semibold()), P.text);
            let sub = if sa { "Cut lines +1 cm, hems +2.5 / 2 cm, notches" } else { "Off — net stitch lines only" };
            ui.painter().text(pos2(r.left() + 10.0, r.bottom() - 7.0), Align2::LEFT_BOTTOM, sub, FontId::proportional(11.0), P.muted);
            let sw = Rect::from_center_size(r.right_center() - vec2(26.0, 0.0), vec2(34.0, 20.0));
            ui.painter().rect_filled(sw, CornerRadius::same(10), if sa { P.accent } else { P.panel3 });
            ui.painter().circle_filled(pos2(if sa { sw.right() - 10.0 } else { sw.left() + 10.0 }, sw.center().y), 7.0, Color32::WHITE);
            if resp.clicked() {
                sa = !sa;
            }
            menu_label(ui, "Sewing pattern");
            let items = [
                (Icon::Image, "Print sheet · outside", "PNG, 100 dpi, with artwork", Export::PrintPng { side: Side::Outside, dpi: 100.0 }),
                (Icon::Image, "Print sheet · high-res", "PNG, 150 dpi", Export::PrintPng { side: Side::Outside, dpi: 150.0 }),
                (Icon::Image, "Print sheet · inside", "PNG, labels & inside prints", Export::PrintPng { side: Side::Inside, dpi: 100.0 }),
                (Icon::File, "Print & cut sheet · outside", "SVG, true scale, artwork + cut lines", Export::PrintSvg { side: Side::Outside }),
                (Icon::File, "Print & cut sheet · inside", "SVG, inside prints", Export::PrintSvg { side: Side::Inside }),
                (Icon::File, "Cut lines only", "SVG, true scale, notches & labels", Export::CutSvg),
            ];
            for (ic, title, sub, e) in items {
                if menu_row(ui, ic, title, sub, false, true) {
                    chosen = Some(e);
                }
            }
            menu_label(ui, "Preview");
            if menu_row(ui, Icon::Camera, "3D snapshot", "PNG, 2000 px", false, self.viewer.gpu_ok) {
                chosen = Some(Export::Snapshot);
            }
            menu_label(ui, "Design file");
            if menu_row(ui, Icon::Save, "Save as…", ".sb — this app's design file (Ctrl+Shift+S)", false, true) {
                save_as = true;
            }
            if menu_row(ui, Icon::Download, "Web app project", ".json — opens in the ShirtBuilder web app", false, true) {
                chosen = Some(Export::Project);
            }
        });
        self.prefs.seam_allowance = sa;
        self.popups.export = open && chosen.is_none() && !save_as;
        if let Some(e) = chosen {
            self.export(&ctx, e, now);
        }
        if save_as {
            self.save_design_as(now);
        }

        let mut open = self.popups.theme;
        popup(&ctx, "theme-menu", theme_anchor, true, &mut open, 540.0, |ui| self.theme_menu(ui));
        self.popups.theme = open;
    }

    /// changelog and help windows
    pub(crate) fn windows(&mut self, ctx: &egui::Context, _now: f64) {
        let mut open = self.popups.changelog;
        let releases = crate::changelog::CHANGELOG.len();
        let sub = format!("ShirtBuilder {VERSION} · Rust edition · {releases} releases");
        dialog(ctx, "changelog-dialog", "What's new", Some(&sub), 600.0, &mut open, |ui| {
            for (i, (version, date, title, items)) in crate::changelog::CHANGELOG.iter().enumerate() {
                if i > 0 {
                    ui.add_space(8.0);
                    ui.separator();
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("v{version}")).monospace().color(P.accent));
                    ui.label(RichText::new(*title).family(semibold()).size(14.0));
                    if i == 0 {
                        egui::Frame::NONE.fill(P.accent.gamma_multiply(0.18)).corner_radius(CornerRadius::same(6)).inner_margin(egui::Margin::symmetric(6, 1)).show(ui, |ui| {
                            ui.label(RichText::new("LATEST").size(9.5).family(semibold()).color(P.accent));
                        });
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| ui.label(RichText::new(*date).size(11.5).color(P.muted)));
                });
                ui.add_space(4.0);
                for (kind, text) in *items {
                    let (tag, color) = match *kind {
                        "new" => ("NEW", P.accent),
                        "improved" => ("IMPROVED", Color32::from_rgb(0x6f, 0xa8, 0xdc)),
                        "fixed" => ("FIXED", Color32::from_rgb(0xe0, 0xa8, 0x4a)),
                        other => (other, P.muted),
                    };
                    ui.horizontal_top(|ui| {
                        // fixed tag column, so every line of the text lines up after it
                        ui.allocate_ui_with_layout(vec2(64.0, 18.0), Layout::left_to_right(Align::Center), |ui| {
                            ui.set_min_width(64.0);
                            ui.label(RichText::new(tag.to_uppercase()).size(9.5).family(semibold()).color(color));
                        });
                        ui.add(egui::Label::new(*text).wrap());
                    });
                    ui.add_space(3.0);
                }
            }
        });
        self.popups.changelog = open;

        let mut open = self.popups.help;
        dialog(ctx, "help-dialog", "Shortcuts", None, 540.0, &mut open, |ui| {
                ui.label(RichText::new("Design on the flat sewing pattern; the 3D preview follows live. Designs are saved as .sb files — drop one onto the window (or double-click it in the file manager) to open it; drop a picture to add it as artwork.").color(P.muted));
                ui.add_space(6.0);
                egui::Grid::new("keys").num_columns(2).spacing([16.0, 6.0]).show(ui, |ui| {
                    for (k, v) in [
                        ("V / H", "Select tool / Hand tool"),
                        ("T / R / I", "Text / Shape / Image — then click a piece"),
                        ("Drag", "Move the selected artwork · pan empty space"),
                        ("Corner / knob", "Scale / rotate (Shift: free scale, 15° steps)"),
                        ("Alt + drag", "Move without snapping to the centre line"),
                        ("Ctrl+D", "Duplicate layer"),
                        ("Middle-drag", "Pan the pattern"),
                        ("Wheel", "Zoom around the cursor"),
                        ("F · double-click", "Fit pattern to view"),
                        ("Arrows", "Nudge 1 mm (Shift = 1 cm)"),
                        ("Del / Backspace", "Delete selected layer"),
                        ("Ctrl+Z / Ctrl+Shift+Z", "Undo / Redo"),
                        ("Ctrl+N", "New design"),
                        ("Ctrl+O", "Open a design (.sb, or a web app .json)"),
                        ("Ctrl+S", "Save the design (.sb)"),
                        ("Ctrl+Shift+S", "Save as… a new .sb file"),
                        ("1 / 2 / 3", "Pattern / Split / 3D layout"),
                        ("3D: drag / right-drag", "Orbit / pan the camera · wheel zooms"),
                        ("Esc", "Deselect"),
                    ] {
                        ui.label(RichText::new(k).monospace().color(P.text));
                        ui.label(v);
                        ui.end_row();
                    }
                });
        });
        self.popups.help = open;
    }
}

/// A centred dialog over the dimmed app. It is never taller than the window — the body scrolls —
/// so its title and close button always stay on screen; Esc or a click outside also closes it.
pub(crate) fn dialog(ctx: &egui::Context, id: &str, title: &str, subtitle: Option<&str>, width: f32, open: &mut bool, body: impl FnOnce(&mut Ui)) {
    if !*open {
        return;
    }
    let screen = ctx.content_rect();
    let margin = 18.0;
    let w = width.min(screen.width() - 32.0 - 2.0 * margin).max(220.0);
    let max_h = (screen.height() - 80.0 - 2.0 * margin).clamp(140.0, 720.0);
    let frame = egui::Frame::NONE
        .shadow(egui::Shadow { offset: [0, 10], blur: 30, spread: 0, color: Color32::from_black_alpha(115) })
        .fill(P.menu)
        .stroke(Stroke::new(1.0, P.border))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(egui::Margin::same(margin as i8));
    let resp = egui::Modal::new(egui::Id::unique(id)).frame(frame).backdrop_color(Color32::from_black_alpha(110)).show(ctx, |ui| {
        ui.set_width(w);
        let top = ui.cursor().top();
        let mut close = false;
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new(title).family(semibold()).size(18.0).color(P.text));
                if let Some(s) = subtitle {
                    ui.label(RichText::new(s).size(11.5).color(P.muted));
                }
            });
            ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                close = icon_button(ui, Icon::X, "Close (Esc)", true).clicked();
            });
        });
        ui.add_space(6.0);
        ui.separator();
        let head = ui.cursor().top() - top;
        let list_h = (max_h - head).max(80.0);
        // use all the allowed height when the body is long; shrink to fit when it is short
        egui::ScrollArea::vertical().max_height(list_h).min_scrolled_height(list_h).auto_shrink([false, true]).show(ui, |ui| {
            ui.set_width(ui.available_width());
            body(ui);
            ui.add_space(4.0);
        });
        close
    });
    if resp.inner || resp.should_close() {
        *open = false;
    }
}
