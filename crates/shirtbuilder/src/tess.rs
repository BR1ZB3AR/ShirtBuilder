//! Triangulating pattern pieces for filled drawing. egui fills convex paths only, and pieces are
//! concave (armholes, necklines), so they are cut into triangles by ear clipping.

use sb_core::V2;

fn cross(o: V2, a: V2, b: V2) -> f64 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

fn inside_tri(p: V2, a: V2, b: V2, c: V2) -> bool {
    let d1 = cross(a, b, p);
    let d2 = cross(b, c, p);
    let d3 = cross(c, a, p);
    let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(neg && pos)
}

/// Triangles (indices into `poly`) covering a simple polygon. Duplicate consecutive points are
/// skipped. Works for either winding.
pub fn triangulate(poly: &[V2]) -> Vec<[u32; 3]> {
    let mut idx: Vec<usize> = Vec::with_capacity(poly.len());
    for (i, p) in poly.iter().enumerate() {
        if idx.last().is_none_or(|&j| poly[j].dist(*p) > 1e-9) {
            idx.push(i);
        }
    }
    while idx.len() > 2 && poly[idx[0]].dist(poly[*idx.last().unwrap()]) <= 1e-9 {
        idx.pop();
    }
    let area: f64 = (0..idx.len())
        .map(|i| {
            let a = poly[idx[i]];
            let b = poly[idx[(i + 1) % idx.len()]];
            a.x * b.y - b.x * a.y
        })
        .sum();
    if area < 0.0 {
        idx.reverse(); // make it counter-clockwise (in y-down space: area > 0)
    }
    let mut out = Vec::with_capacity(idx.len().saturating_sub(2));
    let mut guard = 0;
    while idx.len() > 3 && guard < 10_000 {
        guard += 1;
        let n = idx.len();
        let mut clipped = false;
        for i in 0..n {
            let (ia, ib, ic) = (idx[(i + n - 1) % n], idx[i], idx[(i + 1) % n]);
            let (a, b, c) = (poly[ia], poly[ib], poly[ic]);
            if cross(a, b, c) <= 1e-12 {
                continue; // reflex or degenerate corner
            }
            let blocked = idx.iter().any(|&j| j != ia && j != ib && j != ic && inside_tri(poly[j], a, b, c));
            if blocked {
                continue;
            }
            out.push([ia as u32, ib as u32, ic as u32]);
            idx.remove(i);
            clipped = true;
            break;
        }
        if !clipped {
            // numerically stuck (collinear runs): drop the flattest corner and carry on
            let n = idx.len();
            let flat = (0..n)
                .min_by(|&x, &y| {
                    let f = |i: usize| cross(poly[idx[(i + n - 1) % n]], poly[idx[i]], poly[idx[(i + 1) % n]]).abs();
                    f(x).total_cmp(&f(y))
                })
                .unwrap();
            idx.remove(flat);
        }
    }
    if idx.len() == 3 {
        out.push([idx[0] as u32, idx[1] as u32, idx[2] as u32]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use sb_core::pattern::{garment, GarmentId};

    fn tri_area(poly: &[V2], t: &[u32; 3]) -> f64 {
        cross(poly[t[0] as usize], poly[t[1] as usize], poly[t[2] as usize]).abs() / 2.0
    }

    #[test]
    fn every_piece_is_covered_exactly() {
        for id in GarmentId::all() {
            let g = garment(id);
            for p in &g.pieces {
                let tris = triangulate(&p.outline);
                let covered: f64 = tris.iter().map(|t| tri_area(&p.outline, t)).sum();
                let n = p.outline.len();
                let real = (0..n)
                    .map(|i| {
                        let a = p.outline[i];
                        let b = p.outline[(i + 1) % n];
                        a.x * b.y - b.x * a.y
                    })
                    .sum::<f64>()
                    .abs()
                    / 2.0;
                assert!((covered - real).abs() < real * 1e-6, "{id} {:?}: covered {covered} vs area {real}", p.id);
            }
        }
    }
}
