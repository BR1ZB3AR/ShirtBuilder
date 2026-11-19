//! ShirtBuilder app crate: shared by the native binary and the web (wasm) build.

mod app;
mod changelog;
pub mod gpu;
pub(crate) mod exports;
mod file;
mod scene3d;
mod panes;
mod pattern_view;
mod sidebar;
mod style;
mod tess;
mod theme_menu;
mod themes;
mod titlebar;
mod topbar;
mod widgets;

pub use app::{restore_saved, ShirtBuilderApp};
