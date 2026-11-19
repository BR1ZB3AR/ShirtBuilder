//! Right-hand sidebar: Design and Scene tabs, same sections as the web app.

use crate::app::{Quality, ShirtBuilderApp, Tab};
use crate::style::{icon, icons, semibold, P};
use crate::widgets::{big_button, hex_color, hint, icon_button, section, seg, slider_row, swatch, to_hex, toggle_row};
use egui::{vec2, Align, Align2, Color32, CornerRadius, FontId, Layout, RichText, Sense, Stroke, StrokeKind, Ui};
use lucide_icons::Icon;
use sb_core::doc::{Backdrop, FabricMode, LayerType, MaterialId, ModelKind, SceneSettings, ShapeKind, Side};
use sb_core::pattern::{garment, Style, SleeveLen};

const SWATCHES: [&str; 10] = ["#e8e7e2", "#ffffff", "#151515", "#243a5c", "#5b6b47", "#9c1f2a", "#d3b68f", "#a9bfd8", "#e9a7b6", "#6b6b6b"];

fn material_name(m: MaterialId) -> &'static str {
    match m {
        MaterialId::Cotton => "Cotton",
        MaterialId::Jersey => "Jersey",
        MaterialId::Polyester => "Polyester",
        MaterialId::Linen => "Linen",
        MaterialId::Fleece => "Fleece",
    }
}

