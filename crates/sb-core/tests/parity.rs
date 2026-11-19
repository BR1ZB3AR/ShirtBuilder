//! Parity with the web app: `fixtures/web-garments.json` was exported from the TypeScript
//! implementation (pattern.ts / cutting.ts / seams.ts). Every garment × size must match it.

use sb_core::cutting::{cut_outline, notches};
use sb_core::doc::Side;
use sb_core::pattern::{fmt_len, Garment, GarmentId, PieceKind, Unit, BODY_ROW_GAP};
use sb_core::seams::seams;
use serde_json::Value;

const TOL: f64 = 1e-6;

/// the desktop sheet leaves more room above the front / back row than the web one (11 cm)
const ROW_SHIFT: f64 = BODY_ROW_GAP - 11.0;

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/web-garments.json")).expect("fixture JSON")
}

fn num(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))
}

#[track_caller]
fn close(what: &str, a: f64, b: f64) {
    assert!((a - b).abs() <= TOL * (1.0 + b.abs()), "{what}: rust {a} vs web {b}");
}

#[track_caller]
fn close_pt(what: &str, p: sb_core::V2, v: &Value) {
    close(&format!("{what}.x"), p.x, num(&v[0]));
    close(&format!("{what}.y"), p.y, num(&v[1]));
}

fn samples(a: &[sb_core::V2]) -> Vec<sb_core::V2> {
    let n = a.len();
    [0, n / 4, n / 2, (3 * n) / 4, n - 1].iter().map(|&i| a[i]).collect()
}

