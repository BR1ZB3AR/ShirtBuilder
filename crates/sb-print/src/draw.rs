//! Drawing pattern pieces: fabric (solid or a gradient running across the whole garment),
//! construction details (rib texture, stitching, placket, pocket, buttons), and the artwork
//! layers — including art that runs over a seam onto the neighbouring piece.
//! Port of the web app's `draw.ts` (canvas 2D → tiny-skia).

use crate::color::{color, luminance, mix, rgba, shade};
use crate::images::{rasterize_svg, Decoded, ImageCache};
use crate::text::{self, BASE};
use resvg::tiny_skia::{
    Color, FillRule, GradientStop, LineCap, LineJoin, LinearGradient, Mask, Paint, Path, PathBuilder, Pixmap, PixmapPaint, Point, Rect, Shader, SpreadMode, Stroke, StrokeDash,
    Transform,
};
use sb_core::doc::{Align, Doc, Fabric, FabricMode, Layer, LayerType, ShapeKind, Side};
use sb_core::geom::{mirror_x, V2};
use sb_core::pattern::{atlas_rect, CollarKind, Garment, Piece, PieceId, PieceKind};
use sb_core::seams::{seams, seams_of, Seam, SeamSide};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// the pattern editor: drawn as seen from the side (inside view mirrored), with marks
    Editor,
    /// texture for the 3D preview: outside orientation, seams bled outwards
    Atlas,
    /// texture for the realistic 3D model: like `Atlas` without the construction details
    /// (stitching, hem lines, rib) — the model has real ones, which drawn ones would only smear over
    AtlasPlain,
    /// print sheets
    Export,
}

// ---------------------------------------------------------------- path helpers
pub fn poly_path(pts: &[V2], close: bool) -> Option<Path> {
    let mut pb = PathBuilder::new();
    for (i, p) in pts.iter().enumerate() {
        if i == 0 {
            pb.move_to(p.x as f32, p.y as f32);
        } else {
            pb.line_to(p.x as f32, p.y as f32);
        }
    }
    if close {
        pb.close();
    }
    pb.finish()
}

/// canvas `roundRect` (radius clamped to the box)
pub fn round_rect(x: f64, y: f64, w: f64, h: f64, r: f64) -> Option<Path> {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0) as f32;
    let (x, y, w, h) = (x as f32, y as f32, w as f32, h as f32);
    if r <= 0.0 {
        return Some(PathBuilder::from_rect(Rect::from_xywh(x, y, w, h)?));
    }
    let k = 0.552_284_8 * r;
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
    pb.finish()
}

fn solid(c: Color) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(c);
    p.anti_alias = true;
    p
}

fn stroke(w: f64, dash: Option<(f64, f64)>) -> Stroke {
    Stroke { width: w as f32, line_cap: LineCap::Butt, line_join: LineJoin::Miter, miter_limit: 10.0, dash: dash.and_then(|(a, b)| StrokeDash::new(vec![a as f32, b as f32], 0.0)) }
}

fn flip(ts: Transform, w: f64) -> Transform {
    ts.pre_translate(w as f32, 0.0).pre_scale(-1.0, 1.0)
}

