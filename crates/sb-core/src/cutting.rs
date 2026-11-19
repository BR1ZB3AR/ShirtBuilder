//! Cutting layout: seam / hem allowances and notches, derived from the net (stitch-line) pieces.
//! Port of the web app's `cutting.ts`.

use crate::geom::{point_in_poly, poly_len, v2, V2};
use crate::pattern::{CollarKind, Garment, Piece, PieceId, PieceKind, Style};

/// standard seam allowance (cm)
pub const SEAM: f64 = 1.0;
/// notches sit this far (cm) up the armhole from the underarm
const ARM_NOTCH: f64 = 8.0;

/// allowance to add outside the edge a→b of a piece
fn edge_allowance(p: &Piece, g: &Garment, a: V2, b: V2) -> f64 {
    let s = &g.spec;
    let eps = 0.02;
    match p.kind {
        PieceKind::Body => {
            let on_hem = |q: V2| (q.y - g.hem_at(q.x)).abs() < eps && q.y > p.h * 0.5;
            if on_hem(a) && on_hem(b) {
                // narrow shirt hem / twin-needle tee hem
                return if s.style() == Style::Shirt { 1.2 } else { 2.5 };
            }
            SEAM
        }
        PieceKind::Sleeve => {
            if a.y >= p.h - eps && b.y >= p.h - eps {
                // a cuff is sewn on, otherwise a turned hem
                return if s.cuff_h != 0.0 { SEAM } else { 2.0 };
            }
            SEAM
        }
        PieceKind::Collar => {
            // a knitted polo collar has finished (fully-fashioned) edges: only the neck edge is sewn
            if s.collar == CollarKind::Polo {
                return if a.y < eps && b.y < eps { SEAM } else { 0.0 };
            }
            SEAM
        }
        PieceKind::Rib => SEAM, // raw edges and the shoulder joins
    }
}

fn dedupe(pts: &[V2]) -> Vec<V2> {
    let mut out: Vec<V2> = vec![];
    for &q in pts {
        if out.last().is_none_or(|l| q.dist(*l) > 1e-4) {
            out.push(q);
        }
    }
    while out.len() > 2 && out[0].dist(out[out.len() - 1]) < 1e-4 {
        out.pop();
    }
    out
}

fn signed_area(pts: &[V2]) -> f64 {
    let n = pts.len();
    (0..n).map(|i| {
        let p = pts[i];
        let q = pts[(i + 1) % n];
        p.x * q.y - q.x * p.y
    }).sum::<f64>()
        / 2.0
}