#[test]
fn garment_ids_match_web_order() {
    let fx = fixture();
    let web: Vec<String> = fx["ids"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
    let rust: Vec<String> = GarmentId::all().iter().map(|g| g.to_string()).collect();
    assert_eq!(rust, web);
}

#[test]
fn every_garment_matches_the_web_app() {
    let fx = fixture();
    let gs = fx["garments"].as_object().unwrap();
    assert_eq!(gs.len(), 96);
    for id in GarmentId::all() {
        let key = id.to_string();
        let w = &gs[&key];
        let g = Garment::build(id);
        let ctx = |s: &str| format!("{key} {s}");

        assert_eq!(g.spec.title, w["title"].as_str().unwrap(), "{key} title");
        assert_eq!(g.spec.subtitle, w["subtitle"].as_str().unwrap(), "{key} subtitle");
        close(&ctx("sheet W"), g.w, num(&w["W"]));
        close(&ctx("sheet H"), g.h, num(&w["H"]) + ROW_SHIFT);
        close(&ctx("capH"), g.spec.cap_h, num(&w["capH"]));
        close(&ctx("bodyW"), g.spec.body_w, num(&w["bodyW"]));
        close(&ctx("bodyH"), g.spec.body_h, num(&w["bodyH"]));

        let wb = w["buttons"].as_array().unwrap();
        assert_eq!(g.buttons.len(), wb.len(), "{key} buttons");
        for (i, b) in g.buttons.iter().enumerate() {
            close_pt(&ctx(&format!("button {i}")), *b, &wb[i]);
        }
        for (i, p) in samples(&g.front_neck).iter().enumerate() {
            close_pt(&ctx(&format!("frontNeck {i}")), *p, &w["frontNeck"][i]);
        }
        for (i, p) in samples(&g.armhole).iter().enumerate() {
            close_pt(&ctx(&format!("armhole {i}")), *p, &w["armhole"][i]);
        }
        for (name, f) in [
            ("capY", &(|x| g.cap_y(x)) as &dyn Fn(f64) -> f64),
            ("halfW", &|y| g.sleeve_half_w(y)),
            ("inset", &|y| g.inset(y)),
            ("hemAt", &|x| g.hem_at(x)),
        ] {
            for s in w[name].as_array().unwrap() {
                let x = num(&s[0]);
                close(&ctx(&format!("{name}({x})")), f(x), num(&s[1]));
            }
        }

        let wp = w["pieces"].as_array().unwrap();
        assert_eq!(g.pieces.len(), wp.len(), "{key} piece count");
        for (p, q) in g.pieces.iter().zip(wp) {
            let pc = |s: &str| format!("{key} {} {s}", p.id.as_str());
            assert_eq!(p.id.as_str(), q["id"].as_str().unwrap(), "{key} piece order");
            assert_eq!(p.name, q["name"].as_str().unwrap(), "{}", pc("name"));
            assert_eq!(p.cut as u64, q["cut"].as_u64().unwrap(), "{}", pc("cut"));
            for (f, v) in [("w", p.w), ("h", p.h), ("x", p.x), ("y", p.y)] {
                let shift = if f == "y" && p.kind == PieceKind::Body { ROW_SHIFT } else { 0.0 };
                close(&pc(f), v, num(&q[f]) + shift);
            }
            assert_eq!(p.outline.len() as u64, q["n"].as_u64().unwrap(), "{}", pc("outline points"));
            close(&pc("perimeter"), sb_core::geom::poly_len(&p.outline), num(&q["perim"]));
            for (i, s) in samples(&p.outline).iter().enumerate() {
                close_pt(&pc(&format!("outline {i}")), *s, &q["samples"][i]);
            }

            let cut = cut_outline(p, &g);
            assert_eq!(cut.len() as u64, q["cutN"].as_u64().unwrap(), "{}", pc("cut points"));
            let bb = [
                cut.iter().map(|c| c.x).fold(f64::INFINITY, f64::min),
                cut.iter().map(|c| c.y).fold(f64::INFINITY, f64::min),
                cut.iter().map(|c| c.x).fold(f64::NEG_INFINITY, f64::max),
                cut.iter().map(|c| c.y).fold(f64::NEG_INFINITY, f64::max),
            ];
            for (i, b) in bb.iter().enumerate() {
                close(&pc(&format!("cut bbox {i}")), *b, num(&q["cutBbox"][i]));
            }
            for (i, s) in samples(&cut).iter().enumerate() {
                close_pt(&pc(&format!("cut {i}")), *s, &q["cutSamples"][i]);
            }

            let ns = notches(p, &g);
            let wn = q["notches"].as_array().unwrap();
            assert_eq!(ns.len(), wn.len(), "{}", pc("notch count"));
            for (i, (n, v)) in ns.iter().zip(wn).enumerate() {
                close(&pc(&format!("notch {i} px")), n.p.x, num(&v[0]));
                close(&pc(&format!("notch {i} py")), n.p.y, num(&v[1]));
                close(&pc(&format!("notch {i} nx")), n.n.x, num(&v[2]));
                close(&pc(&format!("notch {i} ny")), n.n.y, num(&v[3]));
                assert_eq!(n.count as u64, v[4].as_u64().unwrap(), "{}", pc("notch kind"));
            }
        }

        for (side, sk) in [(Side::Outside, "outside"), (Side::Inside, "inside")] {
            let ss = seams(&g, side);
            let ws = w["seams"][sk].as_array().unwrap();
            assert_eq!(ss.len(), ws.len(), "{key} {sk} seam count");
            for (i, (sm, v)) in ss.iter().zip(ws).enumerate() {
                for (half, sv) in [(&sm.a, &v["a"]), (&sm.b, &v["b"])] {
                    let sc = |s: &str| format!("{key} {sk} seam {i} {} {s}", half.piece.as_str());
                    assert_eq!(half.piece.as_str(), sv["piece"].as_str().unwrap(), "{}", sc("piece"));
                    assert_eq!(half.pts.len() as u64, sv["n"].as_u64().unwrap(), "{}", sc("samples"));
                    assert_eq!(half.fixed, sv["fixed"].as_bool().unwrap(), "{}", sc("fixed"));
                    close(&sc("seg"), half.seg, num(&sv["seg"]));
                    let m = half.pts.len() / 2;
                    close_pt(&sc("p0"), half.pts[0], &sv["p0"]);
                    close_pt(&sc("pm"), half.pts[m], &sv["pm"]);
                    close_pt(&sc("pn"), *half.pts.last().unwrap(), &sv["pn"]);
                    close_pt(&sc("nm"), half.nrm[m], &sv["nm"]);
                }
            }
        }
    }
}

#[test]
fn unit_formatting_matches_the_web_app() {
    let fx = fixture();
    for case in fx["fmt"].as_array().unwrap() {
        let unit = match case[0].as_str().unwrap() {
            "mm" => Unit::Mm,
            "cm" => Unit::Cm,
            _ => Unit::In,
        };
        let v = num(&case[1]);
        assert_eq!(fmt_len(v, unit, None), case[2].as_str().unwrap(), "fmt({v}, {unit:?})");
    }
}