// ---------------------------------------------------------------- layers
/// draw one artwork layer (ts maps the piece's view-frame cm to pixels)
pub fn draw_layer(pm: &mut Pixmap, ts: Transform, mask: Option<&Mask>, l: &Layer, imgs: &ImageCache) {
    if !l.visible || l.w <= 0.0 || l.h <= 0.0 {
        return;
    }
    let ts = ts.pre_translate(l.x as f32, l.y as f32).pre_concat(Transform::from_rotate(l.rot as f32));
    let alpha = l.opacity.clamp(0.0, 1.0);
    match l.kind {
        LayerType::Image => {
            let Some(src) = l.src.as_deref() else { return };
            let Some(img) = imgs.get(src) else { return };
            let box_ts = ts.pre_translate((-l.w / 2.0) as f32, (-l.h / 2.0) as f32);
            let paint = PixmapPaint { opacity: alpha as f32, quality: resvg::tiny_skia::FilterQuality::Bicubic, ..Default::default() };
            match &**img {
                Decoded::Raster(p) => {
                    let t = box_ts.pre_scale((l.w / p.width() as f64) as f32, (l.h / p.height() as f64) as f32);
                    pm.draw_pixmap(0, 0, p.as_ref(), &paint, t, mask);
                }
                Decoded::Svg(tree) => {
                    // rasterise at the printed size so it stays sharp
                    let k = ((ts.sx * ts.sy - ts.kx * ts.ky).abs() as f64).sqrt();
                    let (pw, ph) = ((l.w * k).ceil().clamp(1.0, 4096.0) as u32, (l.h * k).ceil().clamp(1.0, 4096.0) as u32);
                    if let Some(p) = rasterize_svg(tree, pw, ph) {
                        let t = box_ts.pre_scale((l.w / pw as f64) as f32, (l.h / ph as f64) as f32);
                        pm.draw_pixmap(0, 0, p.as_ref(), &paint, t, mask);
                    }
                }
            }
        }
        LayerType::Text => {
            let b = text::layout(l);
            let Some(path) = b.path(l.align.unwrap_or(Align::Center)) else { return };
            let t = ts.pre_scale((l.w / b.w) as f32, (l.h / b.h) as f32);
            let paint = solid(color(l.color.as_deref().unwrap_or("#ffffff"), alpha));
            pm.fill_path(&path, &paint, FillRule::Winding, t, mask);
            if b.fake_bold() {
                // a browser emboldens fonts without a bold face by stroking the outline (Skia: size / 32)
                let s = Stroke { width: (BASE / 32.0) as f32, line_join: LineJoin::Round, ..Default::default() };
                pm.stroke_path(&path, &paint, &s, t, mask);
            }
        }
        LayerType::Shape => {
            let paint = solid(color(l.fill.as_deref().unwrap_or("#ffffff"), alpha));
            let path = if l.shape == Some(ShapeKind::Ellipse) {
                Rect::from_xywh((-l.w / 2.0) as f32, (-l.h / 2.0) as f32, l.w as f32, l.h as f32).and_then(PathBuilder::from_oval)
            } else {
                round_rect(-l.w / 2.0, -l.h / 2.0, l.w, l.h, l.radius.unwrap_or(0.0))
            };
            if let Some(path) = path {
                pm.fill_path(&path, &paint, FillRule::Winding, ts, mask);
            }
        }
    }
}

// ---------------------------------------------------------------- pieces
pub fn view_outline(p: &Piece, side: Side) -> Vec<V2> {
    if side == Side::Inside {
        mirror_x(&p.outline, p.w)
    } else {
        p.outline.clone()
    }
}

/// Where a piece sits on the worn garment, as an affine map from piece-local cm to a shared
/// "garment frame" (body-panel sized), so a gradient runs continuously across all pieces.
fn garment_frame(p: &Piece, g: &Garment) -> (f64, f64, f64, f64) {
    let s = &g.spec;
    let w = s.body_w;
    match p.kind {
        PieceKind::Body => (0.0, 0.0, 1.0, 1.0),
        PieceKind::Sleeve => {
            let ky = if s.sleeve() == sb_core::pattern::SleeveLen::Long { 0.92 } else { 0.8 };
            (if p.id == PieceId::SleeveR { -0.3 * p.w } else { w }, s.shoulder_drop, 0.3, ky)
        }
        _ => (w / 2.0 - p.w * 0.25, -1.5, 0.5, 0.25),
    }
}

