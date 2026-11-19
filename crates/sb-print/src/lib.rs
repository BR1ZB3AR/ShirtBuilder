//! ShirtBuilder print renderer (CPU, tiny-skia): everything printed on the fabric —
//! fabric colour or gradient, construction details, text and image artwork, and art running
//! over seams — for the pattern editor, the 3D texture atlas, the 3D backdrop and the
//! print / cut sheet exports. Port of the web app's `draw.ts` and export code.

pub mod backdrop;
pub mod color;
pub mod draw;
pub mod images;
pub mod sheets;
pub mod text;

pub use draw::{atlas_ppc, render_atlas, Mode};
pub use images::ImageCache;
pub use resvg::tiny_skia;

/// image sources used by a document's layers
pub fn image_srcs(doc: &sb_core::doc::Doc) -> impl Iterator<Item = &str> {
    doc.layers.iter().filter_map(|l| l.src.as_deref())
}

/// premultiplied RGBA → straight RGBA (for PNG files and GPU textures expecting straight alpha)
pub fn unpremultiply(data: &mut [u8]) {
    for px in data.chunks_exact_mut(4) {
        let a = px[3] as u32;
        if a > 0 && a < 255 {
            for c in &mut px[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
}
