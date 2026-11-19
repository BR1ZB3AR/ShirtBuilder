//! ShirtBuilder — one Rust codebase, two targets:
//! native desktop (`cargo run`) and web (`trunk serve`, WebAssembly).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // no console window on Windows release builds

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    // a couple of command-line flags, handy for packaging checks; anything else is a design file
    // to open (the file manager passes it when a .sb is double-clicked)
    let mut open: Option<std::path::PathBuf> = None;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--version" | "-V" => {
                println!("ShirtBuilder {} (Rust edition)", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--help" | "-h" => {
                println!("ShirtBuilder {} — design T-shirts on the sewing pattern\n\nUsage: shirtbuilder [--version] [--help] [design.sb]\n\nOpens the design file if one is given. Designs are saved as .sb files (Ctrl+S); web app projects (.json) open too.", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            a if !a.starts_with('-') && open.is_none() => open = Some(a.into()),
            _ => {}
        }
    }
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("ShirtBuilder")
            .with_app_id("shirtbuilder") // matches shirtbuilder.desktop (dock icon on Wayland)
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([960.0, 600.0])
            .with_decorations(false) // the top bar is the title bar (titlebar.rs), like SolidCraft
            .with_drag_and_drop(true)
            // window / taskbar icon (Windows, X11; Wayland takes it from the .desktop entry)
            .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon-256.png")).unwrap_or_default()),
        // auto-detect the graphics card (or use the one chosen in Scene › Graphics card)
        wgpu_options: shirtbuilder::gpu::wgpu_options(shirtbuilder::gpu::load_choice()),
        ..Default::default()
    };
    eframe::run_native(
        "ShirtBuilder",
        options,
        Box::new(move |cc| {
            let mut app = shirtbuilder::ShirtBuilderApp::new(cc);
            if let Some(path) = open {
                // asks first if the design from last time has unsaved changes
                app.request_open_path(path, 0.0);
            }
            Ok(Box::new(app))
        }),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    wasm_bindgen_futures::spawn_local(async {
        let document = eframe::web_sys::window().expect("no window").document().expect("no document");
        let canvas = document
            .get_element_by_id("shirtbuilder_canvas")
            .expect("missing #shirtbuilder_canvas")
            .dyn_into::<eframe::web_sys::HtmlCanvasElement>()
            .expect("#shirtbuilder_canvas is not a canvas");
        let started = eframe::WebRunner::new()
            .start(canvas, eframe::WebOptions::default(), Box::new(|cc| Ok(Box::new(shirtbuilder::ShirtBuilderApp::new(cc)))))
            .await;
        if let Some(loading) = document.get_element_by_id("loading") {
            match started {
                Ok(()) => loading.remove(),
                Err(e) => loading.set_inner_html(&format!("<p>ShirtBuilder could not start: {e:?}</p>")),
            }
        }
    });
}