fn fabric_paint(f: &Fabric, p: &Piece, g: &Garment, dim: f64, ts: Transform) -> Paint<'static> {
    if f.mode == FabricMode::Solid {
        return solid(color(&shade(&f.color, dim), 1.0));
    }
    let a = f.angle.to_radians();
    let (w, h) = (g.spec.body_w, g.spec.body_h);
    let r = (a.cos().abs() * w + a.sin().abs() * h) / 2.0;
    let (ox, oy, kx, ky) = garment_frame(p, g);
    let to_local = |fx: f64, fy: f64| Point::from_xy(((fx - ox) / kx) as f32, ((fy - oy) / ky) as f32);
    let p0 = to_local(w / 2.0 - a.cos() * r, h / 2.0 - a.sin() * r);
    let p1 = to_local(w / 2.0 + a.cos() * r, h / 2.0 + a.sin() * r);
    let stops = vec![GradientStop::new(0.0, color(&shade(&f.color, dim), 1.0)), GradientStop::new(1.0, color(&shade(&f.color2, dim), 1.0))];
    let mut paint = Paint::default();
    paint.anti_alias = true;
    paint.shader = LinearGradient::new(p0, p1, stops, SpreadMode::Pad, ts).unwrap_or_else(|| Shader::SolidColor(color(&shade(&f.color, dim), 1.0)));
    paint
}

