//! Renders the real app headlessly on the GPU (wgpu) and clicks through it like a user.
//! Needs a GPU, so it is opt-in:
//!   SB_SCREENSHOT_DIR=/some/dir cargo test -p shirtbuilder --test screenshots -- --ignored

use egui_kittest::kittest::Queryable as _;
use egui_kittest::Harness;
use shirtbuilder::ShirtBuilderApp;

fn save(h: &mut Harness<'_, ShirtBuilderApp>, name: &str) {
    let img = h.render().expect("render");
    if let Ok(dir) = std::env::var("SB_SCREENSHOT_DIR") {
        std::fs::create_dir_all(&dir).unwrap();
        img.save(format!("{dir}/{name}.png")).unwrap();
    }
    assert!(img.width() > 0 && img.height() > 0);
}

/// let the background worker (geometry, print atlases) finish and the view settle
fn settle(h: &mut Harness<'_, ShirtBuilderApp>) {
    for _ in 0..40 {
        h.step();
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

#[test]
#[ignore = "needs a GPU; run with --ignored"]
fn app_renders_and_responds() {
    let mut h = Harness::builder().with_size([1440.0, 900.0]).wgpu().build_eframe(|cc| ShirtBuilderApp::new(cc));
    settle(&mut h);
    save(&mut h, "01-default");

    h.get_by_label("Inside").click();
    settle(&mut h);
    save(&mut h, "02-inside");

    h.get_by_label("Outside").click();
    h.get_by_label("Scene").click();
    h.get_by_label("Back").click();
    settle(&mut h);
    save(&mut h, "03-scene-tab-back");

    h.get_by_label("3D").click();
    h.get_by_label("¾").click();
    settle(&mut h);
    save(&mut h, "04-3d-angle");
}

/// the generated shirt for every style, on the mannequin and on a hanger
#[test]
#[ignore = "needs a GPU; run with --ignored"]
fn generated_garments_render() {
    let mut h = Harness::builder().with_size([1200.0, 900.0]).wgpu().build_eframe(|cc| ShirtBuilderApp::new(cc));
    for (id, hanger, color) in [("shirt-long-m-M", false, "#243a5c"), ("polo-short-f-M", false, "#9c1f2a"), ("tee-long-m-L", true, "#e8e7e2"), ("vneck-short-m-M", false, "#151515")] {
        let mut doc: serde_json::Value = serde_json::from_str(&sb_core::doc::save_doc(&sb_core::doc::default_doc(sb_core::pattern::GarmentId::normalize(id)))).unwrap();
        doc["fabric"]["color"] = color.into();
        doc["scene"]["hanger"] = hanger.into();
        h.state_mut().open_json("test.json", &doc.to_string());
        h.state_mut().set_layout("3d");
        settle(&mut h);
        save(&mut h, &format!("gen-{id}{}", if hanger { "-hanger" } else { "" }));
    }
}

/// the layer panel, the theme gallery and a few themes
#[test]
#[ignore = "needs a GPU; run with --ignored"]
fn themes_and_layer_panel() {
    let mut h = Harness::builder().with_size([1440.0, 900.0]).wgpu().build_eframe(|cc| ShirtBuilderApp::new(cc));
    settle(&mut h);
    h.get_by_label("BUILD IT. NOT GENERATE IT.").click();
    settle(&mut h);
    save(&mut h, "05-layer-props");
    h.get_by_label("Theme").click();
    settle(&mut h);
    save(&mut h, "06-theme-menu");
    // filter the gallery, then pick (cards further down need the filter to be on screen)
    for (filter, name, file) in [("Liquid glass", "Liquid Glass", "07-liquid-glass"), ("Cyberpunk", "Cyberpunk Magenta", "08-cyber"), ("Standard", "Daylight", "09-daylight")] {
        if h.query_by_label(filter).is_none() {
            h.get_by_label("Theme").click(); // re-open the menu
            settle(&mut h);
        }
        h.get_by_label(filter).click();
        settle(&mut h);
        h.get_by_label(name).click();
        settle(&mut h);
        save(&mut h, file);
        if let Some(chip) = h.query_by_label(filter) {
            chip.click(); // filter off again
        }
        settle(&mut h);
    }
}

/// close look at the shading: window light, shadows, sheen on a white generated tee
#[test]
#[ignore = "needs a GPU; run with --ignored"]
fn shading_closeup() {
    let mut h = Harness::builder().with_size([1000.0, 1000.0]).wgpu().build_eframe(|cc| ShirtBuilderApp::new(cc));
    for (gobo, name) in [(true, "shade-gobo"), (false, "shade-plain")] {
        let mut doc: serde_json::Value = serde_json::from_str(&sb_core::doc::save_doc(&sb_core::doc::default_doc(sb_core::pattern::GarmentId::normalize("tee-short-m-M")))).unwrap();
        doc["fabric"]["color"] = "#e8e7e2".into();
        doc["scene"]["model"] = "generated".into();
        doc["scene"]["gobo"] = gobo.into();
        doc["layers"] = serde_json::json!([]);
        h.state_mut().open_json("test.json", &doc.to_string());
        h.state_mut().set_layout("3d");
        settle(&mut h);
        eprintln!("{name}: camera at {:.0} cm", h.state().camera_distance());
        save(&mut h, name);
    }
}

/// the 3D snapshot renders off-screen at 2000 px and comes back as a PNG over the backdrop
#[test]
#[ignore = "needs a GPU; run with --ignored"]
fn snapshot_png() {
    let dir = std::env::temp_dir().join(format!("sb-snapshot-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // SAFETY: set before the app starts; no other thread reads the environment yet
    unsafe { std::env::set_var("SB_SNAPSHOT_DIR", &dir) };
    let mut h = Harness::builder().with_size([1440.0, 900.0]).wgpu().build_eframe(|cc| ShirtBuilderApp::new(cc));
    settle(&mut h);
    h.get_by_label("3D snapshot").click();
    // the snapshot is drawn by the 3D paint callback, which only runs when a frame is rendered
    for _ in 0..10 {
        h.step();
        let _ = h.render();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let png = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).find(|p| p.extension().is_some_and(|x| x == "png")).expect("snapshot written");
    let img = image::open(&png).unwrap();
    assert_eq!(img.width(), 2000);
    if let Ok(out) = std::env::var("SB_SCREENSHOT_DIR") {
        std::fs::copy(&png, format!("{out}/snapshot.png")).unwrap();
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// the realistic male and female tees from the front and the side (their true depth, not the
/// generated shirt's flatter torso)
#[test]
#[ignore = "needs a GPU; run with --ignored"]
fn realistic_models_front_and_side() {
    let mut h = Harness::builder().with_size([1200.0, 900.0]).wgpu().build_eframe(|cc| ShirtBuilderApp::new(cc));
    for id in ["tee-short-m-M", "tee-short-f-M"] {
        let doc = sb_core::doc::save_doc(&sb_core::doc::default_doc(sb_core::pattern::GarmentId::normalize(id)));
        h.state_mut().open_json("test.json", &doc);
        h.state_mut().set_layout("3d");
        for view in ["Front", "Side"] {
            h.get_by_label(view).click();
            settle(&mut h);
            save(&mut h, &format!("real-{id}-{}", view.to_lowercase()));
        }
    }
}
