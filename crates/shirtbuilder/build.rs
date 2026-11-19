//! Windows builds: embed the app icon and version information in ShirtBuilder.exe (shown by
//! Explorer, the taskbar and "Apps & features"). Nothing to do for other targets.

fn main() {
    println!("cargo:rerun-if-changed=assets/shirtbuilder.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/shirtbuilder.ico")
        .set("ProductName", "ShirtBuilder")
        .set("FileDescription", "ShirtBuilder — design T-shirts on the sewing pattern")
        .set("CompanyName", "BR1ZB3AR")
        .set("LegalCopyright", "MIT License — github.com/BR1ZB3AR/ShirtBuilder")
        .set("OriginalFilename", "ShirtBuilder.exe");
    res.compile().expect("couldn't embed the Windows icon and version information");
}
