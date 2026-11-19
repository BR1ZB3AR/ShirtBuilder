//! The Theme pop-up (web app's `ThemeMenu.tsx`): one theme or follow the system, light / dark and
//! kind filters, a gallery of miniature previews (the app in that theme, with a shirt on the
//! canvas) and the accent colour.

use crate::app::ShirtBuilderApp;
use crate::style::{icon, icons, semibold, Fx, P};
use crate::themes::{c, ca, Theme, ACCENTS, THEMES};
use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui};
use lucide_icons::Icon;
use sb_print::color::mix;

/// the shirt outline of the preview (40 × 36 units), as a polygon
fn shirt_outline() -> Vec<Pos2> {
    let mut pts = vec![(14.5, 1.5), (8.0, 3.6), (1.2, 9.6), (5.0, 15.6), (9.0, 13.2), (9.0, 34.5), (31.0, 34.5), (31.0, 13.2), (35.0, 15.6), (38.8, 9.6), (32.0, 3.6), (25.5, 1.5)];
    // the neckline curve back to the start: C24.6 4.6 22.6 6.2 20 6.2 S15.4 4.6 14.5 1.5
    let cub = |p0: (f32, f32), p1: (f32, f32), p2: (f32, f32), p3: (f32, f32), t: f32| {
        let u = 1.0 - t;
        (
            u * u * u * p0.0 + 3.0 * u * u * t * p1.0 + 3.0 * u * t * t * p2.0 + t * t * t * p3.0,
            u * u * u * p0.1 + 3.0 * u * u * t * p1.1 + 3.0 * u * t * t * p2.1 + t * t * t * p3.1,
        )
    };
    for i in 1..=6 {
        pts.push(cub((25.5, 1.5), (24.6, 4.6), (22.6, 6.2), (20.0, 6.2), i as f32 / 6.0));
    }
    for i in 1..6 {
        pts.push(cub((20.0, 6.2), (17.4, 6.2), (15.4, 4.6), (14.5, 1.5), i as f32 / 6.0));
    }
    pts.into_iter().map(|(x, y)| pos2(x, y)).collect()
}

