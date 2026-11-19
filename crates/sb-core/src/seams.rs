//! Seams: which piece edges are sewn to which, so artwork can run across them.
//! Port of the web app's `seams.ts`.
//!
//! Each seam is a pair of matching curves (piece-local cm, one per piece) running in the same
//! direction on the finished garment. Art hanging past one piece's seam edge is "unfolded" across
//! the seam onto the other piece: a point at arc position s and distance d *outside* piece A maps
//! to arc position s and distance d *inside* piece B.

use crate::doc::Side;
use crate::geom::{mirror_x, or_nonzero, point_in_poly, poly_len, v2, V2};
use crate::pattern::{Garment, PieceId};

#[derive(Clone, Debug)]
pub struct SeamSide {
    pub piece: PieceId,
    /// resampled, uniform arc spacing
    pub pts: Vec<V2>,
    /// unit transfer directions pointing OUT of this piece
    pub nrm: Vec<V2>,
    /// nrm is one constant direction (rows / columns), not the curve normal
    pub fixed: bool,
    pub tan: Vec<V2>,
    /// arc length between samples
    pub seg: f64,
}

#[derive(Clone, Debug)]
pub struct Seam {
    pub a: SeamSide,
    pub b: SeamSide,
}

/// How art crosses an edge: perpendicular to the seam curve, or straight across horizontally /
/// vertically, matching how the 3D shirt joins those pieces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Transfer {
    Perp,
    H,
    V,
}

fn resample(pts: &[V2], n: usize) -> Vec<V2> {
    let mut cum = vec![0.0];
    for i in 1..pts.len() {
        cum.push(cum[i - 1] + pts[i].dist(pts[i - 1]));
    }
    let total = or_nonzero(*cum.last().unwrap(), 1.0);
    let mut out = Vec::with_capacity(n + 1);
    let mut k = 0;
    for i in 0..=n {
        let t = (i as f64 / n as f64) * total;
        while k < cum.len() - 2 && cum[k + 1] < t {
            k += 1;
        }
        let f = (t - cum[k]) / or_nonzero(cum[k + 1] - cum[k], 1.0);
        out.push(v2(pts[k].x + (pts[k + 1].x - pts[k].x) * f, pts[k].y + (pts[k + 1].y - pts[k].y) * f));
    }
    out
}

fn make_side(piece: PieceId, curve: &[V2], outline: &[V2], n: usize, how: Transfer) -> SeamSide {
    let pts = resample(curve, n);
    let tan: Vec<V2> = (0..pts.len())
        .map(|i| {
            let a = pts[i.saturating_sub(1)];
            let b = pts[(i + 1).min(n)];
            let l = or_nonzero(a.dist(b), 1.0);
            (b - a) * (1.0 / l)
        })
        .collect();
    // one consistent orientation for the whole curve, decided where it is least ambiguous (middle)
    let m = n / 2;
    let axis = match how {
        Transfer::H => v2(1.0, 0.0),
        Transfer::V => v2(0.0, 1.0),
        Transfer::Perp => v2(tan[m].y, -tan[m].x),
    };
    let inside = point_in_poly(pts[m] + axis * 0.3, outline);
    let sg = if inside { -1.0 } else { 1.0 };
    let nrm = if how == Transfer::Perp {
        tan.iter().map(|t| v2(sg * t.y, -sg * t.x)).collect()
    } else {
        tan.iter().map(|_| axis * sg).collect()
    };
    let seg = poly_len(&pts) / n as f64;
    SeamSide { piece, pts, nrm, fixed: how != Transfer::Perp, tan, seg }
}

