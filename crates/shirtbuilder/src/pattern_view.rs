//! The flat sewing-pattern canvas: pieces with their print (the same atlas the 3D preview uses:
//! fabric, details, text and image artwork), stitch and cut lines, notches, grain lines, labels —
//! select, move, scale, rotate, place, pan and zoom.

use crate::app::Tool;
use crate::style::Palette;
use crate::tess::triangulate;
use egui::{pos2, vec2, Color32, CornerRadius, FontId, PointerButton, Pos2, Rect, Sense, Shape, Stroke, Vec2};
use sb_core::cutting::{cut_outline, notch_segments, notches, Notch, SEAM};
use sb_core::doc::{Doc, Layer, Side};
use sb_core::geom::point_in_poly;
use sb_core::pattern::{fmt_len, Garment, GarmentId, Piece, PieceId, PieceKind, Unit};
use sb_core::{v2, V2};
use std::collections::HashMap;

pub struct PatternView {
    /// screen px per cm
    scale: f32,
    /// screen position of the sheet origin, relative to the canvas top-left
    origin: Vec2,
    /// garment the view was last fitted to (refits when the garment changes)
    fitted: Option<GarmentId>,
    canvas: Rect,
    fills: HashMap<(GarmentId, PieceId, bool), Vec<[u32; 3]>>,
    cuts: HashMap<(GarmentId, PieceId), Vec<V2>>,
    drag: Option<Drag>,
    /// x (sheet cm) of the centre guide shown while snapping
    guide: Option<f32>,
}

#[derive(Clone, Debug, PartialEq)]
enum DragKind {
    Move,
    Scale,
    Rotate,
}

#[derive(Clone)]
struct Drag {
    kind: DragKind,
    orig: Layer,
    /// pointer at the start, sheet cm
    start: V2,
}

impl Default for PatternView {
    fn default() -> Self {
        PatternView {
            scale: 5.0,
            origin: vec2(40.0, 40.0),
            fitted: None,
            canvas: Rect::NOTHING,
            fills: HashMap::new(),
            cuts: HashMap::new(),
            drag: None,
            guide: None,
        }
    }
}

pub struct ViewOptions<'a> {
    pub side: Side,
    pub unit: Unit,
    pub seam_allowance: bool,
    pub grid: bool,
    pub tool: Tool,
    pub selected: Option<&'a str>,
    pub palette: &'a Palette,
    /// the print atlas for this side (when it matches the garment)
    pub atlas: Option<egui::TextureId>,
}

/// what the user did on the canvas this frame
#[derive(Default)]
pub struct ViewEvent {
    /// Some(None) = clicked empty space (deselect)
    pub select: Option<Option<String>>,
    /// a layer changed by dragging (moved, scaled or rotated) — merged into one undo step per key
    pub set_layer: Option<(Layer, String)>,
    /// a move ended: the layer may have been dropped onto another piece
    pub dropped: Option<String>,
    /// place new artwork (Text / Shape tool) at piece-local cm
    pub place: Option<(PieceId, Side, f64, f64)>,
}

fn hex(c: &str) -> Color32 {
    crate::widgets::hex_color(c)
}

fn shade(c: Color32, k: f32) -> Color32 {
    Color32::from_rgb((c.r() as f32 * k) as u8, (c.g() as f32 * k) as u8, (c.b() as f32 * k) as u8)
}

impl PatternView {
    pub fn request_fit(&mut self) {
        self.fitted = None;
    }
    pub fn scale(&self) -> f32 {
        self.scale
    }
    pub fn origin(&self) -> Vec2 {
        self.origin
    }
    /// zoom in / out around the middle of the canvas
    pub fn zoom_by(&mut self, factor: f32) {
        let c = self.canvas.size() / 2.0;
        self.zoom_about(c, factor);
    }
    fn zoom_about(&mut self, local: Vec2, factor: f32) {
        let new_scale = (self.scale * factor).clamp(0.8, 80.0);
        let k = new_scale / self.scale;
        self.origin = local - (local - self.origin) * k;
        self.scale = new_scale;
    }
    /// zoom readout: 100 % = 4 px per cm
    pub fn zoom_percent(&self) -> f32 {
        self.scale / 4.0 * 100.0
    }

