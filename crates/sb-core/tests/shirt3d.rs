//! The generated 3D shirt: on a hanger it matches the web app's `buildShirt` (fixture exported from
//! the TS code); on the ghost mannequin it has the body of the realistic T-shirt models instead
//! (same mesh and print mapping as the web app, different shape).

use sb_core::doc::default_doc;
use sb_core::pattern::{atlas_rect, garment, GarmentId, PieceId, BODY_ROW_GAP};
use sb_core::shirt3d::build_shirt;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Case {
    id: String,
    ghost: bool,
    vertices: usize,
    tris: usize,
    pos: Vec<f64>,
    nrm: Vec<f64>,
    uv: Vec<f64>,
    ao: Vec<f64>,
    idx_sum: f64,
    buttons: Vec<[f64; 3]>,
    bar: Vec<[f64; 3]>,
}

const WEB_BODY_ROW_GAP: f64 = 11.0;

fn close(what: &str, id: &str, a: &[f64], b: &[f64], tol: f64) {
    assert_eq!(a.len(), b.len(), "{id} {what}: length");
    let mut worst = (0.0, 0);
    let bad: Vec<usize> = a.iter().zip(b).enumerate().filter(|(_, (x, y))| (*x - *y).abs() > tol).map(|(i, _)| i).collect();
    if !bad.is_empty() {
        eprintln!("{id} {what}: {} of {} differ; first {:?}", bad.len(), a.len(), &bad[..bad.len().min(12)]);
    }
    for (i, (x, y)) in a.iter().zip(b).enumerate() {
        let d = (x - y).abs();
        if d > worst.0 {
            worst = (d, i);
        }
    }
    assert!(worst.0 <= tol, "{id} {what}: worst diff {} at {} (web {} vs rust {})", worst.0, worst.1, a[worst.1], b[worst.1]);
}

#[test]
fn generated_shirt_matches_the_web_app() {
    let cases: Vec<Case> = serde_json::from_str(include_str!("fixtures/web-shirt3d.json")).unwrap();
    for c in cases {
        let id = GarmentId::normalize(&c.id);
        let name = format!("{} ghost={}", c.id, c.ghost);
        let g = garment(id);
        let f = default_doc(id).fabric;
        let b = build_shirt(&g, &f, c.ghost);
        let m = &b.mesh;
        assert_eq!(m.pos.len(), c.vertices, "{name} vertices");
        assert_eq!(m.idx.len() / 3, c.tris, "{name} triangles");
        assert_eq!(m.idx.iter().map(|&i| i as f64).sum::<f64>(), c.idx_sum, "{name} index sum");
        let pick3 = |v: &[[f32; 3]]| v.iter().step_by(97).flat_map(|p| p.iter().map(|&x| x as f64)).collect::<Vec<_>>();
        if !c.ghost {
            close("pos", &name, &c.pos, &pick3(&m.pos), 2e-3);
            close("nrm", &name, &c.nrm, &pick3(&m.nrm), 2e-3);
        }
        // the web sheet has an 11 cm gap above the front / back row, ours is BODY_ROW_GAP: move the
        // web atlas coordinates of the front / back row down by the difference
        let (_, ay0, _, ah) = atlas_rect(&g);
        let shift = BODY_ROW_GAP - WEB_BODY_ROW_GAP;
        let ah_web = ah - shift;
        let sl = g.piece(PieceId::SleeveR).unwrap();
        let split = sl.y + sl.h + WEB_BODY_ROW_GAP / 2.0;
        let web_uv: Vec<f64> = c
            .uv
            .chunks(2)
            .flat_map(|p| {
                let y = ay0 + (1.0 - p[1]) * ah_web;
                let y = if y > split { y + shift } else { y };
                [p[0], 1.0 - (y - ay0) / ah]
            })
            .collect();
        close("uv", &name, &web_uv, &m.uv.iter().step_by(97).flat_map(|p| p.iter().map(|&x| x as f64)).collect::<Vec<_>>(), 1e-5);
        if !c.ghost {
            close("ao", &name, &c.ao, &m.ao.iter().step_by(97).map(|&x| x as f64).collect::<Vec<_>>(), 1e-4);
            let bt: Vec<f64> = b.buttons.iter().flat_map(|x| [x.pos.x, x.pos.y, x.pos.z]).collect();
            close("buttons", &name, &c.buttons.concat(), &bt, 1e-6);
        }
        if !c.ghost {
            let bar: Vec<f64> = b.hanger_bar.iter().flat_map(|x| [x.x, x.y, x.z]).collect();
            close("hanger", &name, &c.bar.concat(), &bar, 1e-6);
        }
    }
}

/// front depth, back depth and half-width of a torso at height `y` (centre band |x| < 3 cm; width
/// from the widest point of the band)
fn body_at(pos: &[[f32; 3]], y: f64) -> (f64, f64, f64) {
    let band: Vec<&[f32; 3]> = pos.iter().filter(|p| (p[1] as f64 - y).abs() < 1.2).collect();
    let centre: Vec<&&[f32; 3]> = band.iter().filter(|p| p[0].abs() < 3.0).collect();
    let zf = centre.iter().map(|p| p[2] as f64).fold(f64::MIN, f64::max);
    let zb = centre.iter().map(|p| p[2] as f64).fold(f64::MAX, f64::min);
    let hw = band.iter().map(|p| p[0].abs() as f64).fold(0.0, f64::max);
    (zf, -zb, hw)
}

#[test]
fn ghost_body_matches_the_realistic_models() {
    use sb_core::scan::ScanModel;
    for (id, file, bytes, heights) in [
        ("tee-short-m-M", "tee-m", &include_bytes!("../../shirtbuilder/assets/models/tee-m.bin")[..], &[9.0, 17.0, 25.0, 33.0, 41.0, 49.0][..]),
        ("tee-short-f-M", "tee-f", &include_bytes!("../../shirtbuilder/assets/models/tee-f.bin")[..], &[9.0, 17.0, 25.0, 33.0, 41.0][..]),
    ] {
        let g = garment(GarmentId::normalize(id));
        let generated = build_shirt(&g, &default_doc(g.spec.id).fabric, true).mesh;
        let scan = ScanModel::parse(file, bytes).unwrap();
        let (real, out, _) = scan.build(&g);
        let outside: Vec<[f32; 3]> = out.iter().map(|&i| real.pos[i as usize]).collect();
        for &y in heights {
            let (gf, gb, gw) = body_at(&generated.pos, y);
            let (rf, rb, rw) = body_at(&outside, y);
            let what = format!("{id} at {y} cm: generated front {gf:.1} back {gb:.1} half-width {gw:.1}, realistic {rf:.1} / {rb:.1} / {rw:.1}");
            if y <= 33.0 {
                // below the armpits: the same cross-section (depth for its width) as the realistic tee
                assert!((gf / gw - rf / rw).abs() < 0.1 && (gb / gw - rb / rw).abs() < 0.1, "shape differs — {what}");
            }
            if id == "tee-short-m-M" {
                // the men's pattern wraps about as much fabric as the model: same size, too
                assert!((gf - rf).abs() < 1.5 && (gb - rb).abs() < 1.5, "depth differs — {what}");
                assert!(y > 33.0 || (gw - rw).abs() < 1.5, "width differs — {what}");
            }
        }
    }
}