/// miniature of the app drawn in a theme's colours — sidebar, top bar, canvas with a shirt
fn preview(painter: &egui::Painter, r: Rect, t: &Theme, accent: &str) {
    let k = r.width() / 160.0;
    let at = |x: f32, y: f32| pos2(r.left() + x * k, r.top() + y * k);
    let rect = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(at(x, y), vec2(w * k, h * k));
    let dark = t.dark;
    let canvas = t.canvas.map(String::from).unwrap_or_else(|| if dark { mix(t.bg, t.panel, 0.35) } else { mix(t.bg, t.text, 0.03) });
    let line = c(&mix(t.panel, t.text, if t.hc { 0.7 } else { 0.22 }));
    let shirt = if t.hc { t.text.to_string() } else { mix(&mix(t.muted, accent, 0.28), &canvas, 0.15) };
    let shirt_dark = if t.hc { t.bg.to_string() } else { mix(&shirt, "#000000", if dark { 0.35 } else { 0.2 }) };
    let glass = t.fx == Fx::Glass;
    let cyber = t.fx == Fx::Cyber;
    let a2 = t.accent2.unwrap_or(accent);
    let panel = if glass {
        if dark {
            Color32::from_white_alpha(26)
        } else {
            Color32::from_rgba_unmultiplied(255, 255, 255, 153)
        }
    } else {
        c(t.panel)
    };
    let edge = if glass {
        if dark {
            Color32::from_white_alpha(71)
        } else {
            Color32::from_rgba_unmultiplied(255, 255, 255, 242)
        }
    } else if cyber {
        c(accent)
    } else {
        Color32::TRANSPARENT
    };
    painter.rect_filled(r, CornerRadius::same(6), c(t.bg));
    let clip = painter.with_clip_rect(r);
    if glass {
        for (i, (cx, cy, rr)) in [(20.0, 18.0, 70.0), (140.0, 22.0, 65.0), (85.0, 95.0, 80.0)].into_iter().enumerate() {
            crate::themes::glow(&clip, at(cx, cy), rr * k, rr * k, ca(t.blobs[i], 0.95));
        }
    }
    if cyber {
        let s = Stroke::new(0.5 * k, ca(a2, 0.35));
        let mut x = 0.0;
        while x <= 160.0 {
            clip.line_segment([at(x, 0.0), at(x, 90.0)], s);
            x += 8.0;
        }
        let mut y = 0.0;
        while y <= 90.0 {
            clip.line_segment([at(0.0, y), at(160.0, y)], s);
            y += 8.0;
        }
        clip.rect_filled(rect(0.0, 55.0, 160.0, 35.0), 0.0, ca(a2, 0.12));
    }
    // top bar
    clip.rect_filled(rect(0.0, 0.0, 160.0, 11.0), 0.0, panel);
    clip.rect_filled(rect(36.0, 3.5, 14.0, 4.0), CornerRadius::same((2.0 * k) as u8), c(accent));
    clip.rect_filled(rect(53.0, 3.5, 10.0, 4.0), CornerRadius::same((2.0 * k) as u8), line);
    clip.rect_filled(rect(66.0, 3.5, 10.0, 4.0), CornerRadius::same((2.0 * k) as u8), line);
    clip.circle_filled(at(132.0, 5.5), 2.6 * k, c(accent));
    clip.rect_filled(rect(138.0, 3.5, 16.0, 4.0), CornerRadius::same((2.0 * k) as u8), c(a2));
    // sidebar
    clip.rect(rect(0.0, 11.0, 32.0, 79.0), 0.0, panel, Stroke::new(if glass || cyber { 0.6 * k } else { 0.0 }, edge), StrokeKind::Inside);
    for (i, y) in [20.0, 27.0, 34.0, 41.0].into_iter().enumerate() {
        clip.circle_filled(at(6.5, y + 1.5), 1.5 * k, if i == 0 { c(accent) } else { line });
        clip.rect_filled(rect(10.0, y, if i == 0 { 16.0 } else { 12.0 - i as f32 }, 3.0), CornerRadius::same((1.5 * k) as u8), line);
    }
    // canvas
    let cv_fill = if glass || cyber {
        if dark {
            Color32::from_black_alpha(46)
        } else {
            Color32::from_rgba_unmultiplied(255, 255, 255, 89)
        }
    } else {
        c(&canvas)
    };
    let cv_edge = if t.hc { c(t.text) } else if glass || cyber { edge } else { Color32::TRANSPARENT };
    clip.rect(rect(36.0, 15.0, 120.0, 71.0), CornerRadius::same((3.0 * k) as u8), cv_fill, Stroke::new(0.6 * k, cv_edge), StrokeKind::Inside);
    // the shirt (diagonal gradient)
    let outline: Vec<Pos2> = shirt_outline().into_iter().map(|p| at(76.0 + p.x, 26.0 + p.y)).collect();
    let local: Vec<sb_core::V2> = shirt_outline().into_iter().map(|p| sb_core::v2(p.x as f64, p.y as f64)).collect();
    let tris = crate::tess::triangulate(&local);
    let (s0, s1) = (c(&shirt), c(&shirt_dark));
    let mut mesh = egui::Mesh::default();
    for (p, q) in outline.iter().zip(&local) {
        let tt = ((q.x + q.y) / 74.0) as f32;
        mesh.colored_vertex(*p, s0.lerp_to_gamma(s1, tt.clamp(0.0, 1.0)));
    }
    for tr in tris {
        mesh.add_triangle(tr[0], tr[1], tr[2]);
    }
    clip.add(egui::Shape::mesh(mesh));
    let stroke_col = if t.hc { c(t.text) } else { c(&mix(&shirt_dark, t.text, 0.15)) };
    clip.add(egui::Shape::closed_line(outline, Stroke::new(0.7 * k, stroke_col)));
    // play / dots
    clip.add(egui::Shape::convex_polygon(vec![at(86.0, 76.5), at(89.0, 78.5), at(86.0, 80.5)], line, Stroke::NONE));
    for x in [94.0, 100.0, 106.0, 112.0] {
        clip.circle_filled(at(x, 78.5), 1.2 * k, line);
    }
}

fn kind_of(t: &Theme) -> u8 {
    match t.fx {
        Fx::Glass => 1,
        Fx::Cyber => 2,
        Fx::None if t.hc => 3,
        Fx::None => 0,
    }
}

