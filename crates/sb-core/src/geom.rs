//! 2D geometry helpers. All pattern units are centimetres; piece-local coordinates have their
//! origin at the piece's top-left, x → right, y → down, as seen from the outside of the fabric.

use serde::{Deserialize, Serialize};
use std::ops::{Add, Mul, Neg, Sub};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct V2 {
    pub x: f64,
    pub y: f64,
}

pub const fn v2(x: f64, y: f64) -> V2 {
    V2 { x, y }
}

impl V2 {
    pub fn len(self) -> f64 {
        self.x.hypot(self.y)
    }
    pub fn dot(self, o: V2) -> f64 {
        self.x * o.x + self.y * o.y
    }
    pub fn dist(self, o: V2) -> f64 {
        (self.x - o.x).hypot(self.y - o.y)
    }
    pub fn lerp(self, o: V2, t: f64) -> V2 {
        v2(self.x + (o.x - self.x) * t, self.y + (o.y - self.y) * t)
    }
}

impl Add for V2 {
    type Output = V2;
    fn add(self, o: V2) -> V2 {
        v2(self.x + o.x, self.y + o.y)
    }
}
impl Sub for V2 {
    type Output = V2;
    fn sub(self, o: V2) -> V2 {
        v2(self.x - o.x, self.y - o.y)
    }
}
impl Mul<f64> for V2 {
    type Output = V2;
    fn mul(self, k: f64) -> V2 {
        v2(self.x * k, self.y * k)
    }
}
impl Neg for V2 {
    type Output = V2;
    fn neg(self) -> V2 {
        v2(-self.x, -self.y)
    }
}

/// `Math.max(a, Math.min(b, v))`
pub fn clamp(v: f64, a: f64, b: f64) -> f64 {
    a.max(b.min(v))
}

pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// JavaScript's `x || fallback` for numbers: zero (or NaN) falls back.
pub(crate) fn or_nonzero(x: f64, fallback: f64) -> f64 {
    if x == 0.0 || x.is_nan() {
        fallback
    } else {
        x
    }
}

pub fn poly_len(pts: &[V2]) -> f64 {
    pts.windows(2).map(|w| w[0].dist(w[1])).sum()
}

pub fn point_in_poly(p: V2, poly: &[V2]) -> bool {
    let mut inside = false;
    let n = poly.len();
    if n == 0 {
        return false;
    }
    let mut j = n - 1;
    for i in 0..n {
        let a = poly[i];
        let b = poly[j];
        if (a.y > p.y) != (b.y > p.y) && p.x < ((b.x - a.x) * (p.y - a.y)) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

pub fn dist_to_poly(p: V2, poly: &[V2]) -> f64 {
    let mut best = f64::INFINITY;
    let n = poly.len();
    if n == 0 {
        return best;
    }
    let mut j = n - 1;
    for i in 0..n {
        let a = poly[j];
        let b = poly[i];
        let d = b - a;
        let l2 = or_nonzero(d.dot(d), 1e-9);
        let t = clamp((p - a).dot(d) / l2, 0.0, 1.0);
        best = best.min(p.dist(a + d * t));
        j = i;
    }
    best
}

pub fn mirror_x(pts: &[V2], w: f64) -> Vec<V2> {
    pts.iter().map(|p| v2(w - p.x, p.y)).collect()
}

/// cubic Bézier sampled at n + 1 points
pub fn cubic(p0: V2, p1: V2, p2: V2, p3: V2, n: usize) -> Vec<V2> {
    (0..=n)
        .map(|i| {
            let t = i as f64 / n as f64;
            let u = 1.0 - t;
            let a = u * u * u;
            let b = 3.0 * u * u * t;
            let c = 3.0 * u * t * t;
            let d = t * t * t;
            v2(
                a * p0.x + b * p1.x + c * p2.x + d * p3.x,
                a * p0.y + b * p1.y + c * p2.y + d * p3.y,
            )
        })
        .collect()
}
