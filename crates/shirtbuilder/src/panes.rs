//! The two work panes: sewing pattern (rulers, toolbox, zoom bar) and 3D preview chrome.

use crate::app::{ShirtBuilderApp, Tool, View3d};
use crate::pattern_view::ViewOptions;
use crate::style::{icon, icons, semibold, P};
use crate::topbar::popup;
use crate::widgets::seg;
use egui::{pos2, vec2, Align, Align2, Color32, CornerRadius, FontId, Layout, Painter, Rect, RichText, Sense, Stroke, StrokeKind, Ui, UiBuilder};
use lucide_icons::Icon;
use sb_core::doc::Side;
use sb_core::pattern::{garment, Unit};

const HEAD: f32 = 40.0;
const RULER: f32 = 20.0;

fn pane_head(ui: &mut Ui, rect: Rect, title: &str, sub: Option<&str>, right: impl FnOnce(&mut Ui)) {
    let head = Rect::from_min_size(rect.min, vec2(rect.width(), HEAD));
    ui.painter().rect_filled(head, 0.0, P.panel);
    ui.painter().hline(head.x_range(), head.bottom() - 0.5, Stroke::new(1.0, P.border));
    ui.scope_builder(UiBuilder::new().max_rect(head.shrink2(vec2(12.0, 0.0))).layout(Layout::left_to_right(Align::Center)), |ui| {
        ui.label(RichText::new(title).family(semibold()).size(13.0).color(P.accent));
        if let Some(s) = sub {
            ui.label(RichText::new(s).size(12.0).color(P.muted));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), right);
    });
}

/// a floating rounded panel (toolbox, zoom bar, chips)
fn float_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(P.panel)
        .stroke(Stroke::new(1.0, P.border))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::same(4))
        .shadow(egui::Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(90) })
}

fn tool_button(ui: &mut Ui, ic: Icon, on: bool, tip: &str) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(32.0, 30.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, tip));
    if on {
        ui.painter().rect_filled(r, CornerRadius::same(7), P.accent_strong);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, CornerRadius::same(7), P.panel2);
    }
    ui.painter().text(r.center(), Align2::CENTER_CENTER, icon(ic), FontId::new(16.0, icons()), if on { Color32::WHITE } else { P.text });
    resp.on_hover_text(tip).on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

fn text_button(ui: &mut Ui, label: &str, on: bool, w: f32) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(w, 26.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, label));
    if on {
        ui.painter().rect_filled(r, CornerRadius::same(6), P.accent_strong);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, CornerRadius::same(6), P.panel2);
    }
    ui.painter().text(r.center(), Align2::CENTER_CENTER, label, FontId::proportional(12.0), if on { Color32::WHITE } else { P.muted });
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

/// rulers along the top and left edge, in the chosen unit
fn rulers(p: &Painter, canvas: Rect, scale: f32, origin: egui::Vec2, unit: Unit) {
    let top = Rect::from_min_max(canvas.min, pos2(canvas.right(), canvas.top() + RULER));
    let left = Rect::from_min_max(canvas.min, pos2(canvas.left() + RULER, canvas.bottom()));
    p.rect_filled(top, 0.0, P.ruler);
    p.rect_filled(left, 0.0, P.ruler);
    // label spacing: the smallest "nice" step at least 50 px apart
    let per_unit = scale as f64 * unit.cm(); // px per unit
    let step = [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0].into_iter().find(|s| s * per_unit >= 50.0).unwrap_or(1000.0);
    let minor = step / if step >= 10.0 && (step as i64) % 10 == 0 { 10.0 } else { 5.0 };
    let font = FontId::proportional(9.5);
    for (horizontal, len, o, start) in [(true, canvas.width(), origin.x, canvas.left()), (false, canvas.height(), origin.y, canvas.top())] {
        let first = ((-o as f64) / per_unit / minor).floor() * minor;
        let mut v = first;
        while (o as f64 + v * per_unit) < len as f64 {
            let at = start + o + (v * per_unit) as f32;
            let major = (v / step).round() * step == v || ((v / step) - (v / step).round()).abs() < 1e-6;
            let tick = if major { 8.0 } else { 4.0 };
            if horizontal && at > canvas.left() + RULER {
                p.line_segment([pos2(at, top.bottom() - tick), pos2(at, top.bottom())], Stroke::new(1.0, P.ruler_tick));
                if major {
                    p.text(pos2(at + 3.0, top.top() + 2.0), Align2::LEFT_TOP, format!("{}", v.round() as i64), font.clone(), P.muted);
                }
            } else if !horizontal && at > canvas.top() + RULER {
                p.line_segment([pos2(left.right() - tick, at), pos2(left.right(), at)], Stroke::new(1.0, P.ruler_tick));
                if major {
                    // vertical label, written top-to-bottom like the web app's
                    let galley = p.layout_no_wrap(format!("{}", v.round() as i64), font.clone(), P.muted);
                    let mut shape = egui::epaint::TextShape::new(pos2(left.left() + 3.0, at - 3.0), galley, P.muted);
                    shape.angle = -std::f32::consts::FRAC_PI_2;
                    p.add(shape);
                }
            }
            v += minor;
        }
    }
    p.hline(top.x_range(), top.bottom(), Stroke::new(1.0, P.border));
    p.vline(left.right(), left.y_range(), Stroke::new(1.0, P.border));
    let corner = Rect::from_min_size(canvas.min, vec2(RULER, RULER));
    p.rect_filled(corner, 0.0, P.ruler);
    p.text(corner.center(), Align2::CENTER_CENTER, unit.label(), FontId::proportional(9.0), P.muted);
}