impl ShirtBuilderApp {
    pub(crate) fn sidebar(&mut self, ui: &mut Ui, now: f64) {
        // tabs
        let full = ui.max_rect();
        let h = 40.0;
        for (i, (tab, label)) in [(Tab::Design, "Design"), (Tab::Scene, "Scene")].into_iter().enumerate() {
            let r = egui::Rect::from_min_size(full.min + vec2(full.width() / 2.0 * i as f32, 0.0), vec2(full.width() / 2.0, h));
            let resp = ui.interact(r, ui.scope_id().with(("tab", label)), Sense::click());
            let on = self.prefs.tab == tab;
            resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, label));
            ui.painter().text(r.center(), Align2::CENTER_CENTER, label, FontId::new(13.0, semibold()), if on { P.accent } else if resp.hovered() { P.text } else { P.muted });
            if on {
                ui.painter().hline(r.x_range(), r.bottom() - 1.0, Stroke::new(2.0, P.accent));
            }
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                self.prefs.tab = tab;
            }
        }
        ui.painter().hline(full.x_range(), full.top() + h, Stroke::new(1.0, P.border));
        ui.add_space(h + 1.0);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match self.prefs.tab {
            Tab::Design => self.design_tab(ui, now),
            Tab::Scene => self.scene_tab(ui, now),
        });
    }

    fn design_tab(&mut self, ui: &mut Ui, now: f64) {
        let g = garment(self.doc.garment);
        let (tp, ts) = self.target_piece();
        let piece_name = g.piece(tp).map(|p| p.name.to_lowercase()).unwrap_or_default();

        // ---- Artwork
        let sub = format!("Add an image or text to {piece_name}{}", if ts == Side::Inside { " (inside)" } else { "" });
        section(ui, "Artwork", Some(&sub), None, |ui| {
            ui.horizontal(|ui| {
                let w = (ui.available_width() - 8.0) / 2.0;
                if big_button(ui, Icon::Image, "Image", w, true).on_hover_text("PNG, JPEG, WebP, GIF or SVG — or drop a picture onto the window").clicked() {
                    self.pick_image(now, None);
                }
                if big_button(ui, Icon::Type, "Text", w, true).clicked() {
                    self.add_text(now, None);
                }
            });
        });

        // ---- Layers
        let add = section(ui, "Layers", None, Some((Icon::Plus, "Add a shape layer")), |ui| {
            if self.doc.layers.is_empty() {
                hint(ui, "No artwork yet. Add text above, or pick the Text tool and click a pattern piece.");
            }
            let mut toggle_vis: Option<String> = None;
            let mut select: Option<String> = None;
            for l in self.doc.layers.iter().rev() {
                let on = self.selected.as_deref() == Some(l.id.as_str());
                let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 46.0), Sense::click());
                let row_name = l.text.as_deref().map(|t| t.replace('\n', " ")).unwrap_or_else(|| l.name.clone());
                resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, &row_name));
                ui.painter().rect(
                    r,
                    CornerRadius::same(8),
                    if on { P.accent_weak } else if resp.hovered() { P.panel2 } else { Color32::TRANSPARENT },
                    Stroke::new(1.0, if on { P.accent } else { Color32::TRANSPARENT }),
                    StrokeKind::Inside,
                );
                // eye
                let eye = egui::Rect::from_center_size(r.left_center() + vec2(18.0, 0.0), vec2(24.0, 24.0));
                let eye_resp = ui.interact(eye, ui.scope_id().with(("eye", &l.id)), Sense::click()).on_hover_text(if l.visible { "Hide" } else { "Show" });
                eye_resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, l.visible, "Visible"));
                ui.painter().text(eye.center(), Align2::CENTER_CENTER, icon(if l.visible { Icon::Eye } else { Icon::EyeOff }), FontId::new(15.0, icons()), if l.visible { P.accent } else { P.muted });
                if eye_resp.clicked() {
                    toggle_vis = Some(l.id.clone());
                }
                // thumbnail
                let th = egui::Rect::from_center_size(r.left_center() + vec2(52.0, 0.0), vec2(32.0, 32.0));
                ui.painter().rect_filled(th, CornerRadius::same(6), P.panel3);
                let (glyph, family) = match l.kind {
                    LayerType::Text => ("Aa".to_string(), FontId::new(14.0, semibold())),
                    LayerType::Shape => (icon(Icon::Square), FontId::new(15.0, icons())),
                    LayerType::Image => (icon(Icon::Image), FontId::new(15.0, icons())),
                };
                ui.painter().text(th.center(), Align2::CENTER_CENTER, glyph, family, P.text);
                // name + where
                let name = l.text.as_deref().map(|t| t.replace('\n', " ")).unwrap_or_else(|| l.name.clone());
                let piece = g.piece(l.piece).map(|p| p.name.to_lowercase()).unwrap_or_default();
                let kind = match l.kind {
                    LayerType::Text => "text",
                    LayerType::Image => "image",
                    LayerType::Shape => "shape",
                };
                let where_ = format!("{kind} · {piece}{}", if l.side == Side::Inside { " · inside" } else { "" });
                let tx = r.left() + 76.0;
                let max_w = r.right() - tx - 8.0;
                let galley = ui.painter().layout(name, FontId::new(12.5, semibold()), P.text, max_w);
                ui.painter().galley(egui::pos2(tx, r.top() + 8.0), galley, P.text);
                ui.painter().text(egui::pos2(tx, r.bottom() - 8.0), Align2::LEFT_BOTTOM, where_, FontId::proportional(11.0), P.muted);
                if resp.clicked() && !eye_resp.clicked() {
                    select = Some(l.id.clone());
                }
                ui.add_space(2.0);
            }
            if let Some(id) = toggle_vis {
                self.edit(now, None, |d| {
                    if let Some(l) = d.layers.iter_mut().find(|l| l.id == id) {
                        l.visible = !l.visible;
                    }
                });
            }
            if let Some(id) = select {
                if let Some(l) = self.doc.layers.iter().find(|l| l.id == id) {
                    self.side = l.side;
                }
                self.selected = Some(id);
            }
            self.layer_props(ui, now);
        });
        if add {
            self.add_shape(now, None);
        }

        // ---- Fabric
        let sub = format!("{}\n{}", g.spec.title.to_uppercase(), g.spec.subtitle.to_uppercase());
        section(ui, "Fabric", Some(&sub), None, |ui| {
            let cur = self.doc.fabric.color.to_lowercase();
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(4.0, 6.0);
                for c in SWATCHES {
                    if swatch(ui, hex_color(c), cur == c, c).clicked() {
                        self.edit(now, None, |d| d.fabric.color = c.to_string());
                    }
                }
                // custom colour
                let mut rgb = hex_color(&cur);
                let custom = !SWATCHES.contains(&cur.as_str());
                let resp = ui.color_edit_button_srgba(&mut rgb).on_hover_text("Custom colour");
                if custom {
                    ui.painter().circle_stroke(resp.rect.center(), 14.0, Stroke::new(2.0, P.accent));
                }
                if resp.changed() {
                    let hex = to_hex(rgb);
                    self.edit(now, Some("fabric-custom"), |d| d.fabric.color = hex);
                }
                if icon_button(ui, Icon::Pipette, "Pick a colour from the screen", true).clicked() {
                    self.picking = Some((false, egui::Pos2::ZERO));
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Colour");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let mut mode = self.doc.fabric.mode;
                    if seg(ui, &mut mode, &[(FabricMode::Solid, "Solid"), (FabricMode::Gradient, "Gradient")], true) {
                        self.edit(now, None, |d| d.fabric.mode = mode);
                    }
                });
            });
            if self.doc.fabric.mode == FabricMode::Gradient {
                ui.horizontal(|ui| {
                    ui.label("Second colour");
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let mut c2 = hex_color(&self.doc.fabric.color2);
                        if ui.color_edit_button_srgba(&mut c2).changed() {
                            let hex = to_hex(c2);
                            self.edit(now, Some("fabric-color2"), |d| d.fabric.color2 = hex);
                        }
                    });
                });
                let mut a = self.doc.fabric.angle;
                if slider_row(ui, "Angle", &mut a, 0.0..=360.0, 1.0, |v| format!("{v:.0}°"), true) {
                    self.edit(now, Some("fabric-angle"), |d| d.fabric.angle = a);
                }
            }
        });

        // ---- Appearance
        section(ui, "Appearance", None, None, |ui| {
            let mut f = self.doc.fabric.clone();
            ui.horizontal(|ui| {
                let lbl = ui.label("Material");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    egui::ComboBox::new("material", "")
                        .selected_text(material_name(f.material))
                        .width(126.0)
                        .show_ui(ui, |ui| {
                            for m in [MaterialId::Cotton, MaterialId::Jersey, MaterialId::Polyester, MaterialId::Linen, MaterialId::Fleece] {
                                ui.selectable_value(&mut f.material, m, material_name(m));
                            }
                        })
                        .response
                        .labelled_by(lbl.id);
                });
            });
            let mut key = None;
            if toggle_row(ui, "Fold shadows (AO)", &mut f.ao) | toggle_row(ui, "Fabric wrinkles", &mut f.wrinkles_on) {
                key = Some("toggle");
            }
            let one = |v: f64| format!("{v:.1}");
            let two = |v: f64| format!("{v:.2}");
            let wr_on = f.wrinkles_on;
            for (label, v, range, step, fmt, k, enabled) in [
                ("Wrinkles", &mut f.wrinkles, 0.0..=1.0, 0.01, &one as &dyn Fn(f64) -> String, "wr", wr_on),
                ("Texture", &mut f.texture, 0.0..=1.0, 0.01, &one, "tx", true),
                ("Knit size", &mut f.knit, 1.0..=6.0, 0.5, &|v: f64| format!("×{v}"), "knit", true),
                ("Brightness", &mut f.brightness, 0.5..=1.5, 0.01, &two, "br", true),
                ("Contrast", &mut f.contrast, 0.5..=1.5, 0.01, &two, "ct", true),
            ] {
                if slider_row(ui, label, v, range, step, fmt, enabled) {
                    key = Some(k);
                }
            }
            if f != self.doc.fabric {
                let k = key.unwrap_or("material");
                self.edit(now, Some(k), |d| d.fabric = f);
            }
        });
    }

    /// properties of the selected layer (like the web app's layer panel)
    fn layer_props(&mut self, ui: &mut Ui, now: f64) {
        let Some(l) = self.selected_layer().cloned() else { return };
        let unit = self.prefs.unit;
        let g = garment(self.doc.garment);
        let Some(p) = g.piece(l.piece).cloned() else { return };
        let mut action: Option<i32> = None;
        let mut replace = false;
        let mut key = format!("props-{}", l.id);
        let mut n = l.clone();
        ui.add_space(8.0);
        egui::Frame::NONE.fill(P.panel2).corner_radius(CornerRadius::same(10)).inner_margin(egui::Margin::same(10)).stroke(Stroke::new(1.0, P.border)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            if l.kind == LayerType::Text {
                // the words printed on the shirt come first; the layer is named after them
                let lbl = ui.label(RichText::new("Text").color(P.muted));
                let mut text = n.text.clone().unwrap_or_default();
                let r = ui.add(egui::TextEdit::multiline(&mut text).desired_rows(2).desired_width(ui.available_width() - 26.0).hint_text("Type the words to print")).labelled_by(lbl.id);
                if r.changed() {
                    n.name = text.lines().map(str::trim).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ");
                    n.text = Some(text);
                    crate::exports::refit_text(&mut n);
                    key = format!("text-{}", l.id);
                }
                ui.add_space(4.0);
            } else {
                ui.horizontal(|ui| {
                    let lbl = ui.label(RichText::new("Name").color(P.muted));
                    if ui.add(egui::TextEdit::singleline(&mut n.name).desired_width(f32::INFINITY)).labelled_by(lbl.id).changed() {
                        key = format!("name-{}", l.id);
                    }
                });
            }
            ui.horizontal(|ui| {
                let mut piece = n.piece;
                egui::ComboBox::new("layer-piece", "").selected_text(p.name.as_str()).width(110.0).show_ui(ui, |ui| {
                    for pc in &g.pieces {
                        ui.selectable_value(&mut piece, pc.id, &pc.name);
                    }
                });
                if piece != n.piece {
                    // re-place on the new piece, shrunk to fit if needed (web: defaultSpot + fit)
                    if let Some(np) = g.piece(piece) {
                        let (x, y) = self.default_spot(piece);
                        let k = 1f64.min(np.w * 0.8 / n.w).min(np.h * 0.8 / n.h);
                        n.piece = piece;
                        n.x = x;
                        n.y = y;
                        n.w *= k;
                        n.h *= k;
                        key = format!("piece-{}", l.id);
                    }
                }
                let mut side = n.side;
                if seg(ui, &mut side, &[(Side::Outside, "Outside"), (Side::Inside, "Inside")], true) {
                    n.side = side;
                    n.x = p.w - n.x;
                    n.rot = -n.rot;
                    self.side = side;
                }
            });
            let keep_aspect = l.kind != LayerType::Shape;
            egui::Grid::new("props").num_columns(4).spacing([6.0, 6.0]).show(ui, |ui| {
                let k = unit.cm();
                let field = |ui: &mut Ui, label: &str, v: &mut f64, suffix: &str, scale: f64| -> bool {
                    let lbl = ui.label(RichText::new(label).color(P.muted));
                    let mut shown = *v / scale;
                    let r = ui.add(egui::DragValue::new(&mut shown).speed(0.1).max_decimals(2).suffix(suffix)).labelled_by(lbl.id);
                    if r.changed() {
                        *v = shown * scale;
                    }
                    r.changed()
                };
                let sfx = format!(" {}", unit.label());
                field(ui, "X", &mut n.x, &sfx, k);
                field(ui, "Y", &mut n.y, &sfx, k);
                ui.end_row();
                let (w0, h0) = (n.w, n.h);
                if field(ui, "W", &mut n.w, &sfx, k) && keep_aspect && w0 > 0.0 {
                    n.h = h0 * n.w.max(0.1) / w0;
                }
                if field(ui, "H", &mut n.h, &sfx, k) && keep_aspect && h0 > 0.0 {
                    n.w = w0 * n.h.max(0.1) / h0;
                }
                ui.end_row();
                ui.end_row();
            });
            ui.horizontal(|ui| {
                let lbl = ui.label(RichText::new("R").color(P.muted));
                let mut r = n.rot;
                if ui.add(egui::DragValue::new(&mut r).speed(0.5).max_decimals(1).suffix("°")).labelled_by(lbl.id).changed() {
                    n.rot = r;
                }
                if icon_button(ui, Icon::AlignHorizontalJustifyCenter, "Centre horizontally on piece", true).clicked() {
                    n.x = p.w / 2.0;
                }
                if ui.small_button("0°").on_hover_text("Reset rotation").clicked() {
                    n.rot = 0.0;
                }
            });
            n.w = n.w.max(0.1);
            n.h = n.h.max(0.1);
            let mut op = n.opacity;
            if slider_row(ui, "Opacity", &mut op, 0.0..=1.0, 0.01, |v| format!("{:.0}%", v * 100.0), true) {
                n.opacity = op;
                key = format!("op-{}", l.id);
            }
            match l.kind {
                LayerType::Text => {
                    let mut refit = false;
                    ui.horizontal(|ui| {
                        let cur = n.font.clone().unwrap_or_else(|| "Inter".into());
                        let mut font = cur.clone();
                        egui::ComboBox::new("layer-font", "")
                            .selected_text(RichText::new(&cur).family(egui::FontFamily::Name(cur.clone().into())))
                            .width(170.0)
                            .show_ui(ui, |ui| {
                                for f in sb_print::text::FONTS {
                                    ui.selectable_value(&mut font, f.to_string(), RichText::new(f).size(15.0).family(egui::FontFamily::Name(f.into())));
                                }
                            });
                        if font != cur {
                            n.font = Some(font);
                            refit = true;
                        }
                        let mut c = hex_color(n.color.as_deref().unwrap_or("#ffffff"));
                        if ui.color_edit_button_srgba(&mut c).on_hover_text("Text colour").changed() {
                            n.color = Some(to_hex(c));
                            key = format!("color-{}", l.id);
                        }
                    });
                    ui.horizontal(|ui| {
                        let bold = n.weight.unwrap_or(400.0) >= 700.0;
                        if toggle_icon(ui, Icon::Bold, "Bold", bold) {
                            n.weight = Some(if bold { 400.0 } else { 700.0 });
                            refit = true;
                        }
                        let italic = n.italic.unwrap_or(false);
                        if toggle_icon(ui, Icon::Italic, "Italic", italic) {
                            n.italic = Some(!italic);
                            refit = true;
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let align = n.align.unwrap_or(sb_core::doc::Align::Center);
                            for (a, ic, tip) in [(sb_core::doc::Align::Right, Icon::AlignRight, "Align right"), (sb_core::doc::Align::Center, Icon::AlignCenter, "Align centre"), (sb_core::doc::Align::Left, Icon::AlignLeft, "Align left")] {
                                if toggle_icon(ui, ic, tip, align == a) {
                                    n.align = Some(a);
                                }
                            }
                        });
                    });
                    let mut lh = n.line_height.unwrap_or(1.1);
                    if slider_row(ui, "Line height", &mut lh, 0.7..=2.0, 0.01, |v| format!("{v:.2}"), true) {
                        n.line_height = Some(lh);
                        refit = true;
                        key = format!("lh-{}", l.id);
                    }
                    let mut ls = n.letter_spacing.unwrap_or(0.0);
                    if slider_row(ui, "Spacing", &mut ls, -0.1..=0.5, 0.01, |v| format!("{v:.2}"), true) {
                        n.letter_spacing = Some(ls);
                        refit = true;
                        key = format!("ls-{}", l.id);
                    }
                    if refit {
                        crate::exports::refit_text(&mut n);
                    }
                }
                LayerType::Image => {
                    if ui.add_sized([ui.available_width(), 30.0], egui::Button::new("Replace image…")).clicked() {
                        replace = true;
                    }
                }
                LayerType::Shape => {
                    ui.horizontal(|ui| {
                        let mut kind = n.shape.unwrap_or(ShapeKind::Rect);
                        if seg(ui, &mut kind, &[(ShapeKind::Rect, "Rectangle"), (ShapeKind::Ellipse, "Ellipse")], true) {
                            n.shape = Some(kind);
                        }
                        let mut c = hex_color(n.fill.as_deref().unwrap_or("#ffffff"));
                        if ui.color_edit_button_srgba(&mut c).on_hover_text("Fill colour").changed() {
                            n.fill = Some(to_hex(c));
                            key = format!("fill-{}", l.id);
                        }
                    });
                    if n.shape != Some(ShapeKind::Ellipse) {
                        let mut r = n.radius.unwrap_or(0.0);
                        let max = (n.w.min(n.h) / 2.0).max(0.1);
                        if slider_row(ui, "Corners", &mut r, 0.0..=max, 0.1, |v| format!("{v:.1}"), true) {
                            n.radius = Some(r);
                            key = format!("rad-{}", l.id);
                        }
                    }
                }
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if icon_button(ui, Icon::ChevronUp, "Bring forward", true).clicked() {
                    action = Some(1);
                }
                if icon_button(ui, Icon::ChevronDown, "Send backward", true).clicked() {
                    action = Some(-1);
                }
                if ui.button(format!("{}  Duplicate", icon(Icon::Copy))).on_hover_text("Duplicate (Ctrl+D)").clicked() {
                    action = Some(2);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if icon_button(ui, Icon::Trash2, "Delete (Del)", true).clicked() {
                        action = Some(0);
                    }
                });
            });
        });
        if n != l {
            self.edit(now, Some(&key), |d| {
                if let Some(x) = d.layers.iter_mut().find(|x| x.id == l.id) {
                    *x = n;
                }
            });
        }
        if replace {
            self.replace_image(now, &l.id);
        }
        match action {
            Some(0) => self.remove_selected(now),
            Some(2) => self.duplicate_layer(now, &l.id),
            Some(dir) => self.move_layer(now, &l.id, dir as isize),
            None => {}
        }
    }

    fn scene_tab(&mut self, ui: &mut Ui, now: f64) {
        let mut s = self.doc.scene.clone();
        let mut key: Option<&str> = None;
        let fabric = hex_color(&self.doc.fabric.color);

        section(ui, "Backdrop", None, None, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let w = (ui.available_width() - 24.0) / 5.0;
                for (b, label) in [(Backdrop::Window, "Window"), (Backdrop::Leaves, "Leaves"), (Backdrop::Studio, "Studio"), (Backdrop::Gradient, "Gradient"), (Backdrop::Solid, "Solid")] {
                    let (r, resp) = ui.allocate_exact_size(vec2(w, w + 18.0), Sense::click());
                    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, s.bg == b, label));
                    let tile = egui::Rect::from_min_size(r.min, vec2(w, w));
                    let base = if s.match_fabric { fabric.lerp_to_gamma(Color32::from_rgb(0xcf, 0xd6, 0xcf), 0.3) } else { hex_color(&s.bg_color) };
                    let (inner, outer) = match b {
                        Backdrop::Studio => (Color32::from_rgb(0xf2, 0xf2, 0xf0), Color32::from_rgb(0xa5, 0xa7, 0xa5)),
                        Backdrop::Solid => (base, base),
                        _ => (base.lerp_to_gamma(Color32::WHITE, 0.25), base.lerp_to_gamma(Color32::BLACK, 0.4)),
                    };
                    radial(ui.painter(), tile, inner, outer);
                    if b == Backdrop::Window || b == Backdrop::Leaves {
                        for k in 0..3 {
                            let x = tile.left() + w * (0.15 + 0.3 * k as f32);
                            ui.painter().line_segment(
                                [egui::pos2(x, tile.bottom()), egui::pos2(x + w * 0.45, tile.top())],
                                Stroke::new(w * 0.09, Color32::from_white_alpha(if b == Backdrop::Window { 40 } else { 18 })),
                            );
                        }
                    }
                    let on = s.bg == b;
                    if on {
                        ui.painter().rect_stroke(tile, CornerRadius::same(8), Stroke::new(2.0, P.accent), StrokeKind::Outside);
                    }
                    ui.painter().text(egui::pos2(r.center().x, r.bottom()), Align2::CENTER_BOTTOM, label, FontId::proportional(11.0), if on { P.accent } else { P.muted });
                    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        s.bg = b;
                    }
                }
            });
            ui.add_space(4.0);
            toggle_row(ui, "Match fabric colour", &mut s.match_fabric);
            if !s.match_fabric {
                ui.horizontal(|ui| {
                    ui.label("Backdrop colour");
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let mut c = hex_color(&s.bg_color);
                        if ui.color_edit_button_srgba(&mut c).changed() {
                            s.bg_color = to_hex(c);
                            key = Some("bgc");
                        }
                    });
                });
            }
        });

        section(ui, "Lighting", None, None, |ui| {
            let two = |v: f64| format!("{v:.2}");
            if slider_row(ui, "Intensity", &mut s.light, 0.2..=2.0, 0.01, two, true) {
                key = Some("li");
            }
            if slider_row(ui, "Direction", &mut s.light_angle, -180.0..=180.0, 1.0, |v| format!("{v:.0}°"), true) {
                key = Some("la");
            }
            if slider_row(ui, "Reflections", &mut s.env, 0.0..=2.0, 0.01, two, true) {
                key = Some("env");
            }
            if slider_row(ui, "Exposure", &mut s.exposure, 0.4..=2.0, 0.01, two, true) {
                key = Some("exp");
            }
            toggle_row(ui, "Window light pattern", &mut s.gobo);
            toggle_row(ui, "Soft shadows", &mut s.shadows);
        });

        let spec = garment(self.doc.garment).spec.clone();
        section(ui, "Display", None, None, |ui| {
            ui.horizontal(|ui| {
                ui.label("Pose");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let mut pose = s.hanger;
                    if seg(ui, &mut pose, &[(false, "Ghost mannequin"), (true, "On hanger")], true) {
                        s.hanger = pose;
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.label("3D model");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    seg(ui, &mut s.model, &[(ModelKind::Scan, "Realistic"), (ModelKind::Generated, "Generated")], true);
                });
            });
            let has_scan = spec.style() == Style::Tee && spec.sleeve() == SleeveLen::Short;
            hint(
                ui,
                if !has_scan {
                    "A realistic model is available for the Round Neck T-Shirt, short sleeve (male & female) — other garments use the generated shirt."
                } else if s.hanger {
                    "The realistic model is a ghost-mannequin scan — the hanger pose uses the generated shirt."
                } else if s.model == ModelKind::Scan {
                    "Realistic model: real folds and fabric detail. Wrinkle sliders only affect the generated shirt."
                } else {
                    "Generated shirt: wrinkles and fold shadows are adjustable under Appearance."
                },
            );
            toggle_row(ui, "Auto-rotate", &mut s.auto_rotate);
        });

        self.gpu_section(ui, now);

        section(ui, "Performance", None, None, |ui| {
            ui.label("3D quality");
            seg(ui, &mut self.prefs.quality, &[(Quality::Auto, "Auto"), (Quality::High, "High"), (Quality::Balanced, "Balanced"), (Quality::Fast, "Fast")], true);
            hint(
                ui,
                match self.prefs.quality {
                    Quality::Auto => "Starts Balanced and steps down by itself if rotating runs below ~35 fps on this computer.",
                    Quality::High => "Sharpest image and real-time shadows everywhere — needs a strong graphics card.",
                    Quality::Balanced => "Smooth on most computers; the realistic model uses its own baked shading for shadows.",
                    Quality::Fast => "For slow graphics: lower resolution, no shadows or window light, simpler fabric shading.",
                },
            );
            toggle_row(ui, "Show frame rate", &mut self.prefs.show_fps);
            ui.add_space(6.0);
            if ui.add_sized([ui.available_width(), 30.0], egui::Button::new("Reset scene settings")).clicked() {
                s = SceneSettings::default();
                key = None;
            }
        });

        if s != self.doc.scene {
            self.edit(now, key, |d| d.scene = s);
        }
    }
}

