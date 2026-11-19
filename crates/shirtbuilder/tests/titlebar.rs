//! The desktop window draws its own title bar: caption buttons flush in the top-right corner.

use egui_kittest::kittest::Queryable as _;
use egui_kittest::Harness;
use shirtbuilder::ShirtBuilderApp;

#[test]
fn caption_buttons_sit_in_the_top_right_corner() {
    let mut h = Harness::builder().with_size([1440.0, 900.0]).build_eframe(|cc| ShirtBuilderApp::new(cc));
    h.run();
    let close = h.get_by_label("Close").rect();
    let max = h.get_by_label("Maximize").rect();
    let min = h.get_by_label("Minimize").rect();
    assert_eq!((close.right(), close.top()), (1440.0, 0.0), "{close:?}");
    assert_eq!((min.right(), max.right()), (max.left(), close.left()));
    // the top bar's own buttons stay clear of the caption buttons
    let export = h.get_by_label("Export").rect();
    assert!(export.right() <= min.left() - 4.0, "{export:?} vs {min:?}");
}
