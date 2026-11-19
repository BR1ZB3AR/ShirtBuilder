//! The 3D preview's backdrop (the web app's `drawBackdrop`): window light, leaves, studio,
//! gradient or solid, 1280 × 960 px. Canvas `filter: blur()` is a Gaussian; here the shapes are
//! drawn onto their own layer and that layer is blurred with three box passes.

use crate::color::{color, mix, rgba, shade};
use resvg::tiny_skia::{GradientStop, Paint, PathBuilder, Pixmap, PixmapPaint, Point, RadialGradient, Rect, SpreadMode, Transform};
use sb_core::doc::{Backdrop, SceneSettings};

pub const W: u32 = 1280;
pub const H: u32 = 960;

fn rng(mut seed: u32) -> impl FnMut() -> f64 {
    move || {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        seed as f64 / 4294967296.0
    }
}

/// canvas `createRadialGradient(cx0, cy0, r0, cx1, cy1, r1)` filling the whole canvas
fn radial(pm: &mut Pixmap, c0: (f64, f64), r0: f64, c1: (f64, f64), r1: f64, inner: resvg::tiny_skia::Color, outer: resvg::tiny_skia::Color) {
    // tiny-skia's radial gradient starts at radius 0: hold the inner colour out to r0
    let t0 = (r0 / r1).clamp(0.0, 0.99) as f32;
    let stops = vec![GradientStop::new(0.0, inner), GradientStop::new(t0, inner), GradientStop::new(1.0, outer)];
    let _ = c0;
    if let Some(sh) = RadialGradient::new(Point::from_xy(c1.0 as f32, c1.1 as f32), Point::from_xy(c1.0 as f32, c1.1 as f32), r1 as f32, stops, SpreadMode::Pad, Transform::identity()) {
        let mut p = Paint::default();
        p.shader = sh;
        pm.fill_rect(Rect::from_xywh(0.0, 0.0, pm.width() as f32, pm.height() as f32).unwrap(), &p, Transform::identity(), None);
    }
}

/// three box blurs ≈ a Gaussian of standard deviation `sigma` px (premultiplied RGBA)
pub fn blur(pm: &mut Pixmap, sigma: f64) {
    let r = ((sigma * sigma * 4.0 + 1.0).sqrt() / 2.0).round().max(1.0) as usize;
    let (w, h) = (pm.width() as usize, pm.height() as usize);
    let mut tmp = vec![0u8; w * h * 4];
    for _ in 0..3 {
        box_pass(pm.data(), &mut tmp, w, h, r, true);
        box_pass(&tmp, pm.data_mut(), w, h, r, false);
    }
}

fn box_pass(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: usize, horizontal: bool) {
    let (n, lines) = if horizontal { (w, h) } else { (h, w) };
    let idx = |line: usize, i: usize| if horizontal { (line * w + i) * 4 } else { (i * w + line) * 4 };
    let div = (2 * r + 1) as u32;
    for line in 0..lines {
        let mut sum = [0u32; 4];
        // edge pixels repeat
        for k in 0..=2 * r {
            let i = (k as isize - r as isize).clamp(0, n as isize - 1) as usize;
            let o = idx(line, i);
            for c in 0..4 {
                sum[c] += src[o + c] as u32;
            }
        }
        for i in 0..n {
            let o = idx(line, i);
            for c in 0..4 {
                dst[o + c] = ((sum[c] + div / 2) / div) as u8;
            }
            let add = (i + r + 1).min(n - 1);
            let sub = (i as isize - r as isize).max(0) as usize;
            let (oa, os) = (idx(line, add), idx(line, sub));
            for c in 0..4 {
                sum[c] = sum[c] + src[oa + c] as u32 - src[os + c] as u32;
            }
        }
    }
}