/// cut line: the net outline offset outward by each edge's allowance (mitred corners)
pub fn cut_outline(p: &Piece, g: &Garment) -> Vec<V2> {
    let pts = dedupe(&p.outline);
    let n = pts.len();
    let sgn = if signed_area(&pts) > 0.0 { 1.0 } else { -1.0 };
    struct Edge {
        t: V2,
        nrm: V2,
        d: f64,
    }
    let edges: Vec<Edge> = (0..n)
        .map(|i| {
            let a = pts[i];
            let b = pts[(i + 1) % n];
            let l = crate::geom::or_nonzero(a.dist(b), 1.0);
            let t = (b - a) * (1.0 / l);
            Edge { t, nrm: v2(sgn * t.y, -sgn * t.x), d: edge_allowance(p, g, a, b) }
        })
        .collect();
    let mut out = vec![];
    for i in 0..n {
        let e1 = &edges[(i + n - 1) % n];
        let e2 = &edges[i];
        let v = pts[i];
        if e1.d == 0.0 && e2.d == 0.0 {
            out.push(v);
            continue;
        }
        // intersect the two offset lines
        let p1 = v + e1.nrm * e1.d;
        let p2 = v + e2.nrm * e2.d;
        let cr = e1.t.x * e2.t.y - e1.t.y * e2.t.x;
        let q = if cr.abs() < 1e-3 {
            v2((p1.x + p2.x) / 2.0, (p1.y + p2.y) / 2.0)
        } else {
            let u = ((p2.x - p1.x) * e2.t.y - (p2.y - p1.y) * e2.t.x) / cr;
            v2(p1.x + e1.t.x * u, p1.y + e1.t.y * u)
        };
        let lim = 3.0 * e1.d.max(e2.d);
        if q.dist(v) > lim {
            // very sharp corner: bevel instead of a long spike
            out.push(p1);
            out.push(p2);
            continue;
        }
        out.push(q);
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Notch {
    /// on the stitch line
    pub p: V2,
    /// outward
    pub n: V2,
    /// along the edge
    pub t: V2,
    pub count: u8,
}

/// point + tangent at arc length `dist` along a polyline
fn at_arc(poly: &[V2], dist: f64) -> (V2, V2) {
    let mut acc = 0.0;
    for i in 1..poly.len() {
        let a = poly[i - 1];
        let b = poly[i];
        let l = a.dist(b);
        if acc + l >= dist || i == poly.len() - 1 {
            let f = if l != 0.0 { ((dist - acc) / l).clamp(0.0, 1.0) } else { 0.0 };
            let ll = crate::geom::or_nonzero(l, 1.0);
            let mut t = (b - a) * (1.0 / ll);
            // exactly on a corner (V-neck tip, top of a symmetric sleeve cap): use the average direction
            // of the two segments, so the notch points along the centre line and doesn't hinge on rounding
            let other = if (acc + l - dist).abs() < 1e-7 && i < poly.len() - 1 {
                Some(poly[i + 1] - b)
            } else if (dist - acc).abs() < 1e-7 && i > 1 {
                Some(a - poly[i - 2])
            } else {
                None
            };
            if let Some(o) = other {
                let lo = crate::geom::or_nonzero(o.len(), 1.0);
                let sum = t + o * (1.0 / lo);
                let sl = sum.len();
                if sl > 1e-9 {
                    t = sum * (1.0 / sl);
                }
            }
            return (a.lerp(b, f), t);
        }
        acc += l;
    }
    (*poly.last().unwrap(), v2(1.0, 0.0))
}

fn mk(p: &Piece, at: (V2, V2), count: u8) -> Notch {
    let (pt, t) = at;
    let mut n = v2(t.y, -t.x);
    if point_in_poly(pt + n * 0.3, &p.outline) {
        n = -n;
    }
    Notch { p: pt, n, t, count }
}

/// Matching notches: armholes (front single, back double), sleeve caps (same distances, plus the
/// top-of-cap notch that meets the shoulder seam), centre front/back on necklines, ribs and collar.
pub fn notches(p: &Piece, g: &Garment) -> Vec<Notch> {
    let s = &g.spec;
    let mut out = vec![];
    match p.kind {
        PieceKind::Body => {
            let front = p.id == PieceId::Front;
            let right: Vec<V2> = g.armhole.iter().rev().copied().collect(); // underarm → shoulder
            let left: Vec<V2> = right.iter().map(|q| v2(p.w - q.x, q.y)).collect();
            for arm in [&right, &left] {
                out.push(mk(p, at_arc(arm, ARM_NOTCH), if front { 1 } else { 2 }));
            }
            let neck = if front { &g.front_neck } else { &g.back_neck };
            out.push(mk(p, at_arc(neck, poly_len(neck) / 2.0), 1));
        }
        PieceKind::Sleeve => {
            let cap: Vec<V2> = (0..=120)
                .map(|i| {
                    let x = (p.w * i as f64) / 120.0;
                    v2(x, g.cap_y(x))
                })
                .collect();
            let l = poly_len(&cap);
            let ease = s.cap_ease / 2.0;
            // right sleeve: front half is x > centre; left sleeve: front half is x < centre
            let front_right = p.id == PieceId::SleeveR;
            out.push(mk(p, at_arc(&cap, ARM_NOTCH + ease), if front_right { 2 } else { 1 }));
            out.push(mk(p, at_arc(&cap, l - ARM_NOTCH - ease), if front_right { 1 } else { 2 }));
            out.push(mk(p, at_arc(&cap, l / 2.0), 1));
        }
        PieceKind::Rib => {
            for y in [0.0, p.h] {
                out.push(mk(p, (v2(p.w / 2.0, y), v2(1.0, 0.0)), 1));
            }
        }
        PieceKind::Collar => {
            let fh = poly_len(&g.front_neck) / 2.0;
            let bl = poly_len(&g.back_neck);
            for x in [fh - s.collar_gap, fh - s.collar_gap + bl / 2.0, fh - s.collar_gap + bl] {
                out.push(mk(p, (v2(x, 0.0), v2(1.0, 0.0)), 1));
            }
        }
    }
    out
}

/// notches drawn as short cuts into the allowance from the cut edge (piece cm)
pub fn notch_segments(nt: &Notch, allowance: f64) -> Vec<(V2, V2)> {
    let len = 0.6;
    let base = allowance.max(0.6);
    let offs: &[f64] = if nt.count == 2 { &[-0.3, 0.3] } else { &[0.0] };
    offs.iter()
        .map(|o| {
            let c = nt.p + nt.t * *o;
            (c + nt.n * (base - len), c + nt.n * (base + 0.05))
        })
        .collect()
}