/// a filter chip with a count; returns true when clicked
fn chip(ui: &mut Ui, ic: Option<Icon>, label: &str, count: usize, on: bool) -> bool {
    let text = format!("{label}  {count}");
    let galley = ui.painter().layout_no_wrap(text.clone(), FontId::proportional(12.0), P.text);
    let w = galley.size().x + if ic.is_some() { 34.0 } else { 18.0 };
    let (r, resp) = ui.allocate_exact_size(vec2(w, 26.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, label));
    ui.painter().rect(r, CornerRadius::same(13), if on { P.accent_weak } else if resp.hovered() { P.panel2 } else { Color32::TRANSPARENT }, Stroke::new(1.0, if on { P.accent } else { P.border }), StrokeKind::Inside);
    let mut x = r.left() + 9.0;
    if let Some(ic) = ic {
        ui.painter().text(pos2(x, r.center().y), Align2::LEFT_CENTER, icon(ic), FontId::new(13.0, icons()), if on { P.accent } else { P.muted });
        x += 16.0;
    }
    ui.painter().galley(pos2(x, r.center().y - galley.size().y / 2.0), galley, if on { P.text } else { P.muted });
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

impl ShirtBuilderApp {
    pub(crate) fn theme_menu(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let active = crate::themes::theme(&self.active_theme_id(&ctx));
        ui.label(RichText::new("Appearance").family(semibold()).size(17.0));
        ui.label(RichText::new("Colours, text and accessibility.").size(12.0).color(P.muted));
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Theme").family(semibold()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| ui.label(RichText::new(format!("{} themes", THEMES.len())).size(11.0).color(P.muted)));
        });
        ui.horizontal(|ui| {
            let mut system = self.prefs.theme_system;
            if crate::widgets::seg(ui, &mut system, &[(false, "One theme"), (true, "Follow system")], false) {
                if system {
                    if active.dark {
                        self.prefs.theme_dark = active.id.into();
                    } else {
                        self.prefs.theme_light = active.id.into();
                    }
                } else {
                    self.prefs.theme = active.id.into();
                }
                self.prefs.theme_system = system;
            }
            if self.prefs.theme_system {
                ui.label(
                    RichText::new(format!("Light: {} · Dark: {}", crate::themes::theme(&self.prefs.theme_light).name, crate::themes::theme(&self.prefs.theme_dark).name)).size(11.0).color(P.muted),
                );
            }
        });
        ui.add_space(2.0);
        let count = |f: &dyn Fn(&Theme) -> bool| THEMES.iter().filter(|t| f(t)).count();
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            for (dark, ic, label) in [(false, Icon::Sun, "Light"), (true, Icon::Moon, "Dark")] {
                let on = self.popups.theme_mode == Some(dark);
                if chip(ui, Some(ic), label, count(&|t| t.dark == dark), on) {
                    self.popups.theme_mode = if on { None } else { Some(dark) };
                }
            }
            ui.add_space(6.0);
            for (k, ic, label) in [(0u8, None, "Standard"), (1, Some(Icon::Sparkles), "Liquid glass"), (2, Some(Icon::Grid3x3), "Cyberpunk"), (3, Some(Icon::Contrast), "High contrast")] {
                let on = self.popups.theme_kind == Some(k);
                if chip(ui, ic, label, count(&|t| kind_of(t) == k), on) {
                    self.popups.theme_kind = if on { None } else { Some(k) };
                }
            }
        });
        ui.add_space(6.0);
        let shown: Vec<&Theme> = THEMES.iter().filter(|t| self.popups.theme_mode.is_none_or(|d| t.dark == d) && self.popups.theme_kind.is_none_or(|k| kind_of(t) == k)).collect();
        let mut picked: Option<&Theme> = None;
        egui::ScrollArea::vertical().max_height(400.0).id_salt("theme-grid").show(ui, |ui| {
            if shown.is_empty() {
                ui.label(RichText::new("No themes match these filters.").color(P.muted));
            }
            let cols = 3;
            let gap = 10.0;
            let w = (ui.available_width() - gap * (cols as f32 - 1.0)) / cols as f32;
            for row in shown.chunks(cols) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for t in row {
                        let on = if self.prefs.theme_system { t.id == self.prefs.theme_light || t.id == self.prefs.theme_dark } else { t.id == self.prefs.theme };
                        let (r, resp) = ui.allocate_exact_size(vec2(w, w * 0.5625 + 38.0), Sense::click());
                        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, t.name));
                        let prev = Rect::from_min_size(r.min, vec2(w, w * 0.5625));
                        let accent = self.prefs.accent.as_deref().filter(|_| !t.hc && t.fx == Fx::None).unwrap_or(t.accent);
                        preview(ui.painter(), prev, t, accent);
                        let ring = if on { Stroke::new(2.0, P.accent) } else if resp.hovered() { Stroke::new(1.0, P.muted) } else { Stroke::new(1.0, P.border) };
                        ui.painter().rect_stroke(prev, CornerRadius::same(6), ring, StrokeKind::Outside);
                        // badges: mode (+ high contrast), check when chosen
                        let badge = Rect::from_min_size(prev.min + vec2(5.0, 5.0), vec2(if t.hc { 32.0 } else { 18.0 }, 16.0));
                        ui.painter().rect_filled(badge, CornerRadius::same(8), Color32::from_black_alpha(140));
                        ui.painter().text(badge.left_center() + vec2(9.0, 0.0), Align2::CENTER_CENTER, icon(if t.dark { Icon::Moon } else { Icon::Sun }), FontId::new(10.0, icons()), Color32::WHITE);
                        if t.hc {
                            ui.painter().text(badge.left_center() + vec2(23.0, 0.0), Align2::CENTER_CENTER, icon(Icon::Contrast), FontId::new(10.0, icons()), Color32::WHITE);
                        }
                        if on {
                            let cc = prev.right_top() + vec2(-11.0, 11.0);
                            ui.painter().circle_filled(cc, 8.0, P.accent);
                            ui.painter().text(cc, Align2::CENTER_CENTER, icon(Icon::Check), FontId::new(10.0, icons()), P.accent_ink);
                        }
                        ui.painter().text(pos2(r.left() + 2.0, prev.bottom() + 6.0), Align2::LEFT_TOP, t.name, FontId::new(12.5, semibold()), P.text);
                        let kind = if t.hc {
                            " · High contrast"
                        } else {
                            match t.fx {
                                Fx::Glass => " · Glass",
                                Fx::Cyber => " · Neon",
                                Fx::None => "",
                            }
                        };
                        let slot = if self.prefs.theme_system && on { if t.dark { " · Dark slot" } else { " · Light slot" } } else { "" };
                        ui.painter().text(pos2(r.left() + 2.0, prev.bottom() + 22.0), Align2::LEFT_TOP, format!("Built in{kind}{slot}"), FontId::proportional(11.0), P.muted);
                        if resp.on_hover_text(t.name).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                            picked = Some(t);
                        }
                    }
                });
                ui.add_space(8.0);
            }
        });
        if let Some(t) = picked {
            if self.prefs.theme_system {
                if t.dark {
                    self.prefs.theme_dark = t.id.into();
                } else {
                    self.prefs.theme_light = t.id.into();
                }
            } else {
                self.prefs.theme = t.id.into();
            }
        }
        ui.add_space(4.0);
        ui.label(RichText::new("Accent colour").family(semibold()));
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            // the theme's own accent
            let on = self.prefs.accent.is_none();
            let (r, resp) = ui.allocate_exact_size(vec2(72.0, 26.0), Sense::click());
            resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, "Theme accent"));
            ui.painter().rect(r, CornerRadius::same(13), if resp.hovered() { P.panel2 } else { Color32::TRANSPARENT }, Stroke::new(if on { 2.0 } else { 1.0 }, if on { P.accent } else { P.border }), StrokeKind::Inside);
            ui.painter().circle_filled(r.left_center() + vec2(14.0, 0.0), 6.0, c(active.accent));
            ui.painter().text(r.left_center() + vec2(26.0, 0.0), Align2::LEFT_CENTER, "Theme", FontId::proportional(12.0), P.text);
            if resp.on_hover_text("Use the theme's accent").clicked() {
                self.prefs.accent = None;
            }
            for a in ACCENTS {
                let on = self.prefs.accent.as_deref() == Some(a);
                let (r, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
                resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, format!("Accent {a}")));
                ui.painter().circle_filled(r.center(), 11.0, c(a));
                if on || resp.hovered() {
                    ui.painter().circle_stroke(r.center(), 13.0, Stroke::new(2.0, if on { P.text } else { P.muted }));
                }
                if resp.on_hover_text(a).clicked() {
                    self.prefs.accent = Some(a.into());
                }
            }
            let mut col = c(self.prefs.accent.as_deref().unwrap_or(active.accent));
            if ui.color_edit_button_srgba(&mut col).on_hover_text("Custom accent").changed() {
                self.prefs.accent = Some(format!("#{:02x}{:02x}{:02x}", col.r(), col.g(), col.b()));
            }
        });
        if active.hc || active.fx != Fx::None {
            ui.label(RichText::new("Glass, neon and high-contrast themes keep their own accent.").size(11.0).color(P.muted));
        }
    }
}
