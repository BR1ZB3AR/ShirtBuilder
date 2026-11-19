//! ShirtBuilder core — garment patterns, cutting lines, seams, the design document and undo/redo.
//!
//! No UI and no platform code: the same crate runs in the native app and in the web (wasm) build.
//! Geometry is a line-by-line port of the web app, checked against it by `tests/parity.rs`.

pub mod cutting;
pub mod doc;
pub mod geom;
pub mod history;
pub mod pattern;
pub mod scan;
pub mod seams;
pub mod shirt3d;

pub use geom::{v2, V2};
pub use pattern::{garment, Garment, GarmentId, Piece, PieceId, PieceKind, Unit};