impl ShirtBuilderApp {
    /// Scene › Graphics card: what draws the 3D, every card found, and the choice for next start
    fn gpu_section(&mut self, ui: &mut Ui, now: f64) {
        let mut restart = false;
        section(ui, "Graphics card", None, None, |ui| {
            let gpu = &mut self.gpu;
            match &gpu.active {
                Some(a) => {
                    let (ic, tint) = if a.software() { (Icon::TriangleAlert, Color32::from_rgb(0xe0, 0xa8, 0x4a)) } else { (Icon::Cpu, P.accent) };
                    egui::Frame::NONE.fill(P.panel2).corner_radius(CornerRadius::same(10)).inner_margin(egui::Margin::same(10)).stroke(Stroke::new(1.0, P.border)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(icon(ic)).family(icons()).size(18.0).color(tint));
                            ui.vertical(|ui| {
                                ui.label(RichText::new(&a.name).family(semibold()));
                                ui.label(RichText::new(format!("{} · {}", a.backend, a.kind_label())).size(11.0).color(P.muted));
                                if !a.driver.is_empty() {
                                    ui.label(RichText::new(format!("Driver {}", a.driver)).size(11.0).color(P.muted));
                                }
                            });
                        });
                    });
                    if let (true, Some(p)) = (a.software(), &gpu.problem) {
                        ui.label(RichText::new(p).size(11.5).color(Color32::from_rgb(0xe0, 0xa8, 0x4a)));
                    } else if a.software() {
                        hint(
                            ui,
                            if gpu.better_than_active().is_some() {
                                "The 3D is being drawn by the CPU although a graphics card was found. Choose it below and restart."
                            } else {
                                "No graphics card driver could be used, so the 3D is drawn by the CPU (slow). Install your card's Vulkan driver (NVIDIA: nvidia-utils · AMD: vulkan-radeon · Intel: vulkan-intel) and restart."
                            },
                        );
                    } else {
                        hint(ui, "The 3D preview and the whole window are drawn by this graphics card.");
                    }
                }
                None => hint(ui, "This build draws without wgpu, so the graphics card can't be chosen."),
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                let label = |c: &crate::gpu::GpuChoice| match c {
                    crate::gpu::GpuChoice::Auto => "Auto-detect (best card)".to_string(),
                    crate::gpu::GpuChoice::Adapter { name, backend } => format!("{name} · {backend}"),
                };
                let mut choice = gpu.choice.clone();
                ui.horizontal(|ui| {
                    let lbl = ui.label("Use");
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        egui::ComboBox::new("gpu-choice", "")
                            .selected_text(label(&choice))
                            .width(200.0)
                            .truncate()
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut choice, crate::gpu::GpuChoice::Auto, label(&crate::gpu::GpuChoice::Auto));
                                for g in &gpu.all {
                                    let text = format!("{} · {}{}", g.name, g.backend, if g.software() { " (CPU)" } else { "" });
                                    ui.selectable_value(&mut choice, g.key(), text);
                                }
                            })
                            .response
                            .labelled_by(lbl.id);
                    });
                });
                if choice != gpu.choice {
                    match crate::gpu::save_choice(&choice) {
                        Ok(()) => gpu.choice = choice,
                        Err(e) => self.status = Some((format!("Couldn't save the graphics card choice: {e}"), now)),
                    }
                }
                let gpu = &self.gpu;
                if gpu.choice != gpu.started_with {
                    hint(ui, "The new choice takes effect when ShirtBuilder restarts.");
                    if ui.add_sized([ui.available_width(), 30.0], egui::Button::new("Restart ShirtBuilder now")).clicked() {
                        restart = true;
                    }
                }
            }
        });
        #[cfg(not(target_arch = "wasm32"))]
        if restart {
            match crate::gpu::relaunch() {
                Ok(()) => ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close),
                Err(e) => self.say(format!("Couldn't restart: {e}"), now),
            }
        }
        let _ = restart;
    }
}

