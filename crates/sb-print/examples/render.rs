//! Render the default design's atlases, a backdrop and the export sheets into a folder:
//!   cargo run -p sb-print --release --example render -- /tmp/out
use sb_core::doc::{default_doc, Backdrop, Side};
use sb_core::pattern::{garment, GarmentId, Unit};
use sb_print::*;
use std::time::Instant;

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    std::fs::create_dir_all(&dir).unwrap();
    for id in ["tee-short-m-M", "shirt-long-f-L", "polo-short-m-XL"] {
        let gid = GarmentId::normalize(id);
        let mut doc = default_doc(gid);
        if id.starts_with("shirt") {
            doc.fabric.mode = sb_core::doc::FabricMode::Gradient;
            doc.fabric.color = "#243a5c".into();
            doc.fabric.color2 = "#9c1f2a".into();
        }
        let g = garment(gid);
        let mut imgs = ImageCache::default();
        imgs.prepare(image_srcs(&doc));
        for side in [Side::Outside, Side::Inside] {
            let t = Instant::now();
            let pm = render_atlas(&g, &doc, side, atlas_ppc(&g), &imgs).unwrap();
            println!("{id} {side:?}: {}×{} in {:.0} ms", pm.width(), pm.height(), t.elapsed().as_secs_f64() * 1000.0);
            pm.save_png(format!("{dir}/atlas-{id}-{side:?}.png")).unwrap();
        }
        let t = Instant::now();
        let png = sheets::pattern_png(&doc, Side::Outside, Unit::Cm, true, 100.0, &imgs).unwrap();
        std::fs::write(format!("{dir}/sheet-{id}.png"), png).unwrap();
        let svg = sheets::print_svg(&doc, Side::Outside, Unit::Cm, true, 150.0, &imgs).unwrap();
        std::fs::write(format!("{dir}/print-{id}.svg"), svg).unwrap();
        std::fs::write(format!("{dir}/cut-{id}.svg"), sheets::cutlines_svg(&doc, Unit::Cm, true)).unwrap();
        println!("{id} sheets in {:.0} ms", t.elapsed().as_secs_f64() * 1000.0);
    }
    let doc = default_doc(GarmentId::normalize("tee-short-m-M"));
    for bg in [Backdrop::Window, Backdrop::Leaves, Backdrop::Studio] {
        let mut s = doc.scene.clone();
        s.bg = bg;
        let t = Instant::now();
        let pm = backdrop::render_backdrop(&s, &doc.fabric.color);
        println!("backdrop {bg:?} in {:.0} ms", t.elapsed().as_secs_f64() * 1000.0);
        pm.save_png(format!("{dir}/backdrop-{bg:?}.png")).unwrap();
    }
}