/// all seams of a garment, in the view frame of `side` (the inside view is mirrored per piece)
pub fn seams(g: &Garment, side: Side) -> Vec<Seam> {
    let s = &g.spec;
    let w = s.body_w;
    let h = s.body_h;
    let cx = w / 2.0;
    let ay = s.shoulder_drop + s.arm_depth;
    let side_end = h - s.hem_rise;

    let mut side_r = vec![];
    let mut side_l = vec![];
    for i in 0..=40 {
        let y = ay + ((side_end - ay) * i as f64) / 40.0;
        side_r.push(v2(w - g.inset(y), y));
        side_l.push(v2(g.inset(y), y));
    }
    let sh_r = vec![v2(cx + s.neck_w, 0.0), v2(cx + s.shoulder_half, s.shoulder_drop)];
    let sh_l = mirror_x(&sh_r, w);
    let arm_r = g.armhole.clone(); // shoulder → underarm, right-hand side of a body panel
    let arm_l = mirror_x(&arm_r, w);
    let sw = s.sleeve_w;
    let c = sw / 2.0;
    let cap = |x0: f64, x1: f64| -> Vec<V2> {
        (0..=60)
            .map(|i| {
                let x = x0 + ((x1 - x0) * i as f64) / 60.0;
                v2(x, g.cap_y(x))
            })
            .collect()
    };
    let taper = (sw - s.sleeve_hem_w) / 2.0;

    use PieceId::*;
    use Transfer::*;
    // (pieceA, curveA, transferA, pieceB, curveB, transferB): curves run the same way on the sewn garment
    let mut defs: Vec<(PieceId, Vec<V2>, Transfer, PieceId, Vec<V2>, Transfer)> = vec![
        // side seams: front's right edge meets back's left edge and vice versa
        (Front, side_r.clone(), H, Back, side_l.clone(), H),
        (Front, side_l, H, Back, side_r, H),
        // shoulder seams (neck point → shoulder point)
        (Front, sh_r.clone(), V, Back, sh_l.clone(), V),
        (Front, sh_l, V, Back, sh_r, V),
        // armholes ↔ sleeve caps (shoulder → underarm)
        (Front, arm_r.clone(), Perp, SleeveL, cap(c, 0.0), V),
        (Back, arm_l.clone(), Perp, SleeveL, cap(c, sw), V),
        (Front, arm_l, Perp, SleeveR, cap(c, sw), V),
        (Back, arm_r, Perp, SleeveR, cap(c, 0.0), V),
    ];
    // sleeve underarm seams: one long edge meets the other
    for id in [SleeveR, SleeveL] {
        defs.push((
            id,
            vec![v2(0.0, g.cap_y(0.0)), v2(taper, s.sleeve_h)],
            H,
            id,
            vec![v2(sw, g.cap_y(sw)), v2(sw - taper, s.sleeve_h)],
            H,
        ));
    }
    if let (Some(rf), Some(rb)) = (g.piece(RibF), g.piece(RibB)) {
        defs.push((Front, g.front_neck.clone(), Perp, RibF, vec![v2(0.0, rf.h), v2(rf.w, rf.h)], V));
        defs.push((Back, g.back_neck.clone(), Perp, RibB, vec![v2(0.0, rb.h), v2(rb.w, rb.h)], V));
        // rib ends join at the shoulders
        defs.push((RibF, vec![v2(0.0, 0.0), v2(0.0, rf.h)], H, RibB, vec![v2(rb.w, 0.0), v2(rb.w, rb.h)], H));
        defs.push((RibF, vec![v2(rf.w, 0.0), v2(rf.w, rf.h)], H, RibB, vec![v2(0.0, 0.0), v2(0.0, rb.h)], H));
    }
    if g.has_piece(Collar) {
        // the collar's neck edge runs from centre front, round the wearer's left, across the back
        let gap = s.collar_gap;
        let fnk = &g.front_neck;
        let mid = fnk.len() / 2;
        let fh = poly_len(&fnk[mid..]);
        let bl = poly_len(&g.back_neck);
        let edge = |a: f64, b: f64| vec![v2(a, 0.0), v2(b, 0.0)];
        defs.push((Front, fnk[mid..].to_vec(), Perp, Collar, edge(-gap, fh - gap), V));
        defs.push((Back, g.back_neck.clone(), Perp, Collar, edge(fh - gap, fh - gap + bl), V));
        defs.push((Front, fnk[..=mid].to_vec(), Perp, Collar, edge(fh - gap + bl, 2.0 * fh - gap + bl), V));
    }

    let view = |id: PieceId, pts: &[V2]| -> Vec<V2> {
        if side == Side::Inside {
            mirror_x(pts, g.piece(id).unwrap().w)
        } else {
            pts.to_vec()
        }
    };
    defs.into_iter()
        .map(|(pa, ca, ha, pb, cb, hb)| {
            let len = poly_len(&ca).max(poly_len(&cb));
            let n = 4.max((len / 0.75).ceil() as usize);
            Seam {
                a: make_side(pa, &view(pa, &ca), &view(pa, &g.piece(pa).unwrap().outline), n, ha),
                b: make_side(pb, &view(pb, &cb), &view(pb, &g.piece(pb).unwrap().outline), n, hb),
            }
        })
        .collect()
}

/// seams with `piece` on one side, oriented as (that piece's side, the other side)
pub fn seams_of(all: &[Seam], piece: PieceId) -> Vec<(&SeamSide, &SeamSide)> {
    let mut out = vec![];
    for sm in all {
        if sm.a.piece == piece {
            out.push((&sm.a, &sm.b));
        }
        if sm.b.piece == piece {
            out.push((&sm.b, &sm.a));
        }
    }
    out
}

/// Map a point lying OUTSIDE `from` (beyond its seam edge) to where it lands on `to`.
/// `None` if the point is on the piece side of the seam or past either end of it.
pub fn map_across(from: &SeamSide, to: &SeamSide, p: V2) -> Option<V2> {
    let n = from.pts.len() - 1;
    let land = |i: usize, t: f64, d: f64| -> V2 {
        let tb = to.pts[i].lerp(to.pts[i + 1], t);
        let tn = to.nrm[i].lerp(to.nrm[i + 1], t);
        tb - tn * d
    };
    if from.fixed {
        // walk back along the constant transfer direction until we hit the seam curve
        let dir = from.nrm[0];
        for i in 0..n {
            let a = from.pts[i];
            let b = from.pts[i + 1];
            let e = b - a;
            let den = dir.x * e.y - dir.y * e.x;
            if den.abs() < 1e-9 {
                continue;
            }
            let t = (dir.x * (p.y - a.y) - dir.y * (p.x - a.x)) / den;
            if !(0.0..=1.0).contains(&t) {
                continue;
            }
            let d = (p.x - a.x - e.x * t) * dir.x + (p.y - a.y - e.y * t) * dir.y;
            return if d > 0.0 { Some(land(i, t, d)) } else { None };
        }
        return None;
    }
    let (best, _) = from
        .pts
        .iter()
        .enumerate()
        .map(|(i, q)| (i, (p.x - q.x).powi(2) + (p.y - q.y).powi(2)))
        .fold((0, f64::INFINITY), |acc, x| if x.1 < acc.1 { x } else { acc });
    let q = from.pts[best];
    let along = (p - q).dot(from.tan[best]);
    let f = best as f64 + along / from.seg;
    if f < -0.5 || f > n as f64 + 0.5 {
        return None;
    }
    let i = (f.floor().max(0.0) as usize).min(n - 1);
    let t = (f - i as f64).clamp(0.0, 1.0);
    let base = from.pts[i].lerp(from.pts[i + 1], t);
    let nr = from.nrm[i].lerp(from.nrm[i + 1], t);
    let d = (p - base).dot(nr);
    if d <= 0.0 {
        return None;
    }
    Some(land(i, t, d))
}