    fn fit(&mut self, g: &Garment, size: Vec2) {
        let m = 60.0;
        let s = ((size.x - 2.0 * m) / g.w as f32).min((size.y - 2.0 * m) / g.h as f32).max(0.5);
        self.scale = s;
        self.origin = vec2((size.x - g.w as f32 * s) / 2.0, (size.y - g.h as f32 * s) / 2.0 + 6.0);
    }

    /// layer under a canvas point (topmost first), in its piece's view frame
    fn hit_layer<'d>(&self, doc: &'d Doc, g: &Garment, side: Side, p: Pos2) -> Option<&'d Layer> {
        let sheet = (p - self.canvas.min - self.origin) / self.scale;
        doc.layers.iter().rev().filter(|l| l.side == side && l.visible).find(|l| {
            let Some(pc) = g.piece(l.piece) else { return false };
            let q = v2(sheet.x as f64 - pc.x - l.x, sheet.y as f64 - pc.y - l.y);
            let (s, c) = (-l.rot).to_radians().sin_cos();
            let (rx, ry) = (q.x * c - q.y * s, q.x * s + q.y * c);
            rx.abs() <= l.w / 2.0 && ry.abs() <= l.h / 2.0
        })
    }

    fn to_sheet(&self, p: Pos2) -> V2 {
        let v = (p - self.canvas.min - self.origin) / self.scale;
        v2(v.x as f64, v.y as f64)
    }

    /// screen corners and rotation knob of a layer
    pub(crate) fn handles(&self, g: &Garment, l: &Layer) -> Option<([Pos2; 4], Pos2)> {
        let p = g.piece(l.piece)?;
        let o = self.canvas.min + self.origin;
        let (sin, cos) = l.rot.to_radians().sin_cos();
        let corner = |a: f64, b: f64| {
            let (dx, dy) = (a * l.w / 2.0, b * l.h / 2.0);
            o + vec2((p.x + l.x + dx * cos - dy * sin) as f32 * self.scale, (p.y + l.y + dx * sin + dy * cos) as f32 * self.scale)
        };
        let cs = [corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0)];
        let top = pos2((cs[0].x + cs[1].x) / 2.0, (cs[0].y + cs[1].y) / 2.0);
        let up = (top - pos2((cs[2].x + cs[3].x) / 2.0, (cs[2].y + cs[3].y) / 2.0)).normalized();
        Some((cs, top + up * 22.0))
    }

    /// a handle of the selected layer under the pointer
    fn hit_handle(&self, doc: &Doc, g: &Garment, sel: Option<&str>, p: Pos2) -> Option<(Layer, DragKind)> {
        let l = doc.layers.iter().find(|l| Some(l.id.as_str()) == sel)?;
        let (cs, knob) = self.handles(g, l)?;
        if knob.distance(p) <= 10.0 {
            return Some((l.clone(), DragKind::Rotate));
        }
        cs.iter().any(|c| c.distance(p) <= 9.0).then(|| (l.clone(), DragKind::Scale))
    }

    /// piece under a canvas point, with the piece-local (view-frame) position
    fn hit_piece(&self, g: &Garment, side: Side, p: Pos2) -> Option<(PieceId, f64, f64)> {
        let sheet = (p - self.canvas.min - self.origin) / self.scale;
        g.pieces.iter().find_map(|pc| {
            let q = v2(sheet.x as f64 - pc.x, sheet.y as f64 - pc.y);
            let probe = if side == Side::Inside { v2(pc.w - q.x, q.y) } else { q };
            point_in_poly(probe, &pc.outline).then_some((pc.id, q.x, q.y))
        })
    }

    pub fn show(&mut self, ui: &mut egui::Ui, canvas: Rect, doc: &Doc, g: &Garment, opt: &ViewOptions) -> ViewEvent {
        self.canvas = canvas;
        let mut ev = ViewEvent::default();
        let pal = opt.palette;
        if self.fitted != Some(doc.garment) {
            self.fit(g, canvas.size());
            self.fitted = Some(doc.garment);
        }
        let resp = ui.interact(canvas, ui.scope_id().with("pattern-canvas"), Sense::click_and_drag());

        // ---- interaction (web PatternView: handles scale / rotate, bodies move, empty space pans)
        let pan_tool = opt.tool == Tool::Hand;
        let alt = ui.input(|i| i.modifiers.alt);
        let shift = ui.input(|i| i.modifiers.shift);
        if resp.drag_started_by(PointerButton::Primary) && !pan_tool && opt.tool == Tool::Select {
            self.drag = None;
            // what to drag is decided where the button went down: egui reports the drag only after
            // the pointer has travelled a few pixels, by when it may have left a small handle
            if let Some(pp) = ui.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos()) {
                let start = self.to_sheet(pp);
                if let Some((l, kind)) = self.hit_handle(doc, g, opt.selected, pp) {
                    self.drag = Some(Drag { kind, orig: l, start });
                } else if let Some(l) = self.hit_layer(doc, g, opt.side, pp) {
                    ev.select = Some(Some(l.id.clone()));
                    self.drag = Some(Drag { kind: DragKind::Move, orig: l.clone(), start });
                }
            }
        }
        if resp.dragged() {
            match (&self.drag, resp.dragged_by(PointerButton::Primary) && !pan_tool, resp.interact_pointer_pos()) {
                (Some(d), true, Some(pp)) => {
                    let w = self.to_sheet(pp);
                    let mut l = d.orig.clone();
                    let p = g.piece(l.piece);
                    self.guide = None;
                    match (d.kind.clone(), p) {
                        (DragKind::Move, Some(p)) => {
                            l.x = d.orig.x + (w.x - d.start.x);
                            l.y = d.orig.y + (w.y - d.start.y);
                            // snap to the piece's centre line
                            if !alt && ((l.x - p.w / 2.0) * self.scale as f64).abs() < 6.0 {
                                l.x = p.w / 2.0;
                                self.guide = Some((p.x + p.w / 2.0) as f32);
                            }
                        }
                        (DragKind::Scale, Some(p)) => {
                            let c = v2(p.x + d.orig.x, p.y + d.orig.y);
                            let (s, co) = (-d.orig.rot).to_radians().sin_cos();
                            let (vx, vy) = (w.x - c.x, w.y - c.y);
                            let (lx, ly) = (vx * co - vy * s, vx * s + vy * co);
                            if shift {
                                l.w = (lx.abs() * 2.0).max(0.3);
                                l.h = (ly.abs() * 2.0).max(0.3);
                            } else {
                                let (hw, hh) = (d.orig.w / 2.0, d.orig.h / 2.0);
                                let k = ((lx.abs() * hw + ly.abs() * hh) / (hw * hw + hh * hh)).max(0.02);
                                l.w = d.orig.w * k;
                                l.h = d.orig.h * k;
                            }
                        }
                        (DragKind::Rotate, Some(p)) => {
                            let c = v2(p.x + d.orig.x, p.y + d.orig.y);
                            let mut a = (w.y - c.y).atan2(w.x - c.x).to_degrees() + 90.0;
                            if a > 180.0 {
                                a -= 360.0;
                            }
                            if shift {
                                a = (a / 15.0).round() * 15.0;
                            } else {
                                let near = (a / 45.0).round() * 45.0;
                                if (a - near).abs() < 3.0 {
                                    a = near;
                                }
                            }
                            l.rot = (a * 10.0).round() / 10.0;
                        }
                        _ => {}
                    }
                    let key = format!("{}-{}", match d.kind { DragKind::Move => "move", DragKind::Scale => "scale", DragKind::Rotate => "rotate" }, l.id);
                    ev.set_layer = Some((l, key));
                }
                _ => self.origin += resp.drag_delta(),
            }
        }
        if resp.drag_stopped() {
            if let Some(d) = self.drag.take() {
                if d.kind == DragKind::Move {
                    ev.dropped = Some(d.orig.id.clone());
                }
            }
            self.guide = None;
        }
        if resp.clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                match opt.tool {
                    Tool::Text | Tool::Shape | Tool::Image => {
                        if let Some((piece, x, y)) = self.hit_piece(g, opt.side, p) {
                            ev.place = Some((piece, opt.side, x, y));
                        }
                    }
                    Tool::Select => ev.select = Some(self.hit_layer(doc, g, opt.side, p).map(|l| l.id.clone())),
                    _ => {}
                }
            }
        }
        if resp.double_clicked() && resp.interact_pointer_pos().and_then(|p| self.hit_layer(doc, g, opt.side, p)).is_none() {
            self.fit(g, canvas.size());
        }
        if resp.hovered() {
            let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let factor = zoom * (scroll * 0.0015).exp();
            if (factor - 1.0).abs() > 1e-4 {
                if let Some(p) = resp.hover_pos() {
                    self.zoom_about(p - canvas.min, factor);
                }
            }
            let hp = resp.hover_pos();
            let cursor = match opt.tool {
                Tool::Hand => egui::CursorIcon::Grab,
                Tool::Text | Tool::Shape | Tool::Image => {
                    if hp.and_then(|p| self.hit_piece(g, opt.side, p)).is_some() {
                        egui::CursorIcon::Copy
                    } else {
                        egui::CursorIcon::NotAllowed
                    }
                }
                Tool::Select => match hp.and_then(|p| self.hit_handle(doc, g, opt.selected, p)) {
                    Some((_, DragKind::Rotate)) => egui::CursorIcon::Crosshair,
                    Some(_) => egui::CursorIcon::ResizeNwSe,
                    None if hp.and_then(|p| self.hit_layer(doc, g, opt.side, p)).is_some() => egui::CursorIcon::Move,
                    None => egui::CursorIcon::Default,
                },
            };
            ui.ctx().set_cursor_icon(if resp.dragged() && self.drag.is_none() { egui::CursorIcon::Grabbing } else { cursor });
        }

        // ---- drawing
        let painter = ui.painter_at(canvas);
        painter.rect_filled(canvas, 0.0, pal.canvas);
        let s = self.scale;
        let o = canvas.min + self.origin;
        let to = |p: &Piece, q: V2| -> Pos2 { o + vec2((p.x + q.x) as f32 * s, (p.y + q.y) as f32 * s) };
        let inside = opt.side == Side::Inside;
        let view = |p: &Piece, q: V2| if inside { v2(p.w - q.x, q.y) } else { q };

        if opt.grid {
            let step = if s * 5.0 >= 14.0 { 5.0 } else if s * 10.0 >= 14.0 { 10.0 } else { 20.0 };
            let x0 = ((-self.origin.x) / s / step).floor() * step;
            let y0 = ((-self.origin.y) / s / step).floor() * step;
            let mut x = x0;
            while o.x + x * s < canvas.right() {
                let mut y = y0;
                while o.y + y * s < canvas.bottom() {
                    painter.circle_filled(o + vec2(x * s, y * s), 1.0, Color32::from_white_alpha(26));
                    y += step;
                }
                x += step;
            }
        }

        let fabric = hex(&doc.fabric.color);
        let fill = if inside { shade(fabric, 0.9) } else { fabric };
        let line = Stroke::new(1.0, pal.piece_line);
        let (ax0, ay0, aw, ah) = sb_core::pattern::atlas_rect(g);

        for p in &g.pieces {
            let pts: Vec<V2> = p.outline.iter().map(|q| view(p, *q)).collect();
            let screen: Vec<Pos2> = pts.iter().map(|q| to(p, *q)).collect();
            let tris = self.fills.entry((doc.garment, p.id, inside)).or_insert_with(|| triangulate(&pts));
            let mut mesh = match opt.atlas {
                Some(t) => egui::Mesh::with_texture(t),
                None => egui::Mesh::default(),
            };
            for (q, v) in screen.iter().zip(&pts) {
                // the atlas is in outside orientation: undo the inside view's mirror
                let a = view(p, *v);
                let uv = pos2(((p.x + a.x - ax0) / aw) as f32, ((p.y + a.y - ay0) / ah) as f32);
                mesh.vertices.push(egui::epaint::Vertex { pos: *q, uv, color: if opt.atlas.is_some() { Color32::WHITE } else { fill } });
            }
            for t in tris.iter() {
                mesh.add_triangle(t[0], t[1], t[2]);
            }
            painter.add(Shape::mesh(mesh));
            painter.add(Shape::closed_line(screen, line));
            self.editor_marks(&painter, p, g, doc, inside, &to, &view);

            if !p.kind.is_trim() {
                let a = to(p, v2(p.w / 2.0, if p.kind == PieceKind::Body { 12.0 } else { 3.0 }));
                let b = to(p, v2(p.w / 2.0, p.h - 3.0));
                painter.extend(Shape::dashed_line(&[a, b], Stroke::new(1.0, Color32::from_white_alpha(56)), 4.0, 4.0));
            }

            if opt.seam_allowance {
                let cut = self.cuts.entry((doc.garment, p.id)).or_insert_with(|| cut_outline(p, g));
                let mut cs: Vec<Pos2> = cut.iter().map(|q| to(p, view(p, *q))).collect();
                if let Some(first) = cs.first().copied() {
                    cs.push(first);
                }
                painter.extend(Shape::dashed_line(&cs, Stroke::new(1.0, pal.piece_line), 3.0, 3.0));
                for nt in notches(p, g) {
                    let nt = if inside { Notch { p: view(p, nt.p), n: v2(-nt.n.x, nt.n.y), t: v2(-nt.t.x, nt.t.y), count: nt.count } } else { nt };
                    for (a, b) in notch_segments(&nt, SEAM) {
                        painter.line_segment([to(p, a), to(p, b)], Stroke::new(1.4, pal.piece_line));
                    }
                }
            }

            let cx = to(p, v2(p.w / 2.0, 0.0)).x;
            let dims = format!("{} × {} {} · cut {}", fmt_len(p.w, opt.unit, None), fmt_len(p.h, opt.unit, None), opt.unit.label(), p.cut);
            if p.kind.is_trim() {
                let y = to(p, v2(0.0, if opt.seam_allowance { -1.0 } else { 0.0 })).y - 6.0;
                painter.text(pos2(cx, y), egui::Align2::CENTER_BOTTOM, &p.name, FontId::proportional(11.0), pal.text);
            } else {
                let extra = if opt.seam_allowance { if p.kind == PieceKind::Body { 2.5 } else { 2.0 } } else { 0.0 };
                let y = to(p, v2(0.0, p.h + extra)).y;
                painter.text(pos2(cx, y + 8.0), egui::Align2::CENTER_TOP, &p.name, FontId::proportional(11.0), pal.text);
                painter.text(pos2(cx, y + 22.0), egui::Align2::CENTER_TOP, dims, FontId::proportional(10.0), pal.muted);
            }
        }

        // artwork is in the print itself: here only the selection box / handles and a hover outline
        let hover_id = canvas_hover(ui, canvas).and_then(|hp| self.hit_layer(doc, g, opt.side, hp)).map(|l| l.id.clone());
        for l in doc.layers.iter().filter(|l| l.side == opt.side && l.visible) {
            let Some((cs, knob)) = self.handles(g, l) else { continue };
            let selected = opt.selected == Some(l.id.as_str());
            let mut ring = cs.to_vec();
            ring.push(cs[0]);
            if selected {
                painter.add(Shape::line(ring, Stroke::new(1.25, pal.accent)));
                for c in cs {
                    let r = Rect::from_center_size(c, Vec2::splat(8.0));
                    painter.rect(r, CornerRadius::ZERO, Color32::WHITE, Stroke::new(1.0, pal.accent), egui::StrokeKind::Middle);
                }
                let top = pos2((cs[0].x + cs[1].x) / 2.0, (cs[0].y + cs[1].y) / 2.0);
                painter.line_segment([top, knob], Stroke::new(1.25, pal.accent));
                painter.circle(knob, 5.0, Color32::WHITE, Stroke::new(1.0, pal.accent));
                let size = format!("{} × {} {}", fmt_len(l.w, opt.unit, Some(1)), fmt_len(l.h, opt.unit, Some(1)), opt.unit.label());
                let bottom = cs.iter().copied().fold(cs[0], |a, b| if b.y > a.y { b } else { a });
                let galley = painter.layout_no_wrap(size, FontId::proportional(10.0), Color32::WHITE);
                let r = Rect::from_center_size(bottom + vec2(0.0, 16.0), galley.size() + vec2(10.0, 4.0));
                painter.rect_filled(r, CornerRadius::same(4), pal.accent);
                painter.galley(r.min + vec2(5.0, 2.0), galley, Color32::WHITE);
            } else if hover_id.as_deref() == Some(l.id.as_str()) || opt.atlas.is_none() {
                painter.extend(Shape::dashed_line(&ring, Stroke::new(1.0, pal.accent.gamma_multiply(0.7)), 5.0, 3.0));
            }
        }
        if let Some(gx) = self.guide {
            let x = o.x + gx * s;
            painter.vline(x, canvas.y_range(), Stroke::new(1.0, Color32::from_rgb(0xff, 0x4f, 0x9a)));
        }

        if inside {
            let badge = "Inside view — mirrored. Prints here appear inside the garment.";
            let galley = painter.layout_no_wrap(badge.into(), FontId::proportional(11.5), pal.text);
            let r = Rect::from_center_size(canvas.center_top() + vec2(0.0, 36.0), galley.size() + vec2(20.0, 8.0));
            painter.rect_filled(r, CornerRadius::same(12), pal.chip);
            painter.galley(r.min + vec2(10.0, 4.0), galley, pal.text);
        }
        ev
    }

    /// marks only the flat pattern shows (the 3D has real ones): rib / collar fold lines and the
    /// FOLD label, and the buttons on the placket
    #[allow(clippy::too_many_arguments)]
    fn editor_marks(&self, painter: &egui::Painter, p: &Piece, g: &Garment, doc: &Doc, inside: bool, to: &dyn Fn(&Piece, V2) -> Pos2, view: &dyn Fn(&Piece, V2) -> V2) {
        let s = &g.spec;
        let dark = sb_print::color::luminance(&doc.fabric.color) < 0.25;
        let ink = |a: f32| if dark { Color32::from_white_alpha((a * 0.8 * 255.0) as u8) } else { Color32::from_black_alpha((a * 255.0) as u8) };
        let px = self.scale;
        let fold = |y: f64, a: f32| {
            let pts = [to(p, view(p, v2(0.0, y))), to(p, view(p, v2(p.w, y)))];
            painter.extend(Shape::dashed_line(&pts, Stroke::new((0.05 * px).max(0.8), ink(a)), (0.28 * px).max(2.0), (0.14 * px).max(1.5)));
        };
        match p.kind {
            PieceKind::Rib => {
                fold(s.rib_h, 0.4);
                let at = to(p, view(p, v2(if inside { p.w - 0.4 } else { 0.4 }, s.rib_h - 0.25)));
                if 0.7 * px >= 5.0 {
                    let align = if inside { egui::Align2::RIGHT_BOTTOM } else { egui::Align2::LEFT_BOTTOM };
                    painter.text(at, align, "FOLD", FontId::new(0.7 * px, crate::style::semibold()), ink(0.35));
                }
            }
            PieceKind::Collar => fold(s.collar_stand, 0.35),
            _ => {}
        }
        if p.id == PieceId::Front && !inside && s.placket_l > 0.0 {
            let fill = hex(&sb_print::color::mix(&doc.fabric.color, if dark { "#ffffff" } else { "#000000" }, 0.18));
            for b in &g.buttons {
                let c = to(p, *b);
                painter.circle(c, (0.55 * px).max(1.5), fill, Stroke::new((0.05 * px).max(0.6), ink(0.35)));
                if px > 6.0 {
                    for (dx, dy) in [(-0.16, -0.16), (0.16, -0.16), (-0.16, 0.16), (0.16, 0.16)] {
                        painter.circle_filled(to(p, v2(b.x + dx, b.y + dy)), 0.05 * px, ink(0.45));
                    }
                }
            }
        }
    }
}

/// pointer position if it is over `canvas`
fn canvas_hover(ui: &egui::Ui, canvas: Rect) -> Option<Pos2> {
    ui.input(|i| i.pointer.hover_pos()).filter(|p| canvas.contains(*p))
}
