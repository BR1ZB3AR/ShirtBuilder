//! Heavy lifting off the UI thread: building the 3D geometry and rendering the print atlases.
//! On the desktop a worker thread does it (only the newest request of each kind is worked on);
//! the web build has no threads, so there it runs right away on the UI thread.

use super::textures::{mips, MipImage};
use sb_core::doc::{Doc, Fabric, SceneSettings, Side};
use sb_core::pattern::{garment, GarmentId};
use sb_core::scan::{scan_name_for, ScanModel};
use sb_core::shirt3d::{build_shirt, buttons_mesh, hanger_mesh, Mesh};
use sb_print::ImageCache;
use std::sync::{Arc, OnceLock};

pub enum Cloth {
    Generated(Mesh),
    Scan { mesh: Mesh, out: Vec<u32>, inn: Vec<u32> },
}

#[allow(dead_code)] // some fields are for future use / debugging
pub struct GeomOut {
    pub gen: u64,
    pub garment: GarmentId,
    pub cloth: Cloth,
    pub buttons: Mesh,
    pub hanger: Option<Mesh>,
    pub top_y: f64,
    /// realistic model on screen
    pub scan: Option<&'static str>,
}

#[allow(dead_code)] // some fields are for future use / debugging
pub struct AtlasOut {
    pub gen: u64,
    pub garment: GarmentId,
    pub ppc: f64,
    pub images: [MipImage; 2],
}

pub enum Job {
    Geometry { gen: u64, garment: GarmentId, fabric: Fabric, scene: SceneSettings },
    Atlas { gen: u64, doc: Doc },
}

pub enum Done {
    Geometry(Arc<GeomOut>),
    Atlas(Arc<AtlasOut>),
}

const TEE_M: &[u8] = include_bytes!("../../assets/models/tee-m.bin");
const TEE_F: &[u8] = include_bytes!("../../assets/models/tee-f.bin");

fn scan_model(name: &str) -> Option<&'static ScanModel> {
    static M: OnceLock<Option<ScanModel>> = OnceLock::new();
    static F: OnceLock<Option<ScanModel>> = OnceLock::new();
    let (cell, bytes) = if name == "tee-f" { (&F, TEE_F) } else { (&M, TEE_M) };
    cell.get_or_init(|| match ScanModel::parse(name, bytes) {
        Ok(m) => Some(m),
        Err(e) => {
            log::warn!("realistic model {name} unavailable, using the generated shirt: {e}");
            None
        }
    })
    .as_ref()
}

fn geometry(gen: u64, id: GarmentId, fabric: &Fabric, scene: &SceneSettings) -> GeomOut {
    let g = garment(id);
    let doc_like = Doc { version: 1, garment: id, layers: vec![], fabric: fabric.clone(), scene: scene.clone() };
    if let Some(name) = scan_name_for(&doc_like) {
        if let Some(m) = scan_model(name) {
            let (mesh, out, inn) = m.build(&g);
            return GeomOut { gen, garment: id, cloth: Cloth::Scan { mesh, out, inn }, buttons: Mesh::default(), hanger: None, top_y: g.spec.body_h, scan: Some(name) };
        }
    }
    let b = build_shirt(&g, fabric, !scene.hanger);
    let hanger = scene.hanger.then(|| hanger_mesh(&b));
    GeomOut { gen, garment: id, buttons: buttons_mesh(&b), hanger, top_y: b.top_y, cloth: Cloth::Generated(b.mesh), scan: None }
}

fn atlas(gen: u64, doc: &Doc, imgs: &mut ImageCache) -> Option<AtlasOut> {
    let g = garment(doc.garment);
    imgs.prepare(sb_print::image_srcs(doc));
    imgs.retain(sb_print::image_srcs(doc));
    let ppc = sb_print::atlas_ppc(&g);
    // the realistic model has real hems, stitching and rib: don't paint pattern ones over it
    let details = scan_name_for(doc).is_none();
    let render = |side: Side| -> Option<MipImage> {
        let pm = sb_print::render_atlas(&g, doc, side, ppc, imgs, details)?;
        let (w, h) = (pm.width(), pm.height());
        let mut data = pm.take();
        sb_print::unpremultiply(&mut data);
        Some(mips(w, h, data))
    };
    #[cfg(not(target_arch = "wasm32"))]
    let (a, b) = std::thread::scope(|s| {
        let h = s.spawn(|| render(Side::Inside));
        (render(Side::Outside), h.join().ok().flatten())
    });
    #[cfg(target_arch = "wasm32")]
    let (a, b) = (render(Side::Outside), render(Side::Inside));
    Some(AtlasOut { gen, garment: doc.garment, ppc, images: [a?, b?] })
}

pub struct Worker {
    #[cfg(not(target_arch = "wasm32"))]
    tx: std::sync::mpsc::Sender<Job>,
    #[cfg(not(target_arch = "wasm32"))]
    rx: std::sync::mpsc::Receiver<Done>,
    #[cfg(target_arch = "wasm32")]
    done: Vec<Done>,
    #[cfg(target_arch = "wasm32")]
    imgs: ImageCache,
}

impl Worker {
    pub fn new(ctx: egui::Context) -> Worker {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (tx, jobs) = std::sync::mpsc::channel::<Job>();
            let (done_tx, rx) = std::sync::mpsc::channel::<Done>();
            std::thread::Builder::new()
                .name("shirtbuilder-render".into())
                .spawn(move || {
                    let mut imgs = ImageCache::default();
                    while let Ok(first) = jobs.recv() {
                        // keep only the newest request of each kind
                        let (mut geo, mut atl) = (None, None);
                        for j in std::iter::once(first).chain(jobs.try_iter()) {
                            match j {
                                Job::Geometry { .. } => geo = Some(j),
                                Job::Atlas { .. } => atl = Some(j),
                            }
                        }
                        if let Some(Job::Atlas { gen, doc }) = atl {
                            if let Some(a) = atlas(gen, &doc, &mut imgs) {
                                if done_tx.send(Done::Atlas(Arc::new(a))).is_err() {
                                    return;
                                }
                                ctx.request_repaint();
                            }
                        }
                        if let Some(Job::Geometry { gen, garment, fabric, scene }) = geo {
                            let g = geometry(gen, garment, &fabric, &scene);
                            if done_tx.send(Done::Geometry(Arc::new(g))).is_err() {
                                return;
                            }
                            ctx.request_repaint();
                        }
                    }
                })
                .expect("render thread");
            Worker { tx, rx }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = ctx;
            Worker { done: Vec::new(), imgs: ImageCache::default() }
        }
    }

    pub fn submit(&mut self, job: Job) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = self.tx.send(job);
        }
        #[cfg(target_arch = "wasm32")]
        match job {
            Job::Atlas { gen, doc } => {
                if let Some(a) = atlas(gen, &doc, &mut self.imgs) {
                    self.done.push(Done::Atlas(Arc::new(a)));
                }
            }
            Job::Geometry { gen, garment, fabric, scene } => self.done.push(Done::Geometry(Arc::new(geometry(gen, garment, &fabric, &scene)))),
        }
    }

    pub fn poll(&mut self) -> Vec<Done> {
        #[cfg(not(target_arch = "wasm32"))]
        return self.rx.try_iter().collect();
        #[cfg(target_arch = "wasm32")]
        return std::mem::take(&mut self.done);
    }
}
