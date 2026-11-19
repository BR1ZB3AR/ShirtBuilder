//! Realistic (scanned) garment models, baked by the web app's `tools/bake.html` (same `.bin`
//! files as the web app's `public/models`). Each vertex carries its pattern piece and piece-local
//! position (cm, size M), so the same texture atlas the generated shirt uses — fabric colour,
//! prints, inside prints — maps onto it. Port of the web app's `scan.ts`.

use crate::doc::{Doc, ModelKind};
use crate::pattern::{atlas_rect, garment, Fit, Garment, GarmentId, PieceId, Size, SleeveLen, Style};
use crate::shirt3d::Mesh;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanMeta {
    pub version: u32,
    pub garment: String,
    pub vertices: usize,
    pub out_tris: usize,
    pub in_tris: usize,
    pub pieces: Vec<PieceId>,
    pub sleeve_deg: f64,
    pub base_mean: f64,
    #[serde(default)]
    pub has_base: Option<bool>,
    pub has_normal: bool,
    pub layout: HashMap<String, usize>,
}

pub struct ScanModel {
    pub name: String,
    pub meta: ScanMeta,
    pub pos: Vec<[f32; 3]>,
    pub nrm: Vec<[f32; 3]>,
    pub loc: Vec<[f32; 2]>,
    pub piece: Vec<u8>,
    /// per-vertex ambient occlusion (models without a baked shading texture)
    pub ao: Option<Vec<f32>>,
    pub out: Vec<u32>,
    pub inn: Vec<u32>,
}

/// which realistic model (if any) the document should be shown with
pub fn scan_name_for(doc: &Doc) -> Option<&'static str> {
    if doc.scene.model == ModelKind::Generated || doc.scene.hanger {
        return None;
    }
    has_scan_model(doc.garment).then_some(if doc.garment.fit == Fit::F { "tee-f" } else { "tee-m" })
}

pub fn has_scan_model(id: GarmentId) -> bool {
    id.style == Style::Tee && id.sleeve == SleeveLen::Short
}

fn read<T: Copy>(buf: &[u8], off: usize, n: usize, conv: impl Fn(&[u8]) -> T, size: usize) -> Result<Vec<T>, String> {
    let end = off + n * size;
    if end > buf.len() {
        return Err(format!("model data ends early ({end} > {})", buf.len()));
    }
    Ok(buf[off..end].chunks_exact(size).map(conv).collect())
}

impl ScanModel {
    pub fn parse(name: &str, buf: &[u8]) -> Result<ScanModel, String> {
        if buf.len() < 4 {
            return Err("empty model file".into());
        }
        let head = u32::from_le_bytes(buf[0..4].try_into().unwrap()) as usize;
        let meta: ScanMeta = serde_json::from_slice(buf.get(4..4 + head).ok_or("bad header")?).map_err(|e| format!("model header: {e}"))?;
        let n = meta.vertices;
        let mut o = 4 + head;
        let len = |k: &str| meta.layout.get(k).copied().unwrap_or(0);
        let f32s = |b: &[u8]| f32::from_le_bytes(b.try_into().unwrap());
        let pos_flat = read(buf, o, n * 3, f32s, 4)?;
        o += len("pos");
        let nrm_flat = read(buf, o, n * 3, |b| b[0] as i8, 1)?;
        o += len("nrm");
        o += len("uv"); // the model's own UVs (for baked textures; these models have none)
        let loc_flat = read(buf, o, n * 2, f32s, 4)?;
        o += len("loc");
        let piece = read(buf, o, n, |b| b[0], 1)?;
        o += len("piece");
        let ao = if len("ao") > 0 {
            let v = read(buf, o, n, |b| b[0] as f32 / 255.0, 1)?;
            o += len("ao");
            Some(v)
        } else {
            None
        };
        let u32s = |b: &[u8]| u32::from_le_bytes(b.try_into().unwrap());
        let out = read(buf, o, meta.out_tris * 3, u32s, 4)?;
        o += len("out");
        let inn = read(buf, o, meta.in_tris * 3, u32s, 4)?;
        if let Some(bad) = out.iter().chain(&inn).find(|&&i| i as usize >= n) {
            return Err(format!("triangle index {bad} out of range"));
        }
        let pos = pos_flat.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
        let nrm = nrm_flat
            .chunks_exact(3)
            .map(|c| {
                // three.js normalised Int8 attribute: max(v / 127, -1)
                let f = |v: i8| (v as f32 / 127.0).max(-1.0);
                [f(c[0]), f(c[1]), f(c[2])]
            })
            .collect();
        let loc = loc_flat.chunks_exact(2).map(|c| [c[0], c[1]]).collect();
        let has_base = meta.has_base.unwrap_or(true);
        Ok(ScanModel { name: name.into(), meta, pos, nrm, loc, piece, ao: if has_base { None } else { ao }, out, inn })
    }

    /// Geometry for garment `g`: the model (baked at size M) is scaled to the graded size and each
    /// vertex's pattern position is rescaled to the current pieces and turned into atlas UVs.
    /// Returns the outside and inside surfaces (shared vertices, separate triangles).
    pub fn build(&self, g: &Garment) -> (Mesh, Vec<u32>, Vec<u32>) {
        let id = g.spec.id;
        let gm = garment(GarmentId { style: Style::Tee, sleeve: SleeveLen::Short, fit: id.fit, size: Size::M });
        let kx = (g.spec.body_w / gm.spec.body_w) as f32;
        let ky = (g.spec.body_h / gm.spec.body_h) as f32;
        let (ax0, ay0, aw, ah) = atlas_rect(g);
        let pieces: Vec<Option<(&crate::pattern::Piece, &crate::pattern::Piece)>> = self.meta.pieces.iter().map(|pid| g.piece(*pid).zip(gm.piece(*pid))).collect();
        let n = self.meta.vertices;
        let mut m = Mesh { pos: Vec::with_capacity(n), nrm: self.nrm.clone(), uv: Vec::with_capacity(n), ao: self.ao.clone().unwrap_or_else(|| vec![1.0; n]), idx: Vec::new() };
        for i in 0..n {
            let p = self.pos[i];
            m.pos.push([p[0] * kx, p[1] * ky, p[2] * kx]);
            let uv = match pieces.get(self.piece[i] as usize).copied().flatten() {
                Some((p, pm)) => {
                    let px = self.loc[i][0] as f64 * (p.w / pm.w);
                    let py = self.loc[i][1] as f64 * (p.h / pm.h);
                    [((p.x + px - ax0) / aw) as f32, (1.0 - (p.y + py - ay0) / ah) as f32]
                }
                None => [0.0, 0.0],
            };
            m.uv.push(uv);
        }
        (m, self.out.clone(), self.inn.clone())
    }
}