impl ShirtBuilderApp {
    pub(crate) fn pattern_pane(&mut self, ui: &mut Ui, rect: Rect, now: f64) {
        let g = garment(self.doc.garment);
        let pieces = format!("{} pieces", g.pieces.len());
        let mut side = self.side;
        pane_head(ui, rect, "Sewing pattern", Some(&pieces), |ui| {
            seg(ui, &mut side, &[(Side::Inside, "Inside"), (Side::Outside, "Outside")], true);
        });
        // keep the segment order Outside · Inside on screen (right-to-left layout reverses it)
        if side != self.side {
            self.side = side;
            self.selected = None;
        }

        let body = Rect::from_min_max(pos2(rect.left(), rect.top() + HEAD), rect.max);
        let canvas = Rect::from_min_max(body.min + vec2(RULER, RULER), body.max);
        let opt = ViewOptions {
            side: self.side,
            unit: self.prefs.unit,
            seam_allowance: self.prefs.seam_allowance,
            grid: self.prefs.grid,
            tool: self.tool,
            selected: self.selected.as_deref(),
            palette: &*P,
            atlas: self.viewer.atlas.filter(|a| a.garment == self.doc.garment).map(|a| a.ids[if self.side == Side::Inside { 1 } else { 0 }]),
        };
        let ev = self.view.show(ui, canvas, &self.doc, &g, &opt);
        if let Some(sel) = ev.select {
            self.selected = sel;
        }
        if let Some((l, key)) = ev.set_layer {
            self.edit(now, Some(&key), |d| {
                if let Some(x) = d.layers.iter_mut().find(|x| x.id == l.id) {
                    *x = l;
                }
            });
        }
        if let Some(id) = ev.dropped {
            // dropped onto another piece → the layer moves to that piece (same place on the sheet)
            let gg = garment(self.doc.garment);
            if let Some(l) = self.doc.layers.iter().find(|l| l.id == id).cloned() {
                if let Some(p) = gg.piece(l.piece) {
                    let outline = sb_print::draw::view_outline(p, l.side);
                    if !sb_core::geom::point_in_poly(sb_core::v2(l.x, l.y), &outline) {
                        let (wx, wy) = (p.x + l.x, p.y + l.y);
                        let np = gg.pieces.iter().find(|q| {
                            let probe = sb_core::v2(wx - q.x, wy - q.y);
                            let probe = if l.side == Side::Inside { sb_core::v2(q.w - probe.x, probe.y) } else { probe };
                            sb_core::geom::point_in_poly(probe, &q.outline)
                        });
                        if let Some(np) = np.filter(|np| np.id != p.id) {
                            let (pid, nx, ny) = (np.id, wx - np.x, wy - np.y);
                            self.edit(now, Some(&format!("move-{id}")), |d| {
                                if let Some(x) = d.layers.iter_mut().find(|x| x.id == id) {
                                    x.piece = pid;
                                    x.x = nx;
                                    x.y = ny;
                                }
                            });
                            self.say(format!("Moved to the {}", np.name.to_lowercase()), now);
                        }
                    }
                }
            }
        }
        if let Some(at) = ev.place {
            match self.tool {
                Tool::Text => self.add_text(now, Some(at)),
                Tool::Shape => self.add_shape(now, Some(at)),
                Tool::Image => self.pick_image(now, Some(at)),
                _ => {}
            }
        }
        // rulers sit over the canvas edges
        let painter = ui.painter_at(body);
        rulers(&painter, Rect::from_min_max(body.min, body.max), self.view.scale(), self.view.origin() + vec2(RULER, RULER), self.prefs.unit);

        // ---- floating toolbox (tools + units)
        let tb_rect = Rect::from_min_size(canvas.min + vec2(10.0, 14.0), vec2(44.0, 300.0));
        ui.scope_builder(UiBuilder::new().max_rect(tb_rect).layout(Layout::top_down(Align::Center)), |ui| {
            float_frame().show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                for (t, ic, tip) in [
                    (Tool::Select, Icon::MousePointer2, "Select & move (V)"),
                    (Tool::Hand, Icon::Hand, "Pan (H)"),
                    (Tool::Image, Icon::Image, "Image (I) — click a piece"),
                    (Tool::Text, Icon::Type, "Text (T) — click a piece"),
                    (Tool::Shape, Icon::Square, "Shape (R) — click a piece"),
                ] {
                    if tool_button(ui, ic, self.tool == t, tip) {
                        self.tool = t;
                    }
                }
                ui.add(egui::Separator::default().spacing(6.0));
                for u in [Unit::Mm, Unit::Cm, Unit::In] {
                    if text_button(ui, u.label(), self.prefs.unit == u, 32.0) {
                        self.prefs.unit = u;
                    }
                }
            });
        });

        // ---- zoom bar
        let zb = Rect::from_center_size(pos2(canvas.center().x, canvas.bottom() - 30.0), vec2(190.0, 40.0));
        ui.scope_builder(UiBuilder::new().max_rect(zb).layout(Layout::left_to_right(Align::Center)), |ui| {
            float_frame().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    if text_button(ui, "−", false, 28.0) {
                        self.view.zoom_by(1.0 / 1.2);
                    }
                    ui.add_sized([46.0, 26.0], egui::Label::new(RichText::new(format!("{:.0}%", self.view.zoom_percent())).size(12.5)));
                    if text_button(ui, "+", false, 28.0) {
                        self.view.zoom_by(1.2);
                    }
                    if text_button(ui, "Fit", false, 34.0) {
                        self.view.request_fit();
                    }
                    let grid = self.prefs.grid;
                    let (r, resp) = ui.allocate_exact_size(vec2(28.0, 26.0), Sense::click());
                    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, grid, "Grid"));
                    if grid {
                        ui.painter().rect_filled(r, CornerRadius::same(6), P.accent_strong);
                    }
                    ui.painter().text(r.center(), Align2::CENTER_CENTER, icon(Icon::Grip), FontId::new(14.0, icons()), if grid { Color32::WHITE } else { P.muted });
                    if resp.on_hover_text("Grid").clicked() {
                        self.prefs.grid = !grid;
                    }
                });
            });
        });
    }

    pub(crate) fn viewer_pane(&mut self, ui: &mut Ui, rect: Rect, now: f64) {
        let mut v = self.view3d;
        let mut clicked_view = false;
        pane_head(ui, rect, "3D Preview", None, |ui| {
            clicked_view = seg(ui, &mut v, &[(View3d::Back, "Back"), (View3d::Side, "Side"), (View3d::Angle, "¾"), (View3d::Front, "Front")], false);
        });
        if clicked_view || v != self.view3d {
            self.view3d = v;
            self.viewer.set_view(v, now);
        }
        let body = Rect::from_min_max(pos2(rect.left(), rect.top() + HEAD), rect.max);
        // the shaded shirt over its backdrop (handles orbit / zoom / pan itself)
        self.viewer.show(ui, body, &self.doc, self.prefs.quality);
        let painter = ui.painter_at(body);

        // floating garment picker (top right)
        let g = garment(self.doc.garment);
        let gp = Rect::from_min_size(pos2(body.right() - 232.0, body.top() + 12.0), vec2(220.0, 44.0));
        let resp = ui.interact(gp, ui.scope_id().with("garment-float"), Sense::click());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Choose garment"));
        painter.rect(gp, CornerRadius::same(10), if resp.hovered() { P.panel2 } else { P.chip }, Stroke::new(1.0, Color32::from_white_alpha(20)), StrokeKind::Inside);
        painter.text(gp.left_center() + vec2(14.0, 0.0), Align2::LEFT_CENTER, icon(Icon::Shirt), FontId::new(18.0, icons()), P.text);
        painter.text(pos2(gp.left() + 42.0, gp.top() + 9.0), Align2::LEFT_TOP, g.spec.title.to_uppercase(), FontId::new(11.0, semibold()), P.text);
        painter.text(pos2(gp.left() + 42.0, gp.bottom() - 9.0), Align2::LEFT_BOTTOM, g.spec.subtitle.to_uppercase(), FontId::proportional(9.5), P.accent);
        painter.text(gp.right_center() - vec2(14.0, 0.0), Align2::RIGHT_CENTER, icon(Icon::ChevronDown), FontId::new(14.0, icons()), P.muted);
        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            self.popups.garment_float = !self.popups.garment_float;
        }
        let ctx = ui.ctx().clone();
        let mut open = self.popups.garment_float;
        popup(&ctx, "garment-float", gp, true, &mut open, 260.0, |ui| self.garment_menu(ui, now));
        self.popups.garment_float = open;

        // Live preview chip (bottom left), with the frame rate when asked for
        let label = if self.prefs.show_fps {
            let fps = if self.viewer.fps > 0.0 { format!("{:.0} fps", self.viewer.fps) } else { "rotate to measure".into() };
            format!("Live preview  ·  {fps} · {}", self.viewer.level.name())
        } else {
            "Live preview".to_string()
        };
        let galley = painter.layout_no_wrap(label, FontId::proportional(11.5), P.text);
        let live = Rect::from_min_size(pos2(body.left() + 12.0, body.bottom() - 40.0), vec2(galley.size().x + 34.0, 26.0));
        painter.rect_filled(live, CornerRadius::same(13), P.chip);
        painter.circle_filled(live.left_center() + vec2(13.0, 0.0), 3.5, P.accent);
        painter.galley(pos2(live.left() + 23.0, live.center().y - galley.size().y / 2.0), galley, P.text);
        let mut tag_top = body.top() + 12.0;
        if let Some(name) = self.viewer.active_scan() {
            let tag = if name == "tee-f" { "Realistic model · female tee" } else { "Realistic model · male tee" };
            let g2 = painter.layout_no_wrap(tag.into(), FontId::proportional(11.0), P.muted);
            let r = Rect::from_min_size(pos2(body.left() + 12.0, tag_top), g2.size() + vec2(20.0, 10.0));
            painter.rect_filled(r, CornerRadius::same(12), P.chip);
            painter.galley(r.min + vec2(10.0, 5.0), g2, P.muted);
            tag_top = r.bottom() + 6.0;
        }
        // not on the graphics card: say so where the slowness shows, and lead to Scene › Graphics card
        if self.gpu.on_cpu() {
            let amber = Color32::from_rgb(0xe0, 0xa8, 0x4a);
            let text = if self.gpu.problem.is_some() { "Graphics card unavailable — restart the computer" } else { "Drawing on the CPU, not the graphics card" };
            let g3 = painter.layout_no_wrap(text.into(), FontId::proportional(11.0), amber);
            let r = Rect::from_min_size(pos2(body.left() + 12.0, tag_top), g3.size() + vec2(38.0, 10.0));
            let resp = ui.interact(r, egui::Id::unique("gpu-warning"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
            painter.rect(r, CornerRadius::same(12), if resp.hovered() { P.panel2 } else { P.chip }, Stroke::new(1.0, amber.gamma_multiply(0.6)), StrokeKind::Inside);
            painter.text(pos2(r.left() + 18.0, r.center().y), Align2::CENTER_CENTER, icon(Icon::TriangleAlert), FontId::new(12.0, icons()), amber);
            painter.galley(r.min + vec2(28.0, 5.0), g3, amber);
            let tip = self.gpu.problem.clone().unwrap_or_else(|| "The 3D is slow because no graphics card driver could be used. Click for Scene › Graphics card.".into());
            if resp.on_hover_text(tip).clicked() {
                self.prefs.tab = crate::app::Tab::Scene;
            }
        }

        // view bar (bottom centre): auto-rotate · Reset view · hanger
        let vb = Rect::from_center_size(pos2(body.center().x, body.bottom() - 30.0), vec2(160.0, 38.0));
        let mut scene = self.doc.scene.clone();
        let mut reset = false;
        ui.scope_builder(UiBuilder::new().max_rect(vb).layout(Layout::left_to_right(Align::Center)), |ui| {
            float_frame().fill(P.chip).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    if tool_button(ui, Icon::RotateCcw, scene.auto_rotate, "Auto-rotate") {
                        scene.auto_rotate = !scene.auto_rotate;
                    }
                    reset = text_button(ui, "Reset view", false, 78.0);
                    if tool_button(ui, Icon::Shirt, scene.hanger, if scene.hanger { "On a hanger — click for ghost mannequin" } else { "Ghost mannequin — click to hang on a hanger" }) {
                        scene.hanger = !scene.hanger;
                    }
                });
            });
        });
        if reset {
            self.view3d = View3d::Front;
            self.viewer.reset_view(&self.doc);
        }
        if scene != self.doc.scene {
            self.edit(now, None, |d| d.scene = scene);
        }

        // navigation gizmo (bottom right)
        self.viewer.gizmo(ui, pos2(body.right() - 66.0, body.bottom() - 66.0), &self.doc);
    }
}