pub fn render_backdrop(s: &SceneSettings, fabric_color: &str) -> Pixmap {
    let (w, h) = (W as f64, H as f64);
    let mut pm = Pixmap::new(W, H).expect("backdrop size");
    let base = if s.match_fabric { mix(fabric_color, "#cfd6cf", 0.3) } else { s.bg_color.clone() };
    let rad = |pm: &mut Pixmap, inner: &str, outer: &str| radial(pm, (w * 0.5, h * 0.48), 40.0, (w * 0.5, h * 0.5), w * 0.62, color(inner, 1.0), color(outer, 1.0));
    match s.bg {
        Backdrop::Window => {
            rad(&mut pm, &mix(&base, "#ffffff", 0.12), &shade(&base, 0.62));
            // soft diagonal shafts of window light
            let mut layer = Pixmap::new(W, H).unwrap();
            let ts = Transform::from_translate((w * 0.5) as f32, (h * 0.5) as f32).pre_concat(Transform::from_rotate((-0.62f64).to_degrees() as f32));
            let mut r = rng(11);
            let mut x = -w * 0.95;
            while x < w * 0.95 {
                let bw = 60.0 + r() * 120.0;
                let a = 0.1 + r() * 0.16;
                let mut p = Paint::default();
                p.set_color(rgba(255, 255, 255, a));
                p.anti_alias = true;
                if let Some(rc) = Rect::from_xywh(x as f32, -h as f32, bw as f32, (h * 2.0) as f32) {
                    layer.fill_rect(rc, &p, ts, None);
                }
                x += bw + 40.0 + r() * 110.0;
            }
            blur(&mut layer, 26.0);
            pm.draw_pixmap(0, 0, layer.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
            vignette(&mut pm, 0.28);
        }
        Backdrop::Solid => pm.fill(color(&base, 1.0)),
        Backdrop::Gradient => rad(&mut pm, &mix(&base, "#ffffff", 0.28), &shade(&base, 0.5)),
        Backdrop::Studio => rad(&mut pm, &mix(&base, "#f4f4f2", 0.82), &mix(&base, "#8d8f8d", 0.7)),
        Backdrop::Leaves => {
            rad(&mut pm, &mix(&base, "#ffffff", 0.22), &shade(&base, 0.72));
            let mut layer = Pixmap::new(W, H).unwrap();
            let leaf_col = color(&shade(&base, 0.42), 0.55);
            let mut r = rng(7);
            let branch = |layer: &mut Pixmap, r: &mut dyn FnMut() -> f64, x: f64, y: f64, ang: f64, len: f64, leaves: usize, size: f64| {
                let ts = Transform::from_translate(x as f32, y as f32).pre_concat(Transform::from_rotate(ang.to_degrees() as f32));
                let mut p = Paint::default();
                p.set_color(leaf_col);
                p.anti_alias = true;
                if let Some(rc) = Rect::from_xywh(0.0, -2.0, len as f32, 4.0) {
                    layer.fill_rect(rc, &p, ts, None);
                }
                for i in 0..leaves {
                    let t = (i as f64 + 0.5) / leaves as f64;
                    let side = if i % 2 == 1 { 1.0 } else { -1.0 };
                    let lts = ts.pre_translate((len * t) as f32, 0.0).pre_concat(Transform::from_rotate((side * (0.6 + r() * 0.5)).to_degrees() as f32));
                    let l = size * (0.7 + r() * 0.6);
                    let wd = l * 0.38;
                    let mut pb = PathBuilder::new();
                    pb.move_to(0.0, 0.0);
                    pb.quad_to((l * 0.5) as f32, -wd as f32, l as f32, 0.0);
                    pb.quad_to((l * 0.5) as f32, wd as f32, 0.0, 0.0);
                    if let Some(path) = pb.finish() {
                        layer.fill_path(&path, &p, resvg::tiny_skia::FillRule::Winding, lts, None);
                    }
                }
            };
            for _ in 0..9 {
                let (x, y, a, l) = (w * (0.55 + r() * 0.5), -40.0 + r() * 160.0, std::f64::consts::PI * (0.55 + r() * 0.4), 280.0 + r() * 200.0);
                branch(&mut layer, &mut r, x, y, a, l, 9, 70.0);
            }
            for _ in 0..6 {
                let (x, y, a, l) = (-60.0 + r() * 200.0, h * (0.1 + r() * 0.5), std::f64::consts::PI * (-0.15 + r() * 0.35), 240.0 + r() * 160.0);
                branch(&mut layer, &mut r, x, y, a, l, 8, 60.0);
            }
            for _ in 0..4 {
                let (x, y, a) = (w * (0.7 + r() * 0.3), h * (0.6 + r() * 0.4), std::f64::consts::PI * (1.1 + r() * 0.3));
                branch(&mut layer, &mut r, x, y, a, 220.0, 7, 60.0);
            }
            blur(&mut layer, 16.0);
            pm.draw_pixmap(0, 0, layer.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
            // soft glow behind the garment
            let stops = vec![GradientStop::new(0.0, rgba(255, 255, 255, 0.22)), GradientStop::new(1.0, rgba(255, 255, 255, 0.0))];
            if let Some(sh) = RadialGradient::new(Point::from_xy((w * 0.5) as f32, (h * 0.5) as f32), Point::from_xy((w * 0.5) as f32, (h * 0.5) as f32), (w * 0.38) as f32, stops, SpreadMode::Pad, Transform::identity()) {
                let mut p = Paint::default();
                p.shader = sh;
                pm.fill_rect(Rect::from_xywh(0.0, 0.0, w as f32, h as f32).unwrap(), &p, Transform::identity(), None);
            }
        }
    }
    pm
}

fn vignette(pm: &mut Pixmap, a: f64) {
    let (w, h) = (pm.width() as f32, pm.height() as f32);
    let stops = vec![GradientStop::new(0.0, rgba(0, 0, 0, 0.0)), GradientStop::new(0.333, rgba(0, 0, 0, 0.0)), GradientStop::new(1.0, rgba(0, 0, 0, a))];
    if let Some(sh) = RadialGradient::new(Point::from_xy(w * 0.5, h * 0.5), Point::from_xy(w * 0.5, h * 0.5), w * 0.75, stops, SpreadMode::Pad, Transform::identity()) {
        let mut p = Paint::default();
        p.shader = sh;
        pm.fill_rect(Rect::from_xywh(0.0, 0.0, w, h).unwrap(), &p, Transform::identity(), None);
    }
}
