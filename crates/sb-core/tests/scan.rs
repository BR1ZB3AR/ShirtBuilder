//! The realistic tee models (baked by the web app) load and map onto every size's atlas.

use sb_core::pattern::{garment, Fit, GarmentId, Size, SleeveLen, Style};
use sb_core::scan::ScanModel;

#[test]
fn realistic_models_parse_and_map_into_the_atlas() {
    for (name, fit) in [("tee-m", Fit::M), ("tee-f", Fit::F)] {
        let bytes = std::fs::read(format!("{}/../shirtbuilder/assets/models/{name}.bin", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let m = ScanModel::parse(name, &bytes).unwrap();
        assert!(m.meta.vertices > 10_000, "{name}: {} vertices", m.meta.vertices);
        assert_eq!(m.out.len() % 3, 0);
        for size in [Size::XS, Size::M, Size::XXL] {
            let g = garment(GarmentId { style: Style::Tee, sleeve: SleeveLen::Short, fit, size });
            let (mesh, out, inn) = m.build(&g);
            assert_eq!(mesh.pos.len(), m.meta.vertices);
            assert!(!out.is_empty() && !inn.is_empty());
            let inside = mesh.uv.iter().filter(|uv| (0.0..=1.0).contains(&uv[0]) && (0.0..=1.0).contains(&uv[1])).count();
            assert!(inside as f64 > 0.99 * mesh.uv.len() as f64, "{name} {size:?}: {inside} of {} UVs in the atlas", mesh.uv.len());
            let top = mesh.pos.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
            assert!((top as f64 - g.spec.body_h).abs() < 12.0, "{name} {size:?}: top {top} vs body {}", g.spec.body_h);
        }
    }
}