/// a small square toggle button with an icon; true when clicked
fn toggle_icon(ui: &mut Ui, ic: Icon, tip: &str, on: bool) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(28.0, 26.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, tip));
    if on {
        ui.painter().rect_filled(r, CornerRadius::same(6), P.accent_weak);
        ui.painter().rect_stroke(r, CornerRadius::same(6), Stroke::new(1.0, P.accent), StrokeKind::Inside);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, CornerRadius::same(6), P.panel3);
    }
    ui.painter().text(r.center(), Align2::CENTER_CENTER, icon(ic), FontId::new(14.0, icons()), if on { P.accent } else { P.text });
    resp.on_hover_text(tip).clicked()
}

/// soft radial gradient filling a rounded tile (approximated with a triangle fan)
fn radial(p: &egui::Painter, r: egui::Rect, inner: Color32, outer: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(r.center(), inner);
    let n = 24;
    for i in 0..=n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        let dir = vec2(a.cos(), a.sin());
        // project onto the rectangle border
        let k = (r.width() / 2.0 / dir.x.abs().max(1e-3)).min(r.height() / 2.0 / dir.y.abs().max(1e-3));
        mesh.colored_vertex(r.center() + dir * k, outer);
    }
    for i in 0..n {
        mesh.add_triangle(0, i + 1, i + 2);
    }
    p.add(egui::Shape::mesh(mesh));
}