/// Draw one piece in its local cm frame (`ts` maps piece-local cm to pixels).
/// Editor: drawn as seen from `side` (inside view mirrored). Atlas / Export: always in outside
/// orientation, inside art flipped. `cut`: bleed fabric and art out to this cut line (export).
#[allow(clippy::too_many_arguments)]
pub fn draw_piece_content(pm: &mut Pixmap, ts: Transform, p: &Piece, g: &Garment, doc: &Doc, all_seams: &[Seam], side: Side, mode: Mode, cut: Option<&[V2]>, imgs: &ImageCache) {
    let f = &doc.fabric;
    let dim = if side == Side::Inside { 0.9 } else { 1.0 };
    let outline = if mode == Mode::Editor { view_outline(p, side) } else { p.outline.clone() };
    let Some(path) = poly_path(cut.unwrap_or(&outline), true) else { return };
    // fabric fill — the gradient is laid out in piece coords, so it uses the same transform
    let fill = fabric_paint(f, p, g, dim, Transform::identity());
    pm.fill_path(&path, &fill, FillRule::Winding, ts, None);
    if matches!(mode, Mode::Atlas | Mode::AtlasPlain) {
        // bleed so neighbouring pieces overlap at seams in 3D
        let s = Stroke { width: 1.0, line_join: LineJoin::Round, ..Default::default() };
        pm.stroke_path(&path, &fill, &s, ts, None);
    }
    let mut mask = match Mask::new(pm.width(), pm.height()) {
        Some(m) => m,
        None => return,
    };
    mask.fill_path(&path, FillRule::Winding, true, ts);

    // construction details — drawn in outside coords
    let dts = if mode == Mode::Editor && side == Side::Inside { flip(ts, p.w) } else { ts };
    if mode != Mode::AtlasPlain {
        draw_details(pm, dts, &mask, p, g, f, side, mode);
    }

    let lts = if mode != Mode::Editor && side == Side::Inside { flip(ts, p.w) } else { ts };
    // art from a neighbouring piece (or the other edge of this one) that runs over a seam continues here
    let mine = seams_of(all_seams, p.id);
    for l in &doc.layers {
        if l.side != side || !l.visible {
            continue;
        }
        if l.piece == p.id {
            draw_layer(pm, lts, Some(&mask), l, imgs);
        }
        for (me, other) in &mine {
            if other.piece == l.piece {
                warp_across(pm, lts, &mask, l, other, me, imgs);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_details(pm: &mut Pixmap, ts: Transform, mask: &Mask, p: &Piece, g: &Garment, f: &Fabric, side: Side, mode: Mode) {
    let s = &g.spec;
    let dark = luminance(&f.color) < 0.25;
    let ink = |a: f64| if dark { rgba(255, 255, 255, a * 0.8) } else { rgba(0, 0, 0, a) };
    let line = |pm: &mut Pixmap, ts: Transform, pts: &[V2], a: f64, dash: bool, w: f64| {
        if let Some(path) = poly_path(pts, false) {
            pm.stroke_path(&path, &solid(ink(a)), &stroke(w, dash.then_some((0.28, 0.14))), ts, Some(mask));
        }
    };
    let ribs = |pm: &mut Pixmap, x0: f64, x1: f64, y0: f64, y1: f64, step: f64| {
        let mut pb = PathBuilder::new();
        let mut x = x0 + step / 2.0;
        while x < x1 {
            pb.move_to(x as f32, y0 as f32);
            pb.line_to(x as f32, y1 as f32);
            x += step;
        }
        if let Some(path) = pb.finish() {
            let c = if dark { rgba(255, 255, 255, 0.07) } else { rgba(0, 0, 0, 0.08) };
            pm.stroke_path(&path, &solid(c), &stroke(0.07, None), ts, Some(mask));
        }
    };
    let hl = |y: f64, x0: f64, x1: f64| vec![V2 { x: x0, y }, V2 { x: x1, y }];
    let full = |y: f64| hl(y, -2.0, p.w + 2.0);

    match p.kind {
        PieceKind::Rib => {
            ribs(pm, -1.0, p.w + 1.0, -1.0, p.h + 1.0, 0.3);
            if mode != Mode::Atlas {
                // cut double width, folded here: the lower half is the visible outside of the band
                line(pm, ts, &full(s.rib_h), 0.4, true, 0.05);
                if let Some(t) = text::label_path("FOLD", "Inter", 600.0, 0.7, 0.4, s.rib_h - 0.25, Align::Left) {
                    pm.fill_path(&t, &solid(ink(0.35)), FillRule::Winding, ts, Some(mask));
                }
            }
            return;
        }
        PieceKind::Collar => {
            let st = s.collar_stand;
            let hc = st + s.collar_fall;
            if s.collar == CollarKind::Polo {
                ribs(pm, -1.0, p.w + 1.0, -1.0, p.h + 1.0, 0.26);
                for y in [hc - 1.1, hc - 0.7] {
                    line(pm, ts, &full(y), 0.16, false, 0.12);
                }
            } else {
                let o = 0.6;
                let mut pts = vec![V2 { x: o, y: st }];
                for i in 0..=40 {
                    let x = s.collar_spread + o + (p.w - 2.0 * s.collar_spread - 2.0 * o) * (i as f64 / 40.0);
                    let t = (x - s.collar_spread) / (p.w - 2.0 * s.collar_spread);
                    pts.push(V2 { x, y: hc + s.collar_drop * (2.0 * t - 1.0).abs().powi(3) - o });
                }
                pts.push(V2 { x: p.w - o, y: st });
                line(pm, ts, &pts, 0.22, true, 0.06);
                line(pm, ts, &full(0.5), 0.18, true, 0.06);
            }
            if mode == Mode::Editor {
                line(pm, ts, &full(st), 0.35, true, 0.05);
            }
            return;
        }
        PieceKind::Sleeve => {
            if s.sleeve_band > 0.0 {
                let by = p.h - s.sleeve_band;
                ribs(pm, -1.0, p.w + 1.0, by, p.h + 1.0, 0.3);
                line(pm, ts, &full(by), 0.28, false, 0.07);
            } else if s.cuff_h > 0.0 {
                let by = p.h - s.cuff_h;
                line(pm, ts, &full(by), 0.32, false, 0.08);
                let c = if dark { rgba(255, 255, 255, 0.04) } else { rgba(255, 255, 255, 0.08) };
                if let Some(r) = Rect::from_xywh(-2.0, (by + 0.05) as f32, (p.w + 4.0) as f32, s.cuff_h as f32) {
                    pm.fill_rect(r, &solid(c), ts, Some(mask));
                }
                line(pm, ts, &full(by + 0.35), 0.2, true, 0.06);
                line(pm, ts, &full(p.h - 0.45), 0.2, true, 0.06);
            } else {
                for dy in [0.0, 0.55] {
                    line(pm, ts, &full(p.h - 1.8 + dy), 0.2, true, 0.06);
                }
            }
            return;
        }
        PieceKind::Body => {}
    }

    // body: hem stitching follows the (possibly curved) hem
    let hem_pts = |off: f64| {
        let mut v = Vec::new();
        let mut x = -2.0;
        while x <= p.w + 2.0 {
            v.push(V2 { x, y: g.hem_at(x) - off });
            x += 0.5;
        }
        v
    };
    if s.hem_rise > 0.0 {
        line(pm, ts, &hem_pts(0.9), 0.22, true, 0.06);
    } else {
        for off in [2.0, 1.45] {
            line(pm, ts, &hem_pts(off), 0.2, true, 0.06);
        }
    }
    if p.id != PieceId::Front {
        return;
    }
    let outside = side == Side::Outside;

    if s.placket_l > 0.0 {
        let cx = p.w / 2.0;
        let hw = s.placket_w / 2.0;
        let top = s.front_neck - 1.0;
        let bot = if s.placket_l >= p.h { p.h + 2.0 } else { s.front_neck + s.placket_l };
        if outside {
            // the overlapping placket edge casts a thin shadow
            let stops = vec![GradientStop::new(0.0, rgba(0, 0, 0, 0.0)), GradientStop::new(1.0, if dark { rgba(0, 0, 0, 0.35) } else { rgba(0, 0, 0, 0.16) })];
            if let Some(sh) = LinearGradient::new(Point::from_xy((cx - hw - 0.45) as f32, 0.0), Point::from_xy((cx - hw) as f32, 0.0), stops, SpreadMode::Pad, Transform::identity()) {
                let mut paint = Paint::default();
                paint.shader = sh;
                if let Some(r) = Rect::from_xywh((cx - hw - 0.45) as f32, top as f32, 0.45, (bot - top) as f32) {
                    pm.fill_rect(r, &paint, ts, Some(mask));
                }
            }
            let c = if dark { rgba(255, 255, 255, 0.025) } else { rgba(255, 255, 255, 0.07) };
            if let Some(r) = Rect::from_xywh((cx - hw) as f32, top as f32, (2.0 * hw) as f32, (bot - top) as f32) {
                pm.fill_rect(r, &solid(c), ts, Some(mask));
            }
        }
        let v = |x: f64, y0: f64, y1: f64| vec![V2 { x, y: y0 }, V2 { x, y: y1 }];
        line(pm, ts, &v(cx - hw, top, bot), 0.3, false, 0.06);
        line(pm, ts, &v(cx + hw, top, bot), 0.22, false, 0.05);
        line(pm, ts, &v(cx - hw + 0.3, top, bot - 0.3), 0.2, true, 0.06);
        line(pm, ts, &v(cx + hw - 0.3, top, bot - 0.3), 0.2, true, 0.06);
        if s.placket_l < p.h {
            line(pm, ts, &hl(bot, cx - hw, cx + hw), 0.3, false, 0.06);
            line(pm, ts, &hl(bot - 0.3, cx - hw + 0.3, cx + hw - 0.3), 0.2, true, 0.06);
        }
        if mode != Mode::Atlas && outside {
            // button placement (3D preview renders real buttons; mark them on the flat pattern)
            for b in &g.buttons {
                if let Some(c) = PathBuilder::from_circle(b.x as f32, b.y as f32, 0.55) {
                    pm.fill_path(&c, &solid(color(&mix(&f.color, if dark { "#ffffff" } else { "#000000" }, 0.18), 1.0)), FillRule::Winding, ts, Some(mask));
                    pm.stroke_path(&c, &solid(ink(0.35)), &stroke(0.05, None), ts, Some(mask));
                }
                for (dx, dy) in [(-0.16, -0.16), (0.16, -0.16), (-0.16, 0.16), (0.16, 0.16)] {
                    if let Some(r) = Rect::from_xywh((b.x + dx - 0.05) as f32, (b.y + dy - 0.05) as f32, 0.1, 0.1) {
                        pm.fill_rect(r, &solid(ink(0.45)), ts, Some(mask));
                    }
                }
            }
        } else if mode == Mode::Atlas && outside {
            // buttonholes peek out around the 3D buttons
            for b in &g.buttons {
                if let Some(r) = Rect::from_xywh((b.x - 0.08) as f32, (b.y - 0.75) as f32, 0.16, 1.5) {
                    pm.fill_rect(r, &solid(ink(0.35)), ts, Some(mask));
                }
            }
        }
    }

    if s.pocket {
        let x0 = p.w / 2.0 + 4.5;
        let w = 11.5;
        let y0 = s.front_neck + 11.5;
        let h = 13.0;
        let tip = 1.4;
        let poly = |k: f64| {
            vec![
                V2 { x: x0 + k, y: y0 },
                V2 { x: x0 + w - k, y: y0 },
                V2 { x: x0 + w - k, y: y0 + h - tip - k * 0.6 },
                V2 { x: x0 + w / 2.0, y: y0 + h - k * 1.1 },
                V2 { x: x0 + k, y: y0 + h - tip - k * 0.6 },
                V2 { x: x0 + k, y: y0 },
            ]
        };
        if outside {
            line(pm, ts.pre_translate(0.07, 0.1), &poly(0.0), if dark { 0.5 } else { 0.22 }, false, 0.14);
            if let Some(path) = poly_path(&poly(0.0), true) {
                pm.stroke_path(&path, &solid(rgba(255, 255, 255, 0.1)), &stroke(0.05, None), ts, Some(mask));
            }
            line(pm, ts, &hl(y0 + 2.2, x0, x0 + w), 0.2, true, 0.06);
        }
        line(pm, ts, &poly(0.3), if outside { 0.22 } else { 0.12 }, true, 0.06);
    }
}

// ---------------------------------------------------------------- art across seams
/// Draw the part of layer `l` (on piece `home.piece`) that hangs past the seam edge `home`,
/// unfolded onto the neighbouring piece whose matching edge is `nb` (ts in that piece's cm).
/// The layer is rendered once off-screen, then copied across in small triangles: a point at arc
/// position s and distance d beyond `home` lands at arc s, distance d inside `nb`.
#[allow(clippy::too_many_arguments)]
fn warp_across(pm: &mut Pixmap, ts: Transform, mask: &Mask, l: &Layer, home: &SeamSide, nb: &SeamSide, imgs: &ImageCache) {
    let r = 0.5 * l.w.hypot(l.h);
    let n = home.pts.len() - 1;
    let (mut i0, mut i1) = (usize::MAX, None::<usize>);
    for i in 0..=n {
        if (home.pts[i].x - l.x).hypot(home.pts[i].y - l.y) < r + 0.6 {
            i0 = i0.min(i);
            i1 = Some(i1.map_or(i, |v| v.max(i)));
        }
    }
    let Some(i1) = i1 else { return };
    let i0 = i0.saturating_sub(1);
    let i1 = (i1 + 1).min(n);
    let k = (ts.sx as f64).hypot(ts.ky as f64).clamp(3.0, 16.0);
    let size = ((2.0 * r * k).ceil()).min(2048.0).max(1.0) as u32;
    let kk = size as f64 / (2.0 * r);
    let Some(mut off) = Pixmap::new(size, size) else { return };
    let ots = Transform::from_row(kk as f32, 0.0, 0.0, kk as f32, (-(l.x - r) * kk) as f32, (-(l.y - r) * kk) as f32);
    draw_layer(&mut off, ots, None, l, imgs);
    let d_max = (2.0 * r + 0.6).min(30.0);
    let step = 1.0;
    let pp = |i: usize, d: f64| V2 { x: home.pts[i].x + home.nrm[i].x * d, y: home.pts[i].y + home.nrm[i].y * d };
    let qq = |i: usize, d: f64| V2 { x: nb.pts[i].x - nb.nrm[i].x * d, y: nb.pts[i].y - nb.nrm[i].y * d };
    // source pixels of a home-piece point
    let src_of = |p: V2| ((p.x - (l.x - r)) * kk, (p.y - (l.y - r)) * kk);
    for i in i0..i1 {
        let mut d = 0.0;
        while d < d_max {
            let mid = pp(i, d + step / 2.0);
            if (mid.x - l.x).hypot(mid.y - l.y) <= r + step * 1.2 {
                let (p00, p10, p01, p11) = (pp(i, d), pp(i + 1, d), pp(i, d + step), pp(i + 1, d + step));
                let (q00, q10, q01, q11) = (qq(i, d), qq(i + 1, d), qq(i, d + step), qq(i + 1, d + step));
                raster_tri(pm, ts, mask, &off, [src_of(p00), src_of(p10), src_of(p11)], [q00, q10, q11]);
                raster_tri(pm, ts, mask, &off, [src_of(p00), src_of(p11), src_of(p01)], [q00, q11, q01]);
            }
            d += step;
        }
    }
}

fn apply(ts: Transform, p: V2) -> (f64, f64) {
    (ts.sx as f64 * p.x + ts.kx as f64 * p.y + ts.tx as f64, ts.ky as f64 * p.x + ts.sy as f64 * p.y + ts.ty as f64)
}

/// copy the source triangle `s` (off-screen pixels) onto destination triangle `q` (piece cm,
/// mapped by `ts`), bilinear sampled, clipped by `mask`, composited source-over
fn raster_tri(pm: &mut Pixmap, ts: Transform, mask: &Mask, src: &Pixmap, s: [(f64, f64); 3], q: [V2; 3]) {
    let d = [apply(ts, q[0]), apply(ts, q[1]), apply(ts, q[2])];
    let area = (d[1].0 - d[0].0) * (d[2].1 - d[0].1) - (d[2].0 - d[0].0) * (d[1].1 - d[0].1);
    if area.abs() < 1e-9 {
        return;
    }
    let (w, h) = (pm.width() as i64, pm.height() as i64);
    let x0 = (d.iter().map(|p| p.0).fold(f64::MAX, f64::min).floor() as i64).max(0);
    let x1 = (d.iter().map(|p| p.0).fold(f64::MIN, f64::max).ceil() as i64).min(w - 1);
    let y0 = (d.iter().map(|p| p.1).fold(f64::MAX, f64::min).floor() as i64).max(0);
    let y1 = (d.iter().map(|p| p.1).fold(f64::MIN, f64::max).ceil() as i64).min(h - 1);
    if x0 > x1 || y0 > y1 {
        return;
    }
    let (sw, sh) = (src.width() as i64, src.height() as i64);
    let sdata = src.data();
    let mdata = mask.data();
    let mw = mask.width() as i64;
    let data = pm.data_mut();
    let inv = 1.0 / area;
    for y in y0..=y1 {
        let py = y as f64 + 0.5;
        for x in x0..=x1 {
            let px = x as f64 + 0.5;
            // barycentric weights
            let w0 = ((d[1].0 - px) * (d[2].1 - py) - (d[2].0 - px) * (d[1].1 - py)) * inv;
            let w1 = ((d[2].0 - px) * (d[0].1 - py) - (d[0].0 - px) * (d[2].1 - py)) * inv;
            let w2 = 1.0 - w0 - w1;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }
            let cov = mdata[(y * mw + x) as usize] as u32;
            if cov == 0 {
                continue;
            }
            let sx = w0 * s[0].0 + w1 * s[1].0 + w2 * s[2].0 - 0.5;
            let sy = w0 * s[0].1 + w1 * s[1].1 + w2 * s[2].1 - 0.5;
            let (fx, fy) = (sx.floor(), sy.floor());
            let (tx, ty) = (sx - fx, sy - fy);
            let (ix, iy) = (fx as i64, fy as i64);
            let mut acc = [0.0f64; 4];
            for (dx, dy, wt) in [(0, 0, (1.0 - tx) * (1.0 - ty)), (1, 0, tx * (1.0 - ty)), (0, 1, (1.0 - tx) * ty), (1, 1, tx * ty)] {
                let (cx, cy) = (ix + dx, iy + dy);
                if cx < 0 || cy < 0 || cx >= sw || cy >= sh || wt == 0.0 {
                    continue;
                }
                let o = ((cy * sw + cx) * 4) as usize;
                for c in 0..4 {
                    acc[c] += sdata[o + c] as f64 * wt;
                }
            }
            let k = cov as f64 / 255.0;
            let sa = acc[3] * k;
            if sa <= 0.0 {
                continue;
            }
            let o = ((y * w + x) * 4) as usize;
            let inv_a = 1.0 - sa / 255.0;
            for c in 0..4 {
                let v = acc[c] * k + data[o + c] as f64 * inv_a;
                data[o + c] = v.round().clamp(0.0, 255.0) as u8;
            }
        }
    }
}

// ---------------------------------------------------------------- atlas
/// pixels per cm of the 3D texture atlas (as the web app: ≤ 18, ≤ 4096 px)
pub fn atlas_ppc(g: &Garment) -> f64 {
    let (_, _, w, h) = atlas_rect(g);
    18f64.min(4096.0 / w.max(h))
}

/// one piece rendered into its own small pixmap, placed at (ox, oy) in the atlas
#[allow(clippy::too_many_arguments)]
fn atlas_piece(p: &Piece, g: &Garment, doc: &Doc, all_seams: &[Seam], side: Side, ppc: f64, mode: Mode, imgs: &ImageCache) -> Option<(i32, i32, Pixmap)> {
    let (ax0, ay0, _, _) = atlas_rect(g);
    let m = 1.0; // bleed stroke reaches 0.5 cm beyond the outline
    let ox = (((p.x - ax0 - m) * ppc).floor()).max(0.0) as i32;
    let oy = (((p.y - ay0 - m) * ppc).floor()).max(0.0) as i32;
    let w = ((p.w + 2.0 * m) * ppc).ceil() as u32 + 2;
    let h = ((p.h + 2.0 * m) * ppc).ceil() as u32 + 2;
    let mut pm = Pixmap::new(w, h)?;
    let ts = Transform::from_row(ppc as f32, 0.0, 0.0, ppc as f32, ((p.x - ax0) * ppc) as f32 - ox as f32, ((p.y - ay0) * ppc) as f32 - oy as f32);
    draw_piece_content(&mut pm, ts, p, g, doc, all_seams, side, mode, None, imgs);
    Some((ox, oy, pm))
}

/// Render a full texture atlas for one side of the fabric (pieces drawn in parallel on desktop).
/// `details`: draw the construction details (stitching, hems, rib) — off for the realistic model.
pub fn render_atlas(g: &Garment, doc: &Doc, side: Side, ppc: f64, imgs: &ImageCache, details: bool) -> Option<Pixmap> {
    let mode = if details { Mode::Atlas } else { Mode::AtlasPlain };
    let (_, _, aw, ah) = atlas_rect(g);
    let (w, h) = ((aw * ppc).round() as u32, (ah * ppc).round() as u32);
    let mut atlas = Pixmap::new(w.max(1), h.max(1))?;
    // keep colour in transparent texels so mip-mapped edges don't go dark
    atlas.fill(color(&shade(&doc.fabric.color, if side == Side::Inside { 0.9 } else { 1.0 }), 0.3));
    let all_seams = seams(g, side);
    #[cfg(not(target_arch = "wasm32"))]
    let parts: Vec<_> = std::thread::scope(|sc| {
        let hs: Vec<_> = g.pieces.iter().map(|p| sc.spawn(|| atlas_piece(p, g, doc, &all_seams, side, ppc, mode, imgs))).collect();
        hs.into_iter().filter_map(|h| h.join().ok().flatten()).collect()
    });
    #[cfg(target_arch = "wasm32")]
    let parts: Vec<_> = g.pieces.iter().filter_map(|p| atlas_piece(p, g, doc, &all_seams, side, ppc, mode, imgs)).collect();
    for (ox, oy, pm) in parts {
        atlas.draw_pixmap(ox, oy, pm.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
    }
    Some(atlas)
}
