//! The generated 3D shirt (ported line by line from the web app's `shirt3d.ts`).
//!
//! Every pattern piece is a grid in its own piece-local centimetres, wrapped onto the body by a
//! mapping function: the torso (a ghost-mannequin upper body, or a flatter shirt on a hanger),
//! sleeve tubes lofted from the armhole, and the neck rib or collar rising from the neckline.
//! Each vertex keeps its texture-atlas coordinate, so the atlas (fabric + prints) maps 1:1.
//! No graphics API here: the output is plain arrays for any renderer.

use crate::doc::Fabric;
use crate::geom::{clamp, dist_to_poly, point_in_poly, V2};
use crate::pattern::{atlas_rect, CollarKind, Fit, Garment, Piece, PieceId, SleeveLen};
use std::collections::HashMap;
use std::f64::consts::PI;
use std::ops::{Add, Mul, Neg, Sub};
use std::sync::{Mutex, OnceLock};

// ---------------------------------------------------------------- 3D vector
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}
pub const fn v3(x: f64, y: f64, z: f64) -> V3 {
    V3 { x, y, z }
}
impl V3 {
    pub fn len(self) -> f64 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }
    pub fn len_sq(self) -> f64 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }
    /// like three.js: a zero vector stays zero
    pub fn normalize(self) -> V3 {
        let l = self.len();
        if l > 0.0 {
            self * (1.0 / l)
        } else {
            self
        }
    }
    pub fn dot(self, o: V3) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn cross(self, o: V3) -> V3 {
        v3(self.y * o.z - self.z * o.y, self.z * o.x - self.x * o.z, self.x * o.y - self.y * o.x)
    }
    pub fn lerp(self, o: V3, t: f64) -> V3 {
        self + (o - self) * t
    }
    pub fn dist(self, o: V3) -> f64 {
        (self - o).len()
    }
}
impl Add for V3 {
    type Output = V3;
    fn add(self, o: V3) -> V3 {
        v3(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl Sub for V3 {
    type Output = V3;
    fn sub(self, o: V3) -> V3 {
        v3(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl Mul<f64> for V3 {
    type Output = V3;
    fn mul(self, k: f64) -> V3 {
        v3(self.x * k, self.y * k, self.z * k)
    }
}
impl Neg for V3 {
    type Output = V3;
    fn neg(self) -> V3 {
        v3(-self.x, -self.y, -self.z)
    }
}

type MapFn<'a> = Box<dyn Fn(f64, f64) -> V3 + 'a>;

// ---------------------------------------------------------------- noise
fn hash3(x: i32, y: i32, z: i32) -> f64 {
    let mut h = x.wrapping_mul(374761393) ^ y.wrapping_mul(668265263) ^ z.wrapping_mul(1274126177);
    h = (h ^ ((h as u32) >> 13) as i32).wrapping_mul(1274126177);
    h ^= ((h as u32) >> 16) as i32;
    (h as u32) as f64 / 4294967295.0
}
pub fn vnoise(x: f64, y: f64, z: f64) -> f64 {
    let (xi, yi, zi) = (x.floor(), y.floor(), z.floor());
    let (xf, yf, zf) = (x - xi, y - yi, z - zi);
    let (xi, yi, zi) = (xi as i32, yi as i32, zi as i32);
    let u = xf * xf * (3.0 - 2.0 * xf);
    let v = yf * yf * (3.0 - 2.0 * yf);
    let w = zf * zf * (3.0 - 2.0 * zf);
    let l = |a: f64, b: f64, t: f64| a + (b - a) * t;
    let c = |dx: i32, dy: i32, dz: i32| hash3(xi.wrapping_add(dx), yi.wrapping_add(dy), zi.wrapping_add(dz));
    l(l(l(c(0, 0, 0), c(1, 0, 0), u), l(c(0, 1, 0), c(1, 1, 0), u), v), l(l(c(0, 0, 1), c(1, 0, 1), u), l(c(0, 1, 1), c(1, 1, 1), u), v), w) * 2.0 - 1.0
}
fn fbm(x: f64, y: f64, z: f64) -> f64 {
    vnoise(x, y, z) * 0.62 + vnoise(x * 2.03 + 5.1, y * 2.03, z * 2.03) * 0.26 + vnoise(x * 4.1, y * 4.1 + 3.7, z * 4.1) * 0.12
}
pub fn smooth(e0: f64, e1: f64, x: f64) -> f64 {
    let t = clamp((x - e0) / (e1 - e0), 0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn sq(x: f64) -> f64 {
    x * x
}

// ---------------------------------------------------------------- torso mapping
struct ArcTable {
    s: Vec<f64>,
    total: f64,
}
const ARC_N: usize = 256;
impl ArcTable {
    fn new(a: f64, b: f64) -> ArcTable {
        let mut s = vec![0.0; ARC_N + 1];
        let (mut px, mut pz) = (0.0, b);
        for i in 1..=ARC_N {
            let t = (i as f64 / ARC_N as f64) * (PI / 2.0);
            let x = a * t.sin();
            let z = b * t.cos();
            s[i] = s[i - 1] + (x - px).hypot(z - pz);
            px = x;
            pz = z;
        }
        let total = s[ARC_N];
        ArcTable { s, total }
    }
    fn inv(&self, f: f64) -> f64 {
        let target = f * self.total;
        let (mut lo, mut hi) = (0usize, ARC_N);
        while hi - lo > 1 {
            let m = (lo + hi) >> 1;
            if self.s[m] <= target {
                lo = m
            } else {
                hi = m
            }
        }
        let d = self.s[hi] - self.s[lo];
        let frac = (target - self.s[lo]) / if d != 0.0 { d } else { 1.0 };
        ((lo as f64 + frac) / ARC_N as f64) * (PI / 2.0)
    }
}

struct TorsoCfg {
    a: f64,
    bf: f64,
    bb: f64,
    table: ArcTable,
}

fn torso_cfg(g: &Garment, ghost: bool) -> TorsoCfg {
    let half = g.spec.body_w / 2.0;
    let bf = if ghost { 12.6 } else { 9.2 };
    let bb = if ghost { 10.6 } else { 7.0 };
    let (mut lo, mut hi) = (bf, half * 2.0);
    for _ in 0..40 {
        let m = (lo + hi) / 2.0;
        if ArcTable::new(m, bf).total > half * 0.985 {
            hi = m
        } else {
            lo = m
        }
    }
    let a = (lo + hi) / 2.0;
    TorsoCfg { a, bf, bb, table: ArcTable::new(a, bf) }
}

/// torso on a hanger: one elliptical cross-section, the top folding over the hanger bar
fn torso_map<'a>(g: &'a Garment, back: bool, cfg: &'a TorsoCfg) -> MapFn<'a> {
    let s = &g.spec;
    let w = s.body_w;
    let h = s.body_h;
    let half = w / 2.0;
    let b = if back { cfg.bb } else { cfg.bf };
    let y0 = h - s.shoulder_drop - s.arm_depth + 4.0;
    let slope = s.shoulder_drop / (s.shoulder_half - s.neck_w);
    let top_at = move |ax: f64| if ax <= s.neck_w { h } else { h - (ax - s.neck_w) * slope };
    let bust_y = y0 - 7.0;
    Box::new(move |px, py| {
        let xx = px - half;
        let yy = h - py;
        let hr = half - g.inset(clamp(py, 0.0, h));
        let kr = hr / half;
        let u = xx / hr;
        let au = u.abs();
        let mut th = if au <= 1.0 { cfg.table.inv(au) } else { PI / 2.0 + ((au - 1.0) * hr) / b };
        if u < 0.0 {
            th = -th;
        }
        let mut x = cfg.a * th.sin() * kr;
        let mut z = b * th.cos() * (0.55 + 0.45 * kr);
        if !back && s.bust > 0.0 {
            let bx = x.abs();
            z += s.bust * (-sq((bx - 8.5) / 5.5)).exp() * (-sq((yy - bust_y) / 7.0)).exp() * th.cos().max(0.0);
        }
        let mut y = yy;
        if yy < 24.0 {
            x *= 1.0 + ((24.0 - yy) / 24.0) * 0.035;
        }
        let top = top_at(xx.abs()).max(y0 + 2.0);
        if yy > y0 {
            let n = ((yy - y0) / (top - y0)).min(1.0);
            let ph = (n * PI) / 2.0;
            z *= ph.cos();
            y = y0 + (top - y0) * ph.sin();
            x *= 1.0 - 0.03 * n;
        }
        if back {
            x = -x;
            z = -z;
        }
        v3(x, y, z)
    })
}

/// (A, arc table) for an ellipse of front depth B whose quarter arc equals `len` — cached
fn ellipse_for(b: f64, len: f64) -> std::sync::Arc<(f64, ArcTable)> {
    static CACHE: OnceLock<Mutex<HashMap<i64, std::sync::Arc<(f64, ArcTable)>>>> = OnceLock::new();
    // JS: Math.round(x * 10) / 10 (half up)
    let bq = (b * 10.0 + 0.5).floor() / 10.0;
    let lq = (len * 10.0 + 0.5).floor() / 10.0;
    let key = ((bq * 10.0).round() as i64) * 1_000_000 + (lq * 10.0).round() as i64;
    let cache = CACHE.get_or_init(Default::default);
    if let Some(e) = cache.lock().unwrap().get(&key) {
        return e.clone();
    }
    let (mut lo, mut hi) = (0.2, lq * 2.0);
    for _ in 0..34 {
        let m = (lo + hi) / 2.0;
        if ArcTable::new(m, bq).total > lq {
            hi = m
        } else {
            lo = m
        }
    }
    let a = (lo + hi) / 2.0;
    let e = std::sync::Arc::new((a, ArcTable::new(a, bq)));
    cache.lock().unwrap().insert(key, e.clone());
    e
}

/// Ghost-mannequin torso: the cross-section changes with height like a real upper body —
/// deep and round through the chest, flattening to wide, shallow shoulders, then only the last
/// few centimetres roll over to the shoulder seam. Each row still wraps exactly the fabric width
/// of the pattern at that height, so prints keep their true size.
fn ghost_torso_map(g: &Garment, back: bool) -> MapFn<'_> {
    let s = &g.spec;
    let w = s.body_w;
    let h = s.body_h;
    let half = w / 2.0;
    let female = s.fit() == Fit::F;
    let bf0 = if female { 11.0 } else { 12.0 };
    let bb0 = if female { 9.0 } else { 10.0 };
    let y_arm = h - s.shoulder_drop - s.arm_depth;
    let slope = s.shoulder_drop / (s.shoulder_half - s.neck_w);
    let top_at = move |ax: f64| if ax <= s.neck_w { h } else { h - (ax - s.neck_w) * slope };
    let roll = 9.0;
    let bust_y = y_arm - 4.0;
    Box::new(move |px, py| {
        let xx = px - half;
        let yy = h - py;
        let hr = half - g.inset(clamp(py, 0.0, h));
        let fl = 1.0 - 0.36 * smooth(y_arm - 4.0, h - 3.0, yy);
        let bf = bf0 * fl;
        let b = (if back { bb0 } else { bf0 }) * fl;
        let e = ellipse_for(bf, hr * 0.985);
        let (a, table) = (e.0, &e.1);
        let u = xx / hr;
        let au = u.abs();
        let mut th = if au <= 1.0 { table.inv(au) } else { PI / 2.0 + ((au - 1.0) * hr) / b };
        if u < 0.0 {
            th = -th;
        }
        let mut x = a * th.sin();
        let mut z = b * th.cos();
        let ct = th.cos().max(0.0);
        z *= 1.0 + (if back { 0.06 } else { 0.1 }) * (-sq((yy - (y_arm - 4.0)) / 10.0)).exp() * ct;
        if !back && s.bust > 0.0 {
            let bx = x.abs();
            z += s.bust * (-sq((bx - 8.5) / 5.5)).exp() * (-sq((yy - bust_y) / 7.0)).exp() * ct;
        }
        if !female {
            x *= 1.0 - 0.02 * (-sq((yy - h * 0.42) / 12.0)).exp();
        }
        if yy < 24.0 {
            x *= 1.0 + ((24.0 - yy) / 24.0) * 0.03;
        }
        let mut y = yy;
        let top = top_at(xx.abs());
        let fs = (top - roll).max(y_arm + 2.0);
        if yy > fs {
            let n = ((yy - fs) / (top - fs).max(0.5)).min(1.0);
            let ph = (n * PI) / 2.0;
            z *= ph.cos();
            y = fs + (top - fs) * ph.sin();
        }
        if back {
            x = -x;
            z = -z;
        }
        v3(x, y, z)
    })
}

// ---------------------------------------------------------------- helpers
fn resample(pts: &[V3], n: usize) -> Vec<V3> {
    let mut cum = vec![0.0];
    for i in 1..pts.len() {
        cum.push(cum[i - 1] + pts[i].dist(pts[i - 1]));
    }
    let total = *cum.last().unwrap();
    let total = if total != 0.0 { total } else { 1.0 };
    let mut out = Vec::with_capacity(n + 1);
    let mut k = 0;
    for i in 0..=n {
        let t = (i as f64 / n as f64) * total;
        while k < cum.len() - 2 && cum[k + 1] < t {
            k += 1;
        }
        let d = cum[k + 1] - cum[k];
        let f = (t - cum[k]) / if d != 0.0 { d } else { 1.0 };
        out.push(pts[k].lerp(pts[k + 1], clamp(f, 0.0, 1.0)));
    }
    out
}
fn sample_uniform(pts: &[V3], t: f64) -> V3 {
    let f = clamp(t, 0.0, 1.0) * (pts.len() - 1) as f64;
    let i = (f.floor() as usize).min(pts.len() - 2);
    pts[i].lerp(pts[i + 1], f - i as f64)
}
fn centroid(pts: &[V3]) -> V3 {
    let mut c = V3::default();
    for p in pts {
        c = c + *p;
    }
    c * (1.0 / pts.len() as f64)
}

// ---------------------------------------------------------------- sleeve mapping
#[allow(clippy::too_many_arguments)]
fn sleeve_map<'a>(g: &'a Garment, left: bool, arm_f: Vec<V3>, arm_b: Vec<V3>, ghost: bool, deg_override: Option<f64>, len_mul: f64) -> MapFn<'a> {
    let s = &g.spec;
    let sw = s.sleeve_w;
    let sh = s.sleeve_h;
    let c = sw / 2.0;
    let all: Vec<V3> = arm_f.iter().chain(arm_b.iter()).copied().collect();
    let cc = centroid(&all);
    let side = if left { 1.0 } else { -1.0 };
    let long = s.sleeve() == SleeveLen::Long;
    let deg = deg_override.unwrap_or(if ghost {
        if long {
            76.0
        } else {
            56.0
        }
    } else {
        s.sleeve_angle
    });
    let a = deg.to_radians();
    let d = v3(side * a.cos(), -a.sin(), if ghost { 0.1 } else { 0.06 }).normalize();
    let bend = if ghost && long { 5.5 } else { 0.0 };
    let up = (v3(0.0, 1.0, 0.0) + d * -d.y).normalize();
    let mut fwd = d.cross(up).normalize() * side;
    if fwd.z < 0.0 {
        fwd = -fwd;
    }
    let r = s.sleeve_hem_w / (2.0 * PI);
    let shoulder = arm_f[0];
    let (mut lo, mut hi) = (1.0, sh * 1.6);
    for _ in 0..30 {
        let l = (lo + hi) / 2.0;
        let top = cc + d * l + up * r;
        if top.dist(shoulder) > sh * 0.97 {
            hi = l
        } else {
            lo = l
        }
    }
    let hem_c = cc + d * (((lo + hi) / 2.0) * len_mul);
    let sgn = if left { -1.0 } else { 1.0 };
    Box::new(move |sx, sy| {
        let hw = g.sleeve_half_w(sy.max(0.0));
        let pn = clamp((sx - c) / hw, -1.0, 1.0);
        let cy = g.cap_y(c + (pn * sw) / 2.0);
        let f = clamp((sy - cy) / (sh - cy), 0.0, 1.0);
        let on_front = if left { pn < 0.0 } else { pn > 0.0 };
        let a0 = sample_uniform(if on_front { &arm_f } else { &arm_b }, pn.abs());
        let ang = pn * PI;
        let hp = hem_c + up * (r * ang.cos()) + fwd * (r * 0.92 * ang.sin() * sgn);
        let mut out = a0.lerp(hp, f);
        let axis = cc.lerp(hem_c, f);
        let rad = out - axis;
        let full = if ghost {
            if long {
                1.4
            } else {
                1.7 * smooth(0.0, 0.4, f) * (1.0 - 0.25 * f)
            }
        } else {
            0.9 * (PI * f).sin()
        };
        if rad.len_sq() > 1e-6 {
            out = out + rad.normalize() * (if long { (PI * f).sin() * full } else { full });
        }
        if bend != 0.0 {
            out.z += bend * smooth(0.45, 1.0, f) * smooth(0.45, 1.0, f);
        }
        out
    })
}

// ---------------------------------------------------------------- neck rib / collar mapping
#[derive(Clone)]
struct LoopPt {
    p3: V3,
    p2: V2,
    /// into the body fabric, away from the neck hole (piece-local)
    n2: V2,
    /// which body panel's map (0 front, 1 back)
    map: usize,
}

fn loop_pts(neck2: &[V2], map: &dyn Fn(f64, f64) -> V3, which: usize, from: usize, to: usize) -> Vec<LoopPt> {
    let mut out = Vec::new();
    for i in from..=to {
        let a = neck2[i.saturating_sub(1)];
        let b = neck2[(i + 1).min(neck2.len() - 1)];
        let l = (b.x - a.x).hypot(b.y - a.y);
        let l = if l != 0.0 { l } else { 1.0 };
        out.push(LoopPt { p3: map(neck2[i].x, neck2[i].y), p2: neck2[i], n2: V2 { x: -(b.y - a.y) / l, y: (b.x - a.x) / l }, map: which });
    }
    out
}

/// Neck rib: rises from the neckline continuing the body surface (on the chest it stands up; at
/// the back neck, which sits on the shoulder roll, it curls in over the edge like a real crew neck).
fn rib_map<'a>(neck2: &'a [V2], map: &'a dyn Fn(f64, f64) -> V3, center: V3, rib_h: f64, ratio: f64) -> MapFn<'a> {
    let pts = loop_pts(neck2, map, 0, 0, neck2.len() - 1);
    let dirs: Vec<V3> = pts
        .iter()
        .map(|q| {
            let below = map(q.p2.x + q.n2.x * 1.2, q.p2.y + q.n2.y * 1.2);
            let along = (q.p3 - below).normalize();
            let mut inward = v3(center.x - q.p3.x, 0.0, center.z - q.p3.z);
            if inward.len_sq() > 1e-6 {
                inward = inward.normalize();
            }
            (along * 0.75 + v3(0.0, 0.35, 0.0) + inward * 0.15).normalize()
        })
        .collect();
    let mut cum = vec![0.0];
    for i in 1..neck2.len() {
        cum.push(cum[i - 1] + (neck2[i].x - neck2[i - 1].x).hypot(neck2[i].y - neck2[i - 1].y));
    }
    let total = *cum.last().unwrap();
    Box::new(move |rx, ry| {
        let t = clamp(rx / ratio, 0.0, total);
        let mut k = 0;
        while k < cum.len() - 2 && cum[k + 1] < t {
            k += 1;
        }
        let d = cum[k + 1] - cum[k];
        let f = (t - cum[k]) / if d != 0.0 { d } else { 1.0 };
        let p = pts[k].p3.lerp(pts[k + 1].p3, f);
        let dir = dirs[k].lerp(dirs[k + 1], f).normalize();
        let mut inward = v3(center.x - p.x, 0.0, center.z - p.z);
        if inward.len_sq() > 1e-6 {
            inward = inward.normalize();
        }
        let outer = ry >= rib_h;
        let d = if outer { 2.0 * rib_h - ry } else { ry };
        p + dir * (d * 0.85) + inward * (0.1 + 0.06 * d * d + if outer { 0.0 } else { 0.22 })
    })
}

/// One-piece collar: y = 0 is the neck seam, the stand rises to y = collarStand, folds, and the
/// fall drapes back down over the shoulders/chest lying just above the body.
fn collar_map<'a>(g: &'a Garment, segs: Vec<Vec<LoopPt>>, maps: [&'a dyn Fn(f64, f64) -> V3; 2], center: V3) -> MapFn<'a> {
    let s = &g.spec;
    let mut pts: Vec<LoopPt> = Vec::new();
    let mut cum: Vec<f64> = Vec::new();
    let mut acc = 0.0;
    for seg in &segs {
        for (i, q) in seg.iter().enumerate() {
            if i > 0 {
                acc += (q.p2.x - seg[i - 1].p2.x).hypot(q.p2.y - seg[i - 1].p2.y);
            }
            pts.push(q.clone());
            cum.push(acc);
        }
    }
    let total = acc;
    let st = s.collar_stand;
    Box::new(move |cx, cy| {
        let t = clamp(cx + s.collar_gap, 0.0, total);
        let (mut lo, mut hi) = (0usize, cum.len() - 1);
        while hi - lo > 1 {
            let m = (lo + hi) >> 1;
            if cum[m] <= t {
                lo = m
            } else {
                hi = m
            }
        }
        let span = cum[hi] - cum[lo];
        let f = if span > 1e-6 { (t - cum[lo]) / span } else { 0.0 };
        let (a, b) = (&pts[lo], &pts[hi]);
        let same = a.map == b.map;
        let near = if !same && f > 0.5 { b } else { a };
        let p = a.p3.lerp(b.p3, f);
        let mut inward = v3(center.x - p.x, 0.0, center.z - p.z);
        if inward.len_sq() > 1e-6 {
            inward = inward.normalize();
        }
        let stand_dir = (v3(0.0, 1.0, 0.0) + inward * 0.15).normalize();
        if cy <= st {
            return p + stand_dir * cy;
        }
        let e = cy - st;
        let t3 = p + stand_dir * st + inward * -0.55;
        let fall_dir = (v3(0.0, -0.72, 0.0) + inward * -0.7).normalize();
        let sx = if same { a.p2.x + (b.p2.x - a.p2.x) * f } else { near.p2.x };
        let sy = if same { a.p2.y + (b.p2.y - a.p2.y) * f } else { near.p2.y };
        let n2 = near.n2;
        let along = (e * 0.92 - 0.6).max(0.0);
        let mut bs = maps[near.map](sx + n2.x * along, sy + n2.y * along) + inward * -1.05;
        bs.y += 0.55;
        let w = smooth(0.4, 4.6, e);
        (t3 + fall_dir * e).lerp(bs, w)
    })
}

/// wrinkle displacement along the normal — shared by the cloth mesh and attached trims
fn wrinkle_d(x: f64, y: f64, z: f64, amp: f64, top_y: f64) -> f64 {
    let hang = 0.2 + 0.8 * smooth(top_y - 2.0, top_y - 30.0, y);
    let n1 = fbm(x * 0.062 + 3.3, y * 0.02, z * 0.062);
    let n2 = fbm(x * 0.15 + 7.7, y * 0.045, z * 0.15);
    let ridge = 1.0 - 2.0 * n2.abs();
    amp * hang * (1.2 * n1 + 0.22 * ridge)
}

// ---------------------------------------------------------------- geometry
type FoldFn<'a> = Box<dyn Fn(f64, f64) -> (f64, f64) + 'a>;

struct Part<'a> {
    pos: Vec<V3>,
    uv: Vec<[f32; 2]>,
    idx: Vec<u32>,
    loc: Vec<V2>,
    wrinkle: f64,
    fold: Option<FoldFn<'a>>,
}

/// drape lines, underarm pulls and hem roll on the body panels
fn body_fold(g: &Garment, ghost: bool, back: bool) -> FoldFn<'_> {
    let s = &g.spec;
    let w = s.body_w;
    let h = s.body_h;
    let ay = s.shoulder_drop + s.arm_depth;
    Box::new(move |px, py| {
        let xx = px - w / 2.0;
        let side = xx.abs() / (w / 2.0);
        let drape = (xx * 0.42 + 1.4 * (py * 0.05 + xx * 0.12).sin()).sin() * smooth(ay - 4.0, h, py) * if ghost { 0.32 } else { 0.6 };
        let du = xx.abs() * 0.8 + (py - ay) * 0.6;
        let pull = (du * 0.45 + 0.5).sin() * smooth(0.3, 0.92, side) * (-sq((py - ay - 9.0) / 14.0)).exp() * if ghost { 1.05 } else { 0.75 };
        let gather = (py * 0.55 + 0.7 * (xx * 0.2).sin()).sin() * smooth(0.55, 1.0, side) * (-sq((py - h * 0.7) / 9.0)).exp() * 0.32;
        let roll = (px * 0.3).sin() * smooth(h - 6.0, h, py) * 0.25;
        let hy = g.hem_at(px) - if s.hem_rise > 0.0 { 1.0 } else { 2.2 };
        let band = 0.3 * (-sq((py - hy) / 0.45)).exp();
        let blades = if back { (xx * 0.38 + 0.8).sin() * (-sq((py - (ay + 8.0)) / 13.0)).exp() * (1.0 - smooth(0.55, 0.95, side)) * 0.45 } else { 0.0 };
        (drape + pull + gather + roll + blades, band)
    })
}

/// elbow creases, cuff bunching, and the cuff / band pulled in to the wrist
fn sleeve_fold(g: &Garment, ghost: bool) -> FoldFn<'_> {
    let s = &g.spec;
    let sh = s.sleeve_h;
    let c = s.sleeve_w / 2.0;
    let band = if s.sleeve_band != 0.0 { s.sleeve_band } else { s.cuff_h };
    let long = s.sleeve() == SleeveLen::Long;
    Box::new(move |px, py| {
        let mut w = 0.0;
        let mut fixed = 0.0;
        if long {
            let bunch_end = sh - band - 0.5;
            let irregular = 0.55 + 0.45 * (px * 0.47 + py * 0.31).sin();
            w += (py * 1.05 + 1.7 * (px * 0.21 + py * 0.13).sin()).sin()
                * irregular
                * smooth(bunch_end - 15.0, bunch_end - 3.0, py)
                * (1.0 - smooth(bunch_end - 0.5, bunch_end + 0.8, py))
                * 0.6;
            w += ((py + 0.55 * (px - c)) * 0.8 + 0.8 * (px * 0.3).sin()).sin() * (-sq((py - sh * 0.5) / 6.0)).exp() * if ghost { 0.55 } else { 0.45 };
        } else {
            w += (py * 0.5 + px * 0.28).sin() * smooth(s.cap_h, sh, py) * 0.3;
            let under = smooth(0.55, 1.0, (px - c).abs() / c);
            w += (py * 0.7 + px * 0.15).sin() * under * smooth(s.cap_h * 0.6, s.cap_h + 4.0, py) * 0.4;
        }
        if band == 0.0 {
            fixed += 0.26 * (-sq((py - (sh - 1.9)) / 0.45)).exp();
        }
        if band > 0.0 {
            fixed += -0.55 * smooth(sh - band - 0.6, sh - band + 0.3, py) + 0.35 * (-sq((py - (sh - band - 2.2)) / 1.8)).exp();
        }
        (w, fixed)
    })
}

fn build_piece<'a>(p: &Piece, map: &dyn Fn(f64, f64) -> V3, atlas: (f64, f64, f64, f64), res: f64) -> Part<'a> {
    let bleed = 1.2;
    let (x0, y0) = (-bleed, -bleed);
    let w = p.w + 2.0 * bleed;
    let h = p.h + 2.0 * bleed;
    let nx = ((w / res).ceil() as usize).max(2);
    let ny = ((h / res).ceil() as usize).max(2);
    let count = (nx + 1) * (ny + 1);
    let mut pos = Vec::with_capacity(count);
    let mut uv = Vec::with_capacity(count);
    let mut loc = Vec::with_capacity(count);
    let mut keep = vec![false; count];
    let (ax0, ay0, aw, ah) = atlas;
    for j in 0..=ny {
        for i in 0..=nx {
            let k = j * (nx + 1) + i;
            let lx = x0 + (w * i as f64) / nx as f64;
            let ly = y0 + (h * j as f64) / ny as f64;
            pos.push(r32(map(lx, ly))); // three.js keeps positions in a Float32Array
            uv.push([((p.x + lx - ax0) / aw) as f32, (1.0 - (p.y + ly - ay0) / ah) as f32]);
            loc.push(V2 { x: lx, y: ly });
            let q = V2 { x: lx, y: ly };
            keep[k] = point_in_poly(q, &p.outline) || dist_to_poly(q, &p.outline) < res * 1.5;
        }
    }
    let mut idx = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            let a = j * (nx + 1) + i;
            let b = a + 1;
            let c = a + nx + 1;
            let d = c + 1;
            if keep[a] || keep[b] || keep[c] || keep[d] {
                idx.extend([a as u32, c as u32, b as u32, b as u32, c as u32, d as u32]);
            }
        }
    }
    Part { pos, uv, idx, loc, wrinkle: 1.0, fold: None }
}

/// a vector rounded to 32-bit floats (what a three.js Float32Array attribute stores)
fn r32(v: V3) -> V3 {
    v3(v.x as f32 as f64, v.y as f32 as f64, v.z as f32 as f64)
}

/// three.js `computeVertexNormals` for an indexed mesh: area-weighted face normals, normalised
pub fn vertex_normals(pos: &[V3], idx: &[u32]) -> Vec<V3> {
    let mut n = vec![V3::default(); pos.len()];
    for t in idx.chunks_exact(3) {
        let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
        let f = (pos[c] - pos[b]).cross(pos[a] - pos[b]);
        n[a] = n[a] + f;
        n[b] = n[b] + f;
        n[c] = n[c] + f;
    }
    n.into_iter().map(|v| r32(v.normalize())).collect()
}

/// Pieces are separate grids that overlap slightly at the seams, so their normals disagree there
/// and a shading line shows. Blend each normal with nearby vertices of *other* grids that face
/// the same way (closer than the grid spacing, so a piece's own shading is untouched).
fn smooth_seams(pos: &[V3], nrm: &mut [V3], r: f64) {
    let cell = |p: V3| ((p.x / r).floor() as i64, (p.y / r).floor() as i64, (p.z / r).floor() as i64);
    let mut cells: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
    for (i, p) in pos.iter().enumerate() {
        cells.entry(cell(*p)).or_default().push(i);
    }
    let r2 = r * r;
    let src = nrm.to_vec();
    for (i, p) in pos.iter().enumerate() {
        let (cx, cy, cz) = cell(*p);
        let mut s = src[i];
        for a in -1..=1 {
            for b in -1..=1 {
                for c in -1..=1 {
                    let Some(list) = cells.get(&(cx + a, cy + b, cz + c)) else { continue };
                    for &j in list {
                        if j == i {
                            continue;
                        }
                        let d2 = (pos[j] - *p).len_sq();
                        if d2 >= r2 {
                            continue;
                        }
                        if src[j].dot(src[i]) < 0.5 {
                            continue;
                        }
                        let w = 1.0 - d2.sqrt() / r;
                        s = s + src[j] * w;
                    }
                }
            }
        }
        let l = s.len();
        nrm[i] = r32(if l > 0.0 { s * (1.0 / l) } else { s });
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ButtonSpot {
    pub pos: V3,
    pub normal: V3,
    pub r: f64,
}

/// a renderable mesh: positions, normals, atlas UVs, ambient occlusion, triangles
#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub pos: Vec<[f32; 3]>,
    pub nrm: Vec<[f32; 3]>,
    pub uv: Vec<[f32; 2]>,
    pub ao: Vec<f32>,
    pub idx: Vec<u32>,
}

pub struct ShirtBuild {
    pub mesh: Mesh,
    pub buttons: Vec<ButtonSpot>,
    pub hanger_bar: Vec<V3>,
    pub hook_base: V3,
    pub top_y: f64,
}

fn f3(v: V3) -> [f32; 3] {
    [v.x as f32, v.y as f32, v.z as f32]
}

/// Build the generated shirt for garment `g` (ghost mannequin, or on a hanger).
pub fn build_shirt(g: &Garment, f: &Fabric, ghost: bool) -> ShirtBuild {
    let s = &g.spec;
    let w = s.body_w;
    let cfg = torso_cfg(g, ghost);
    let t_f: MapFn = if ghost { ghost_torso_map(g, false) } else { torso_map(g, false, &cfg) };
    let t_b: MapFn = if ghost { ghost_torso_map(g, true) } else { torso_map(g, true, &cfg) };
    let atlas = atlas_rect(g);
    let map3 = |m: &dyn Fn(f64, f64) -> V3, pts: &[V2]| pts.iter().map(|p| m(p.x, p.y)).collect::<Vec<V3>>();
    let mirror: Vec<V2> = g.armhole.iter().map(|p| V2 { x: w - p.x, y: p.y }).collect();
    let arm_lf = resample(&map3(&t_f, &g.armhole), 64);
    let arm_lb = resample(&map3(&t_b, &mirror), 64);
    let arm_rf = resample(&map3(&t_f, &mirror), 64);
    let arm_rb = resample(&map3(&t_b, &g.armhole), 64);
    let neck_f3 = map3(&t_f, &g.front_neck);
    let neck_b3 = map3(&t_b, &g.back_neck);
    let neck_c = centroid(&neck_f3.iter().chain(neck_b3.iter()).copied().collect::<Vec<_>>());
    let underarms = [arm_lf[64], arm_rf[64]];

    let piece = |id: PieceId| g.piece(id).expect("piece");
    let sl_r = sleeve_map(g, false, arm_rf, arm_rb, ghost, None, 1.0);
    let sl_l = sleeve_map(g, true, arm_lf, arm_lb, ghost, None, 1.0);
    let mut parts: Vec<Part> = vec![
        build_piece(piece(PieceId::Front), &t_f, atlas, 0.6),
        build_piece(piece(PieceId::Back), &t_b, atlas, 0.6),
        build_piece(piece(PieceId::SleeveR), &sl_r, atlas, 0.5),
        build_piece(piece(PieceId::SleeveL), &sl_l, atlas, 0.5),
    ];
    parts[0].fold = Some(body_fold(g, ghost, false));
    parts[1].fold = Some(body_fold(g, ghost, true));
    parts[2].fold = Some(sleeve_fold(g, ghost));
    parts[3].fold = Some(sleeve_fold(g, ghost));
    if s.collar == CollarKind::Rib {
        let rf = rib_map(&g.front_neck, &*t_f, neck_c, s.rib_h, s.rib_ratio);
        let rb = rib_map(&g.back_neck, &*t_b, neck_c, s.rib_h, s.rib_ratio);
        parts.push(build_piece(piece(PieceId::RibF), &rf, atlas, 0.3));
        parts.push(build_piece(piece(PieceId::RibB), &rb, atlas, 0.3));
    } else {
        // loop: centre front → wearer's left → round the back → wearer's right → centre front,
        // so the collar's x runs left→right as seen from outside on every side
        let n = g.front_neck.len();
        let mid = (n - 1) / 2; // JS: (length - 1) / 2 — neck polylines have an odd point count
        let segs = vec![loop_pts(&g.front_neck, &*t_f, 0, mid, n - 1), loop_pts(&g.back_neck, &*t_b, 1, 0, g.back_neck.len() - 1), loop_pts(&g.front_neck, &*t_f, 0, 0, mid)];
        let cm = collar_map(g, segs, [&*t_f, &*t_b], neck_c);
        let mut part = build_piece(piece(PieceId::Collar), &cm, atlas, 0.3);
        part.wrinkle = 0.12;
        parts.push(part);
    }

    let amp = if f.wrinkles_on { f.wrinkles * 2.6 } else { 0.0 };
    let top_y = s.body_h;
    let collar_edge = if s.collar == CollarKind::Rib { 0.0 } else { (s.collar_fall * 0.92 - 0.6).max(0.0) };

    let mut mesh = Mesh::default();
    let mut all_pos: Vec<V3> = Vec::new();
    for (pi, part) in parts.iter_mut().enumerate() {
        let nrm = vertex_normals(&part.pos, &part.idx);
        let n = part.pos.len();
        let mut sgn = 1.0;
        if part.fold.is_some() {
            let mut ctr_x = 0.0;
            for p in &part.pos {
                ctr_x += p.x / n as f64;
            }
            let mut dot = 0.0;
            let mut i = 0;
            while i < n {
                dot += (part.pos[i].x - ctr_x) * nrm[i].x + part.pos[i].z * nrm[i].z;
                i += 7;
            }
            sgn = if dot < 0.0 { -1.0 } else { 1.0 };
        }
        let vo = all_pos.len() as u32;
        for i in 0..n {
            let p = part.pos[i];
            let mut d = 0.0;
            if amp > 0.0 {
                d = wrinkle_d(p.x, p.y, p.z, amp, top_y) * part.wrinkle * if ghost { 0.55 } else { 1.0 };
            }
            if let Some(fold) = &part.fold {
                let (fw, fx) = fold(part.loc[i].x, part.loc[i].y);
                d += sgn * (fw * amp * 0.75 + fx);
            }
            if d != 0.0 {
                part.pos[i] = r32(p + nrm[i] * d);
            }
            let mut ao = 1.0;
            if f.ao && collar_edge > 0.0 && pi < 2 {
                let neck2 = if pi == 0 { &g.front_neck } else { &g.back_neck };
                let (lx, ly) = (part.loc[i].x, part.loc[i].y);
                let mut dn = f64::INFINITY;
                let mut k = 0;
                while k < neck2.len() {
                    dn = dn.min((lx - neck2[k].x).hypot(ly - neck2[k].y));
                    k += 2;
                }
                if dn < collar_edge + 4.0 {
                    ao *= 1.0 - 0.42 * (-sq((dn - collar_edge - 0.3) / 0.9)).exp() - if dn < collar_edge { 0.12 } else { 0.0 };
                }
            }
            if f.ao {
                if amp > 0.0 {
                    ao *= 1.0 - 0.5 * smooth(0.0, 1.0, -d / (amp * 1.1));
                }
                for u in &underarms {
                    // JS used the undisplaced position here
                    let dist = (p.x - u.x).hypot(p.y - u.y).hypot(p.z - u.z);
                    ao *= 0.6 + 0.4 * smooth(1.0, 12.0, dist);
                }
                ao *= 0.9 + 0.1 * smooth(0.0, 10.0, p.y);
            }
            mesh.ao.push(ao as f32);
        }
        all_pos.extend_from_slice(&part.pos);
        mesh.uv.extend_from_slice(&part.uv);
        mesh.idx.extend(part.idx.iter().map(|k| k + vo));
    }
    let mut nrm = vertex_normals(&all_pos, &mesh.idx);
    smooth_seams(&all_pos, &mut nrm, 0.45);
    mesh.pos = all_pos.iter().map(|p| f3(*p)).collect();
    mesh.nrm = nrm.iter().map(|p| f3(*p)).collect();

    // hanger follows the shoulder seam, tucked just under the fabric
    let mut bar = Vec::new();
    let reach = s.shoulder_half - 2.0;
    for i in 0..=12 {
        let x = -reach + (2.0 * reach * i as f64) / 12.0;
        let top = if x.abs() <= s.neck_w { s.body_h } else { s.body_h - ((x.abs() - s.neck_w) * s.shoulder_drop) / (s.shoulder_half - s.neck_w) };
        let v = t_f(w / 2.0 + x, s.body_h - top);
        bar.push(v3(v.x, v.y - 2.6 - (1.0 - x.abs() / s.neck_w).max(0.0) * 0.6, 0.0));
    }
    // buttons sit on the placket, following the same wrinkle displacement as the cloth
    let front_fold = body_fold(g, ghost, false);
    let buttons = g
        .buttons
        .iter()
        .map(|b| {
            let p = t_f(b.x, b.y);
            let a = t_f(b.x + 0.4, b.y);
            let c = t_f(b.x, b.y + 0.4);
            let n = (c - p).cross(a - p).normalize();
            let d = if amp > 0.0 { wrinkle_d(p.x, p.y, p.z, amp, top_y) * if ghost { 0.55 } else { 1.0 } + front_fold(b.x, b.y).0 * amp * 0.75 } else { 0.0 };
            ButtonSpot { pos: p + n * (d + 0.12), normal: n, r: if s.collar == CollarKind::Shirt { 0.55 } else { 0.65 } }
        })
        .collect();
    ShirtBuild { mesh, buttons, hanger_bar: bar, hook_base: v3(0.0, s.body_h - 3.0, 0.0), top_y }
}

// ---------------------------------------------------------------- trims: buttons and hanger

/// three.js `LatheGeometry`: profile (x = radius, y = height) spun round the y axis
pub fn lathe(profile: &[(f64, f64)], segments: usize) -> (Vec<V3>, Vec<u32>) {
    let mut pos = Vec::new();
    for i in 0..=segments {
        let phi = i as f64 / segments as f64 * 2.0 * PI;
        let (sn, cs) = phi.sin_cos();
        for &(x, y) in profile {
            pos.push(v3(x * sn, y, x * cs));
        }
    }
    let m = profile.len();
    let mut idx = Vec::new();
    for i in 0..segments {
        for j in 0..m - 1 {
            let base = (j + i * m) as u32;
            let (a, b, c, d) = (base, base + m as u32, base + m as u32 + 1, base + 1);
            idx.extend([a, b, d, c, d, b]);
        }
    }
    (pos, idx)
}

/// a button mesh (lathe) for radius `r`, standing on +y
pub fn button_mesh(r: f64) -> Mesh {
    let prof = [(0.0, 0.2), (r * 0.55, 0.17), (r * 0.8, 0.2), (r, 0.13), (r, 0.0), (r * 0.9, -0.04), (0.0, -0.04)];
    let (pos, idx) = lathe(&prof, 28);
    let nrm = vertex_normals(&pos, &idx);
    Mesh { pos: pos.iter().map(|p| f3(*p)).collect(), nrm: nrm.iter().map(|p| f3(*p)).collect(), uv: vec![[0.0, 0.0]; pos.len()], ao: vec![1.0; pos.len()], idx }
}

/// centripetal Catmull–Rom through `pts` (three.js `CatmullRomCurve3` default), sampled evenly in t
fn catmull_rom(pts: &[V3], samples: usize) -> Vec<V3> {
    let n = pts.len();
    let mut out = Vec::with_capacity(samples + 1);
    for s in 0..=samples {
        let t = s as f64 / samples as f64;
        let p = (n - 1) as f64 * t;
        let mut i = p.floor() as usize;
        let mut w = p - i as f64;
        if i >= n - 1 {
            i = n - 2;
            w = 1.0;
        }
        let p0 = if i == 0 { pts[0] * 2.0 - pts[1] } else { pts[i - 1] };
        let (p1, p2) = (pts[i], pts[i + 1]);
        let p3 = if i + 2 < n { pts[i + 2] } else { pts[n - 1] * 2.0 - pts[n - 2] };
        // centripetal parametrisation
        let dt = |a: V3, b: V3| a.dist(b).sqrt().sqrt().max(1e-4);
        let (mut d0, d1, mut d2) = (dt(p0, p1), dt(p1, p2), dt(p2, p3));
        if d1 < 1e-4 {
            d0 = 1.0;
            d2 = 1.0;
        }
        let t1 = ((p1 - p0) * (1.0 / d0) - (p2 - p0) * (1.0 / (d0 + d1)) + (p2 - p1) * (1.0 / d1)) * d1;
        let t2 = ((p2 - p1) * (1.0 / d1) - (p3 - p1) * (1.0 / (d1 + d2)) + (p3 - p2) * (1.0 / d2)) * d1;
        let (w2, w3) = (w * w, w * w * w);
        let c0 = p1;
        let c1 = t1;
        let c2 = (p2 - p1) * 3.0 - t1 * 2.0 - t2;
        let c3 = (p1 - p2) * 2.0 + t1 + t2;
        out.push(c0 + c1 * w + c2 * w2 + c3 * w3);
    }
    out
}

/// three.js `TubeGeometry` along a Catmull–Rom curve (parallel-transport frames)
pub fn tube(pts: &[V3], tubular: usize, radius: f64, radial: usize) -> Mesh {
    let path = catmull_rom(pts, tubular);
    let tangents: Vec<V3> = (0..path.len()).map(|i| (path[(i + 1).min(path.len() - 1)] - path[i.saturating_sub(1)]).normalize()).collect();
    // initial normal: the axis least aligned with the tangent
    let t0 = tangents[0];
    let axis = if t0.x.abs() <= t0.y.abs() && t0.x.abs() <= t0.z.abs() {
        v3(1.0, 0.0, 0.0)
    } else if t0.y.abs() <= t0.z.abs() {
        v3(0.0, 1.0, 0.0)
    } else {
        v3(0.0, 0.0, 1.0)
    };
    let mut normal = t0.cross(axis).normalize();
    let mut pos = Vec::new();
    let mut nrm = Vec::new();
    for (i, c) in path.iter().enumerate() {
        if i > 0 {
            // rotate the frame with the tangent
            let b = tangents[i - 1].cross(tangents[i]);
            if b.len() > 1e-6 {
                let b = b.normalize();
                let th = clamp(tangents[i - 1].dot(tangents[i]), -1.0, 1.0).acos();
                // Rodrigues
                normal = normal * th.cos() + b.cross(normal) * th.sin() + b * (b.dot(normal) * (1.0 - th.cos()));
            }
        }
        let binormal = tangents[i].cross(normal).normalize();
        for j in 0..=radial {
            let v = j as f64 / radial as f64 * 2.0 * PI;
            let n = (normal * -v.cos() + binormal * v.sin()).normalize();
            nrm.push(f3(n));
            pos.push(f3(*c + n * radius));
        }
    }
    let mut idx = Vec::new();
    for j in 1..=tubular {
        for i in 1..=radial {
            let a = ((radial + 1) * (j - 1) + (i - 1)) as u32;
            let b = ((radial + 1) * j + (i - 1)) as u32;
            let c = ((radial + 1) * j + i) as u32;
            let d = ((radial + 1) * (j - 1) + i) as u32;
            idx.extend([a, b, d, b, c, d]);
        }
    }
    let n = pos.len();
    Mesh { pos, nrm, uv: vec![[0.0, 0.0]; n], ao: vec![1.0; n], idx }
}

/// a UV sphere (three.js `SphereGeometry`)
pub fn sphere(r: f64, w_seg: usize, h_seg: usize) -> Mesh {
    let mut m = Mesh::default();
    for iy in 0..=h_seg {
        let v = iy as f64 / h_seg as f64;
        for ix in 0..=w_seg {
            let u = ix as f64 / w_seg as f64;
            let n = v3(-(u * 2.0 * PI).cos() * (v * PI).sin(), (v * PI).cos(), (u * 2.0 * PI).sin() * (v * PI).sin());
            m.pos.push(f3(n * r));
            m.nrm.push(f3(n));
        }
    }
    for iy in 0..h_seg {
        for ix in 0..w_seg {
            let a = (iy * (w_seg + 1) + ix + 1) as u32;
            let b = (iy * (w_seg + 1) + ix) as u32;
            let c = ((iy + 1) * (w_seg + 1) + ix) as u32;
            let d = ((iy + 1) * (w_seg + 1) + ix + 1) as u32;
            if iy != 0 {
                m.idx.extend([a, b, d]);
            }
            if iy != h_seg - 1 {
                m.idx.extend([b, c, d]);
            }
        }
    }
    let n = m.pos.len();
    m.uv = vec![[0.0, 0.0]; n];
    m.ao = vec![1.0; n];
    m
}

impl Mesh {
    /// append `o`, moved by `at` and turned so its +y axis points along `up`
    pub fn append_transformed(&mut self, o: &Mesh, at: V3, up: V3) {
        // rotation taking +y onto `up` (three.js Quaternion.setFromUnitVectors)
        let from = v3(0.0, 1.0, 0.0);
        let to = up.normalize();
        let d = from.dot(to);
        let rot = |v: V3| -> V3 {
            if d > 1.0 - 1e-9 {
                return v;
            }
            if d < -1.0 + 1e-9 {
                return v3(v.x, -v.y, -v.z);
            }
            let axis = from.cross(to).normalize();
            let th = d.acos();
            v * th.cos() + axis.cross(v) * th.sin() + axis * (axis.dot(v) * (1.0 - th.cos()))
        };
        let base = self.pos.len() as u32;
        for (p, n) in o.pos.iter().zip(&o.nrm) {
            let p = rot(v3(p[0] as f64, p[1] as f64, p[2] as f64)) + at;
            let n = rot(v3(n[0] as f64, n[1] as f64, n[2] as f64));
            self.pos.push(f3(p));
            self.nrm.push(f3(n));
        }
        self.uv.extend_from_slice(&o.uv);
        self.ao.extend_from_slice(&o.ao);
        self.idx.extend(o.idx.iter().map(|i| i + base));
    }
    pub fn append(&mut self, o: &Mesh) {
        self.append_transformed(o, V3::default(), v3(0.0, 1.0, 0.0));
    }
}

/// the hanger: bar along the shoulders, neck and hook, as one mesh
pub fn hanger_mesh(b: &ShirtBuild) -> Mesh {
    let mut m = tube(&b.hanger_bar, 80, 0.9, 12);
    let hb = b.hook_base;
    let mut hook = vec![hb, v3(0.0, hb.y + 4.0, 0.0), v3(0.0, hb.y + 8.0, 0.0)];
    let r = 3.4;
    let cy = hb.y + 9.0;
    for i in 0..=16 {
        let t = PI - (i as f64 / 16.0) * (PI * 1.25);
        hook.push(v3(r + r * t.cos(), cy + r * t.sin(), 0.0));
    }
    m.append(&tube(&hook, 80, 0.38, 10));
    m.append_transformed(&sphere(0.38, 12, 8), *hook.last().unwrap(), v3(0.0, 1.0, 0.0));
    m
}

/// all the buttons of a shirt as one mesh
pub fn buttons_mesh(b: &ShirtBuild) -> Mesh {
    let mut m = Mesh::default();
    if let Some(first) = b.buttons.first() {
        let one = button_mesh(first.r);
        for bt in &b.buttons {
            m.append_transformed(&one, bt.pos, bt.normal);
        }
    }
    m
}
