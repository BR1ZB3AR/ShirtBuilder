//! Small generated textures for the 3D preview (ports of the web app's `makeFabricNormal` and
//! `makeGobo`) and mip-map chains for everything the GPU samples minified.

use sb_core::shirt3d::vnoise;

/// RGBA8 image with its mip chain (level 0 first)
pub struct MipImage {
    pub w: u32,
    pub h: u32,
    pub levels: Vec<Vec<u8>>,
}

/// box-filter mip chain of straight-alpha RGBA8
pub fn mips(w: u32, h: u32, data: Vec<u8>) -> MipImage {
    let mut levels = vec![data];
    let (mut cw, mut ch) = (w, h);
    while cw > 1 || ch > 1 {
        let (nw, nh) = ((cw / 2).max(1), (ch / 2).max(1));
        let src = levels.last().unwrap();
        let mut dst = vec![0u8; (nw * nh * 4) as usize];
        for y in 0..nh {
            for x in 0..nw {
                let mut acc = [0u32; 4];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let sx = (x * 2 + dx).min(cw - 1);
                    let sy = (y * 2 + dy).min(ch - 1);
                    let o = ((sy * cw + sx) * 4) as usize;
                    for c in 0..4 {
                        acc[c] += src[o + c] as u32;
                    }
                }
                let o = ((y * nw + x) * 4) as usize;
                for c in 0..4 {
                    dst[o + c] = ((acc[c] + 2) / 4) as u8;
                }
            }
        }
        levels.push(dst);
        cw = nw;
        ch = nh;
    }
    MipImage { w, h, levels }
}

fn hash3(x: i32, y: i32, z: i32) -> f64 {
    let mut h = x.wrapping_mul(374761393) ^ y.wrapping_mul(668265263) ^ z.wrapping_mul(1274126177);
    h = (h ^ ((h as u32) >> 13) as i32).wrapping_mul(1274126177);
    h ^= ((h as u32) >> 16) as i32;
    (h as u32) as f64 / 4294967295.0
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Weave {
    Knit,
    Weave,
    Fleece,
}

/// tiling 128 × 128 normal map of a knit, weave or fleece surface
pub fn fabric_normal(kind: Weave) -> MipImage {
    const N: i32 = 128;
    let mut h = vec![0.0f64; (N * N) as usize];
    for y in 0..N {
        for x in 0..N {
            let mut val: f64 = 0.0;
            match kind {
                Weave::Knit => {
                    let u = (x % 32) as f64 / 32.0;
                    let w = (y % 32) as f64 / 32.0;
                    for (cx, ang) in [(0.28f64, 0.55f64), (0.72, -0.55)] {
                        let dx = u - cx;
                        let dy = w - 0.5;
                        let a = dx * ang.cos() - dy * ang.sin();
                        let b = dx * ang.sin() + dy * ang.cos();
                        val = val.max(1.0 - (a / 0.17).powi(2) - (b / 0.5).powi(2));
                    }
                    val = val.max(0.0).sqrt();
                }
                Weave::Weave => {
                    let (cx, cy) = (x / 8, y / 8);
                    let over = (cx + cy) % 2 == 0;
                    let t = if over { (x % 8) as f64 / 8.0 } else { (y % 8) as f64 / 8.0 };
                    val = (std::f64::consts::PI * t).sin() * (0.8 + 0.2 * hash3(cx, cy, 1));
                }
                Weave::Fleece => {
                    val = 0.5 + 0.5 * (vnoise(x as f64 / 6.0, y as f64 / 6.0, 0.0) * 0.6 + vnoise(x as f64 / 2.5, y as f64 / 2.5, 3.0) * 0.4);
                }
            }
            h[(y * N + x) as usize] = val;
        }
    }
    let at = |x: i32, y: i32| h[(((y + N) % N) * N + ((x + N) % N)) as usize];
    let k = 2.2;
    let mut data = vec![0u8; (N * N * 4) as usize];
    for y in 0..N {
        for x in 0..N {
            let dx = (at(x + 1, y) - at(x - 1, y)) * k;
            let dy = (at(x, y + 1) - at(x, y - 1)) * k;
            let l = (dx * dx + dy * dy + 1.0).sqrt();
            let o = ((y * N + x) * 4) as usize;
            data[o] = ((-dx / l * 0.5 + 0.5) * 255.0) as u8;
            data[o + 1] = ((dy / l * 0.5 + 0.5) * 255.0) as u8;
            data[o + 2] = ((1.0 / l * 0.5 + 0.5) * 255.0) as u8;
            data[o + 3] = 255;
        }
    }
    mips(N as u32, N as u32, data)
}

/// window-light cookie for the spot light: soft diagonal shafts with frame shadows between them
pub fn gobo() -> MipImage {
    use sb_print::tiny_skia::{Color, Paint, Pixmap, Rect, Transform};
    const N: u32 = 512;
    let mut pm = Pixmap::new(N, N).unwrap();
    pm.fill(Color::BLACK);
    let mut layer = Pixmap::new(N, N).unwrap();
    let ts = Transform::from_translate(N as f32 / 2.0, N as f32 / 2.0).pre_concat(Transform::from_rotate((-0.62f32).to_degrees()));
    let mut seed: u32 = 5;
    let mut r = || {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        seed as f64 / 4294967296.0
    };
    let n = N as f64;
    let mut x = -n;
    while x < n {
        let w = 34.0 + r() * 40.0;
        let a = 0.75 + r() * 0.25;
        let v = (255.0 * a) as u8;
        let mut p = Paint::default();
        p.set_color_rgba8(v, v, v, 255);
        p.anti_alias = true;
        if let Some(rc) = Rect::from_xywh(x as f32, -n as f32, w as f32, (2.0 * n) as f32) {
            layer.fill_rect(rc, &p, ts, None);
        }
        x += w + 16.0 + r() * 26.0;
    }
    sb_print::backdrop::blur(&mut layer, 7.0);
    pm.draw_pixmap(0, 0, layer.as_ref(), &Default::default(), Transform::identity(), None);
    mips(N, N, pm.take())
}
