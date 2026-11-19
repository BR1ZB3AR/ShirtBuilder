//! The 3D preview: camera (orbit, zoom, pan with damping, animated views), the photo-studio light
//! rig that follows the camera, quality levels, frame-rate stats, the navigation gizmo and
//! snapshots. Geometry and print atlases come from the worker; drawing happens on the GPU
//! (`gpu.rs`). Port of the web app's `ShirtScene` and `ViewGizmo`.

pub mod gpu;
pub mod textures;
pub mod worker;

use crate::app::Quality;
use eframe::egui_wgpu::{self, wgpu};
use egui::{pos2, vec2, Color32, FontId, Pos2, Rect, Sense, Stroke};
use glam::{Mat4, Vec3};
use gpu::{FrameParams, Globals, Gpu3d, Material, Obj, Paint3d, SnapshotState};
use sb_core::doc::{Doc, FabricMode, MaterialId};
use sb_core::pattern::{atlas_rect, garment, GarmentId, SleeveLen};
use sb_print::color::{hex_to_rgb, luminance, mix, shade};
use std::f64::consts::PI;
use std::sync::Arc;
use textures::Weave;
use worker::{AtlasOut, Done, GeomOut, Job, Worker};

/// fabric presets (the web app's MATERIALS)
struct MatPreset {
    roughness: f64,
    sheen: f64,
    sheen_roughness: f64,
    weave: Weave,
    normal: f64,
    tile: f64,
}
fn preset(m: MaterialId) -> MatPreset {
    match m {
        MaterialId::Cotton => MatPreset { roughness: 0.92, sheen: 0.55, sheen_roughness: 0.75, weave: Weave::Knit, normal: 0.45, tile: 0.45 },
        MaterialId::Jersey => MatPreset { roughness: 0.78, sheen: 0.8, sheen_roughness: 0.5, weave: Weave::Knit, normal: 0.35, tile: 0.4 },
        MaterialId::Polyester => MatPreset { roughness: 0.5, sheen: 0.35, sheen_roughness: 0.35, weave: Weave::Knit, normal: 0.3, tile: 0.7 },
        MaterialId::Linen => MatPreset { roughness: 0.96, sheen: 0.25, sheen_roughness: 0.9, weave: Weave::Weave, normal: 0.75, tile: 0.9 },
        MaterialId::Fleece => MatPreset { roughness: 1.0, sheen: 1.0, sheen_roughness: 1.0, weave: Weave::Fleece, normal: 0.45, tile: 2.2 },
    }
}

fn lin(c: f64) -> f32 {
    let c = c / 255.0;
    (if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }) as f32
}
/// `#rrggbb` → linear RGB
fn lin_hex(hex: &str) -> [f32; 3] {
    let [r, g, b] = hex_to_rgb(hex);
    [lin(r), lin(g), lin(b)]
}
fn v4(a: [f32; 3], w: f32) -> [f32; 4] {
    [a[0], a[1], a[2], w]
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Level {
    High,
    Balanced,
    Fast,
}
impl Level {
    pub fn name(self) -> &'static str {
        match self {
            Level::High => "High",
            Level::Balanced => "Balanced",
            Level::Fast => "Fast",
        }
    }
}

/// camera position around the target (three.js Spherical: phi from +y, theta round +y from +z)
#[derive(Clone, Copy, Debug, PartialEq)]
struct Sph {
    r: f64,
    phi: f64,
    theta: f64,
}
impl Sph {
    fn offset(self) -> Vec3 {
        let s = self.phi.sin();
        Vec3::new((self.r * s * self.theta.sin()) as f32, (self.r * self.phi.cos()) as f32, (self.r * s * self.theta.cos()) as f32)
    }
    fn from(v: Vec3) -> Sph {
        let r = v.length() as f64;
        if r == 0.0 {
            return Sph { r: 0.0, phi: 0.0, theta: 0.0 };
        }
        Sph { r, phi: (v.y as f64 / r).clamp(-1.0, 1.0).acos(), theta: (v.x as f64).atan2(v.z as f64) }
    }
}

struct Anim {
    from: Sph,
    to: Sph,
    t0: f64,
}

/// shortest way round from `from` to `to`
fn short_way(from: Sph, mut to: Sph) -> Sph {
    let d = to.theta - from.theta;
    let d = ((d + PI) % (2.0 * PI) + 2.0 * PI) % (2.0 * PI) - PI;
    to.theta = from.theta + d;
    to
}

/// the pattern view borrows the 3D atlas textures (mip-mapped) for its piece fills
#[derive(Clone, Copy)]
pub struct AtlasTex {
    pub garment: GarmentId,
    /// outside, inside
    pub ids: [egui::TextureId; 2],
}

pub struct Viewer3d {
    worker: Worker,
    rs: Option<egui_wgpu::RenderState>,
    gen: u64,
    geom_key: Option<String>,
    atlas_key: Option<String>,
    geom: Option<Arc<GeomOut>>,
    pub atlas: Option<AtlasTex>,
    // camera
    target: Vec3,
    cam: Sph,
    delta: (f64, f64),
    pan: Vec3,
    anim: Option<Anim>,
    built: Option<GarmentId>,
    fov: f64,
    aspect: f64,
    /// pane aspect the camera distance was last fitted for
    fitted_aspect: f64,
    /// the user zoomed by hand: keep their distance when the pane changes shape
    user_zoomed: bool,
    // lights follow the camera, but only once it settles
    light_az: f64,
    lights_placed_for: Option<(f64, f64)>,
    last_move: f64,
    last_frame: f64,
    frame_times: Vec<f64>,
    last_stats: f64,
    pub fps: f64,
    auto_level: Level,
    pub level: Level,
    // backdrop
    bg_key: Option<String>,
    bg_tex: Option<egui::TextureHandle>,
    // gizmo
    gizmo_hover: Option<(usize, bool)>,
    gizmo_drag: Option<Pos2>,
    // snapshot to save, once the GPU has it
    pub snapshot_pending: bool,
    /// whether the GPU renderer is available
    pub gpu_ok: bool,
}

const AXES: [(&str, [f32; 3], Color32); 3] = [
    ("X", [1.0, 0.0, 0.0], Color32::from_rgb(0xff, 0x36, 0x53)),
    ("Y", [0.0, 0.0, -1.0], Color32::from_rgb(0x8a, 0xdb, 0x00)),
    ("Z", [0.0, 1.0, 0.0], Color32::from_rgb(0x2c, 0x8f, 0xff)),
];

impl Viewer3d {
    pub fn new(ctx: &egui::Context, rs: Option<egui_wgpu::RenderState>) -> Viewer3d {
        if let Some(rs) = &rs {
            gpu::install(rs);
        }
        let target = Vec3::new(0.0, 40.0, 0.0);
        Viewer3d {
            worker: Worker::new(ctx.clone()),
            gpu_ok: rs.is_some(),
            rs,
            gen: 0,
            geom_key: None,
            atlas_key: None,
            geom: None,
            atlas: None,
            target,
            cam: Sph::from(Vec3::new(0.0, 44.0, 215.0) - target),
            delta: (0.0, 0.0),
            pan: Vec3::ZERO,
            anim: None,
            built: None,
            fov: 30.0,
            aspect: 1.0,
            fitted_aspect: 1.0,
            user_zoomed: false,
            light_az: f64::NAN,
            lights_placed_for: None,
            last_move: -1e9,
            last_frame: 0.0,
            frame_times: Vec::new(),
            last_stats: 0.0,
            fps: 0.0,
            auto_level: Level::Balanced,
            level: Level::Balanced,
            bg_key: None,
            bg_tex: None,
            gizmo_hover: None,
            gizmo_drag: None,
            snapshot_pending: false,
        }
    }

    fn with_gpu<R>(&self, f: impl FnOnce(&egui_wgpu::RenderState, &mut Gpu3d, &mut egui_wgpu::Renderer) -> R) -> Option<R> {
        let rs = self.rs.as_ref()?;
        let mut renderer = rs.renderer.write();
        // take the GPU state out while the renderer is borrowed for texture registration
        let mut gpu = renderer.callback_resources.remove::<Gpu3d>()?;
        let r = f(rs, &mut gpu, &mut renderer);
        renderer.callback_resources.insert(gpu);
        Some(r)
    }

    /// keep geometry and atlases in step with the document (cheap to call every frame)
    pub fn sync(&mut self, doc: &Doc) {
        let f = &doc.fabric;
        let s = &doc.scene;
        let gk = format!(
            "{}|{}|{}|{}|{}",
            doc.garment,
            if f.wrinkles_on { format!("{:.3}", f.wrinkles) } else { "0".into() },
            f.ao,
            if s.hanger { "hanger" } else { "ghost" },
            sb_core::scan::scan_name_for(doc).unwrap_or("gen")
        );
        if self.geom_key.as_ref() != Some(&gk) {
            self.geom_key = Some(gk);
            self.gen += 1;
            self.worker.submit(Job::Geometry { gen: self.gen, garment: doc.garment, fabric: f.clone(), scene: s.clone() });
        }
        let ak = format!(
            "{}|{}|{}|{:?}|{}|{}|{}",
            doc.garment,
            serde_json::to_string(&doc.layers).unwrap_or_default(),
            f.color,
            f.mode,
            f.color2,
            f.angle,
            sb_core::scan::scan_name_for(doc).unwrap_or("gen") // realistic model: atlas without stitching
        );
        if self.atlas_key.as_ref() != Some(&ak) {
            self.atlas_key = Some(ak);
            self.gen += 1;
            self.worker.submit(Job::Atlas { gen: self.gen, doc: doc.clone() });
        }
        for d in self.worker.poll() {
            match d {
                Done::Geometry(g) => {
                    self.with_gpu(|rs, gpu, _| gpu.set_geometry(&rs.device, &rs.queue, &g));
                    self.frame_garment(g.garment, g.top_y, doc);
                    self.geom = Some(g);
                }
                Done::Atlas(a) => self.upload_atlas(&a),
            }
        }
    }

    fn upload_atlas(&mut self, a: &AtlasOut) {
        let old = self.atlas.take();
        let ids = self.with_gpu(|rs, gpu, renderer| {
            gpu.set_atlas(&rs.device, &rs.queue, &a.images[0], &a.images[1]);
            if let Some(o) = old {
                for id in o.ids {
                    renderer.free_texture(&id);
                }
            }
            let samp = wgpu::SamplerDescriptor {
                label: Some("sb-pattern-atlas"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                anisotropy_clamp: 8,
                ..Default::default()
            };
            let v = gpu.atlas_views_for_egui();
            [renderer.register_native_texture_with_sampler_options(&rs.device, &v[0], samp.clone()), renderer.register_native_texture_with_sampler_options(&rs.device, &v[1], samp)]
        });
        self.atlas = ids.map(|ids| AtlasTex { garment: a.garment, ids });
    }

    /// aim the camera at the garment; re-frames when the garment or its height changes
    fn frame_garment(&mut self, id: GarmentId, top_y: f64, doc: &Doc) {
        let ty = ((top_y + 16.0) / 2.0 - 2.0) as f32;
        if self.target.y != ty || self.built != Some(id) {
            let first = self.built.is_none();
            self.target = Vec3::new(0.0, ty, 0.0);
            self.built = Some(id);
            if first {
                self.pan = Vec3::ZERO;
                self.fitted_aspect = self.aspect;
                self.cam = Sph::from(Vec3::new(0.0, 4.0, self.fit_distance(doc) as f32));
            } else {
                self.reset_view(doc);
            }
        }
    }

    /// camera distance that frames the whole garment for the current viewport aspect
    fn fit_distance(&self, doc: &Doc) -> f64 {
        let spec = self.built.map(|id| garment(id).spec.clone());
        let ghost = !doc.scene.hanger;
        let grow = spec.as_ref().map(|s| s.body_w / 53.0).unwrap_or(1.0);
        let half_w = (if ghost {
            42.0
        } else if spec.as_ref().is_some_and(|s| s.sleeve() == SleeveLen::Long) {
            56.0
        } else {
            46.0
        }) * grow.max(1.0);
        let t = (self.fov.to_radians() / 2.0).tan();
        (half_w / (t * self.aspect.max(0.3))).max(215.0)
    }

    fn animate_to(&mut self, to: Sph, now: f64) {
        self.anim = Some(Anim { from: self.cam, to: short_way(self.cam, to), t0: now });
        self.delta = (0.0, 0.0);
    }

    pub fn set_view(&mut self, view: crate::app::View3d, now: f64) {
        use crate::app::View3d::*;
        let theta = match view {
            Front => 0.0,
            Back => PI,
            Side => PI / 2.0,
            Angle => -0.6,
        };
        self.animate_to(Sph { r: self.cam.r, phi: PI / 2.0 - 0.06, theta }, now);
    }

    fn view_from(&mut self, dir: Vec3, now: f64) {
        let mut to = Sph::from(dir.normalize() * self.cam.r as f32);
        to.phi = to.phi.clamp(0.0005, PI - 0.0005);
        if dir.x.abs() + dir.z.abs() < 1e-6 {
            to.theta = self.cam.theta;
        }
        self.animate_to(to, now);
    }

    /// place the camera directly (tests): distance (cm), polar / azimuth angles, pan from the target
    pub fn set_camera_for_tests(&mut self, r: f64, phi: f64, theta: f64, pan: [f32; 3]) {
        self.anim = None;
        self.user_zoomed = true;
        self.cam = Sph { r, phi, theta };
        self.pan = Vec3::from(pan);
        self.delta = (0.0, 0.0);
    }

    pub fn reset_view(&mut self, doc: &Doc) {
        self.pan = Vec3::ZERO;
        self.user_zoomed = false;
        self.fitted_aspect = self.aspect;
        let to = Sph { r: self.fit_distance(doc), phi: PI / 2.0 - 0.06, theta: 0.0 };
        self.anim = Some(Anim { from: self.cam, to: short_way(self.cam, to), t0: -1.0 });
        self.delta = (0.0, 0.0);
    }

    fn orbit_by(&mut self, d_theta: f64, d_phi: f64, now: f64) {
        self.anim = None;
        self.cam.theta += d_theta;
        self.cam.phi = (self.cam.phi + d_phi).clamp(0.0005, PI - 0.0005);
        self.last_move = now;
    }

    pub fn set_quality(&mut self, q: Quality) {
        self.level = match q {
            Quality::Auto => self.auto_level,
            Quality::High => Level::High,
            Quality::Balanced => Level::Balanced,
            Quality::Fast => Level::Fast,
        };
    }

    /// measure frame times while moving; Auto steps quality down on slow machines
    fn record_frame(&mut self, now: f64, q: Quality) {
        if self.last_frame > 0.0 && now - self.last_frame < 2.0 {
            self.frame_times.push(now - self.last_frame);
            if self.frame_times.len() > 40 {
                self.frame_times.remove(0);
            }
        }
        self.last_frame = now;
        let n = self.frame_times.len();
        if n == 0 {
            return;
        }
        let span: f64 = self.frame_times.iter().sum();
        let avg = span / n as f64;
        if now - self.last_stats > 0.25 {
            self.last_stats = now;
            self.fps = (1.0 / avg).round();
        }
        if q == Quality::Auto && n >= 6 && span > 1.5 && avg > 0.028 && self.level != Level::Fast {
            self.auto_level = if self.level == Level::High { Level::Balanced } else { Level::Fast };
            log::info!("3D quality: {:.0} fps while rotating → stepping down to {}", 1.0 / avg, self.auto_level.name());
            self.level = self.auto_level;
            self.frame_times.clear();
        }
    }

    /// queue a high-resolution still of the current view
    pub fn request_snapshot(&mut self, doc: &Doc) {
        let w = 2000u32;
        let h = (w as f64 / self.aspect.max(0.2)).round() as u32;
        let mut params = self.frame_params(doc, [w, h], true);
        params.size = [w, h];
        if self.with_gpu(|_, gpu, _| *gpu.snapshot.lock().unwrap() = Some(SnapshotState::Requested { w, h, params: Box::new(params) })).is_some() {
            self.snapshot_pending = true;
        }
    }

    /// a finished snapshot as PNG bytes, composited over the backdrop
    pub fn take_snapshot(&mut self, doc: &Doc) -> Option<Result<Vec<u8>, String>> {
        if !self.snapshot_pending {
            return None;
        }
        let done = self.with_gpu(|_, gpu, _| {
            let mut s = gpu.snapshot.lock().unwrap();
            match s.take() {
                Some(SnapshotState::Done(r)) => Some(r),
                other => {
                    *s = other;
                    None
                }
            }
        })??;
        self.snapshot_pending = false;
        Some(done.and_then(|(w, h, rgba)| composite_snapshot(doc, w, h, rgba)))
    }

    fn place_lights(&mut self, doc: &Doc) -> bool {
        let off = self.cam.offset();
        let az = (off.x as f64).atan2(off.z as f64);
        let key = (az, doc.scene.light_angle);
        if self.lights_placed_for.is_some_and(|k| (k.0 - key.0).abs() < 1e-4 && k.1 == key.1) {
            return false;
        }
        self.lights_placed_for = Some(key);
        self.light_az = az;
        true
    }

    fn frame_params(&self, doc: &Doc, size: [u32; 2], still: bool) -> FrameParams {
        let f = &doc.fabric;
        let s = &doc.scene;
        let g = garment(doc.garment);
        let level = if still { Level::High } else { self.level };
        let t = self.target + self.pan;
        let cam_pos = t + self.cam.offset();
        let aspect = size[0] as f32 / size[1].max(1) as f32;
        let view = Mat4::look_at_rh(cam_pos, t, Vec3::Y);
        let proj = Mat4::perspective_rh((self.fov as f32).to_radians(), aspect, 1.0, 3000.0);
        // the studio light rig (fixed relative to the camera's heading)
        let az = if self.light_az.is_nan() { 0.0 } else { self.light_az };
        let a = s.light_angle.to_radians() + az;
        let tt = self.target;
        let key_pos = tt + Vec3::new((a.sin() * 220.0) as f32, 210.0, (a.cos() * 220.0) as f32);
        let fill_pos = tt + Vec3::new(((az - (a - az)).sin() * 200.0) as f32, 60.0, ((az - (a - az)).cos() * 200.0) as f32);
        let rim_pos = tt + Vec3::new(((az + PI + 0.5).sin() * 200.0) as f32, 140.0, ((az + PI + 0.5).cos() * 200.0) as f32);
        let gobo_pos = tt + Vec3::new(((a - 0.5).sin() * 300.0) as f32, 330.0, ((a - 0.5).cos() * 300.0) as f32);
        let scan = self.geom.as_ref().and_then(|g| g.scan);
        let gob = s.gobo && level != Level::Fast;
        let light = s.light as f32;
        let dir = |p: Vec3, i: f32| {
            let d = (p - tt).normalize();
            [d.x, d.y, d.z, i]
        };
        let key_vp = Mat4::orthographic_rh(-75.0, 75.0, -75.0, 75.0, 10.0, 600.0) * Mat4::look_at_rh(key_pos, tt, Vec3::Y);
        let gobo_vp = Mat4::perspective_rh(0.48, 1.0, 80.0, 900.0) * Mat4::look_at_rh(gobo_pos, tt, Vec3::Y);
        let key_shadows = s.shadows && level != Level::Fast && (scan.is_none() || level == Level::High);
        let gd = (tt - gobo_pos).normalize();
        let ground = lin_hex("#50554f");
        let hemi = 0.32 * light;
        let globals = Globals {
            view_proj: (proj * view).to_cols_array_2d(),
            key_vp: key_vp.to_cols_array_2d(),
            gobo_vp: gobo_vp.to_cols_array_2d(),
            cam_pos: [cam_pos.x, cam_pos.y, cam_pos.z, 1.0],
            key: dir(key_pos, (if gob { 0.75 } else { 2.5 }) * light),
            fill: dir(fill_pos, 0.4 * light),
            rim: dir(rim_pos, 1.1 * light),
            sky: [hemi, hemi, hemi, 1.0],
            ground: [ground[0] * hemi, ground[1] * hemi, ground[2] * hemi, 1.0],
            gobo_pos: [gobo_pos.x, gobo_pos.y, gobo_pos.z, if gob { 4.6 * light } else { 0.0 }],
            gobo_dir: [gd.x, gd.y, gd.z, 0.24f32.cos()],
            gobo_col: v4(lin_hex("#fff4e6"), (0.24f32 * (1.0 - 0.35)).cos()),
            env: [(0.32 * s.env) as f32, s.exposure as f32, f.brightness as f32, f.contrast as f32],
            flags: [key_shadows as u8 as f32, gob as u8 as f32, 1.0 / gpu::KEY_SHADOW as f32, 1.0 / gpu::GOBO_SHADOW as f32],
        };
        // materials
        let p = preset(f.material);
        let (_, _, aw, ah) = atlas_rect(&g);
        let tile = p.tile * (f.knit / 3.0);
        let sheen = if level == Level::Fast { 0.0 } else { p.sheen };
        let sheen_col = v4(lin_hex(&mix(&f.color, "#ffffff", 0.22)), 1.0);
        let avg = if f.mode == FabricMode::Gradient { mix(&f.color, &f.color2, 0.5) } else { f.color.clone() };
        let em = lin_hex(&avg).map(|c| c * 0.14);
        let inner = lin(0xc6 as f64);
        let ns = (p.normal * f.texture * 2.0) as f32;
        let cloth = Material {
            tint_out: [1.0, 1.0, 1.0, 0.0],
            tint_in: [inner, inner, inner, 1.0],
            emissive_in: v4(em, 1.0),
            solid: [1.0; 4],
            pbr: [p.roughness as f32, sheen as f32, p.sheen_roughness as f32, 0.0],
            sheen_col,
            nmap: [(aw / tile) as f32, (ah / tile) as f32, ns, ns * 0.5],
            misc: [1.0, 1.0, 1.0, 0.0],
        };
        // knit strength on the realistic model: off at Texture 0, as before from the default 0.5 up
        let sns = (if f.texture < 0.5 { 1.8 * f.texture } else { 0.35 + 1.1 * f.texture }) as f32;
        let scan_ao = f.ao as u8 as f32;
        // misc.w: knit tiles per cm, laid straight onto the model's surface (triplanar) — its pattern
        // coordinates are too uneven for a fine repeating texture
        let scan_out = Material { tint_out: [1.0, 1.0, 1.0, 1.0], nmap: [cloth.nmap[0], cloth.nmap[1], sns, sns], misc: [scan_ao, 0.0, 1.0, (1.0 / tile) as f32], ..cloth };
        let scan_in = Material { tint_out: [1.0, 1.0, 1.0, 2.0], ..scan_out };
        let btn_col = if luminance(&f.color) > 0.35 { shade(&f.color, 0.8) } else { mix(&f.color, "#ffffff", 0.14) };
        let solid = |c: [f32; 3], cc: f32| Material { tint_out: [1.0, 1.0, 1.0, 3.0], solid: v4(c, 1.0), pbr: [0.32, 0.0, 0.0, cc], misc: [0.0, 0.0, 1.0, 0.0], ..Default::default() };
        let materials = [cloth, scan_out, scan_in, solid(lin_hex(&btn_col), 0.7), solid(lin_hex("#f3f3f1"), 0.6)];
        let (draws, casters) = if scan.is_some() {
            (vec![Obj::ScanOut, Obj::ScanIn], if level == Level::High || still { vec![Obj::ScanOut] } else { vec![] })
        } else {
            let mut d = vec![Obj::Cloth, Obj::Buttons];
            if s.hanger {
                d.push(Obj::Hanger);
            }
            (d.clone(), d)
        };
        FrameParams { globals, materials, draws, casters, key_shadows, gobo: gob, weave: p.weave, size }
    }

    /// draw the 3D view into `rect` (the backdrop, then the GPU image) and handle input
    pub fn show(&mut self, ui: &mut egui::Ui, rect: Rect, doc: &Doc, quality: Quality) -> egui::Response {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        let dt = ctx.input(|i| i.stable_dt as f64).min(0.1);
        self.set_quality(quality);
        self.aspect = (rect.width() / rect.height().max(1.0)) as f64;

        // ---- input: left-drag orbits, right/middle-drag (or Shift) pans, wheel zooms
        let resp = ui.interact(rect, ui.scope_id().with("3d-view"), Sense::click_and_drag());
        let h = rect.height().max(1.0) as f64;
        if resp.dragged() {
            let d = resp.drag_delta();
            let pan = resp.dragged_by(egui::PointerButton::Secondary) || resp.dragged_by(egui::PointerButton::Middle) || ctx.input(|i| i.modifiers.shift);
            self.anim = None;
            if pan {
                let dist = self.cam.r * (self.fov.to_radians() / 2.0).tan();
                let off = self.cam.offset();
                let right = Vec3::Y.cross(off).normalize();
                let up = off.cross(right).normalize();
                self.pan += (right * -(d.x as f64 * 2.0 * dist / h) as f32) + (up * (d.y as f64 * 2.0 * dist / h) as f32);
            } else {
                self.delta.0 -= 2.0 * PI * d.x as f64 / h;
                self.delta.1 -= 2.0 * PI * d.y as f64 / h;
            }
            self.last_move = now;
        }
        if resp.hovered() {
            let scroll = ctx.input(|i| i.smooth_scroll_delta.y + (i.zoom_delta() - 1.0) * 400.0) as f64;
            if scroll.abs() > 0.01 {
                let k = 0.95f64.powf(scroll.abs() * 0.05);
                self.cam.r = if scroll > 0.0 { self.cam.r * k } else { self.cam.r / k }.clamp(60.0, 520.0);
                self.anim = None;
                self.user_zoomed = true;
                self.last_move = now;
            }
            ctx.set_cursor_icon(if resp.dragged() { egui::CursorIcon::Grabbing } else { egui::CursorIcon::Grab });
        }
        // the pane changed shape (layout switch, window resize): re-fit the distance unless the
        // user chose their own zoom
        if self.built.is_some() && !self.user_zoomed && self.anim.is_none() && (self.aspect / self.fitted_aspect - 1.0).abs() > 0.05 {
            self.fitted_aspect = self.aspect;
            let to = Sph { r: self.fit_distance(doc), ..self.cam };
            self.anim = Some(Anim { from: self.cam, to, t0: now });
        }
        if doc.scene.auto_rotate {
            self.cam.theta -= 2.0 * PI / 60.0 * 1.6 * dt;
            self.last_move = now;
        }
        // damping (three.js OrbitControls, 0.08 per 60 Hz frame)
        let k = 1.0 - (1.0 - 0.08f64).powf(dt * 60.0);
        if self.delta.0.abs() + self.delta.1.abs() > 1e-5 {
            self.cam.theta += self.delta.0 * k;
            self.cam.phi = (self.cam.phi + self.delta.1 * k).clamp(0.0005, PI - 0.0005);
            self.delta = (self.delta.0 * (1.0 - k), self.delta.1 * (1.0 - k));
            self.last_move = now;
        }
        if let Some(a) = &mut self.anim {
            if a.t0 < 0.0 {
                a.t0 = now;
            }
            let t = ((now - a.t0) / 0.65).min(1.0);
            let e = if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 };
            self.cam = Sph { r: a.from.r + (a.to.r - a.from.r) * e, phi: a.from.phi + (a.to.phi - a.from.phi) * e, theta: a.from.theta + (a.to.theta - a.from.theta) * e };
            if t >= 1.0 {
                self.anim = None;
            }
            self.last_move = now;
        }
        let moving = now - self.last_move < 0.14;
        // lights swing back in front of the camera once it settles (fresh shadows then)
        let lights_moved = if moving { false } else { self.place_lights(doc) };
        if moving {
            self.record_frame(now, quality);
            ctx.request_repaint();
        } else {
            self.last_frame = 0.0;
            if now - self.last_move < 0.5 {
                ctx.request_repaint(); // one settled, full-quality frame
            }
        }

        // ---- backdrop
        let bk = format!("{:?}|{}|{}|{}", doc.scene.bg, doc.scene.match_fabric, doc.scene.bg_color, if doc.scene.match_fabric { doc.fabric.color.as_str() } else { "" });
        if self.bg_key.as_ref() != Some(&bk) {
            self.bg_key = Some(bk);
            let pm = sb_print::backdrop::render_backdrop(&doc.scene, &doc.fabric.color);
            let img = egui::ColorImage::from_rgba_premultiplied([pm.width() as usize, pm.height() as usize], pm.data());
            self.bg_tex = Some(ctx.load_texture("3d-backdrop", img, egui::TextureOptions::LINEAR));
        }
        let painter = ui.painter_at(rect);
        if let Some(tex) = &self.bg_tex {
            // cover: fill the pane, keep the aspect, centred
            let (bw, bh) = (sb_print::backdrop::W as f32, sb_print::backdrop::H as f32);
            let s = (rect.width() / bw).max(rect.height() / bh);
            let r = Rect::from_center_size(rect.center(), vec2(bw * s, bh * s));
            painter.image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }

        // ---- the shirt
        if self.gpu_ok && self.geom.is_some() {
            let ppp = ctx.pixels_per_point();
            let k = match (self.level, moving) {
                (Level::High, false) => ppp.min(2.0),
                (Level::High, true) => ppp.min(1.0),
                (Level::Balanced, false) => ppp.min(1.5),
                (Level::Balanced, true) => ppp.min(1.0) * 0.85,
                (Level::Fast, false) => 1.0,
                (Level::Fast, true) => 0.75,
            };
            let size = [(rect.width() * k).round().max(1.0) as u32, (rect.height() * k).round().max(1.0) as u32];
            let params = self.frame_params(doc, size, false);
            painter.add(egui_wgpu::Callback::new_paint_callback(rect, Paint3d { params, shadows: lights_moved }));
        } else if !self.gpu_ok {
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, "3D preview needs a graphics card (wgpu)", FontId::proportional(13.0), Color32::WHITE);
        } else {
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, "Building the 3D shirt…", FontId::proportional(13.0), Color32::from_white_alpha(200));
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        resp
    }

    /// Blender-style navigation gizmo: axis balls turning with the camera; click a ball to view
    /// from that axis, drag to orbit, the house button resets the view
    pub fn gizmo(&mut self, ui: &mut egui::Ui, centre: Pos2, doc: &Doc) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        const R: f32 = 36.0;
        const BALL: f32 = 9.0;
        const RING: f32 = 6.5;
        let area = Rect::from_center_size(centre, vec2(104.0, 104.0));
        let resp = ui.interact(area, ui.scope_id().with("gizmo"), Sense::click_and_drag());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "View orientation: click an axis to view from it, drag to orbit"));
        let view = Mat4::look_at_rh(self.cam.offset(), Vec3::ZERO, Vec3::Y);
        let mut hits: Vec<(Pos2, f32, usize, bool)> = Vec::new();
        for (i, (_, d, _)) in AXES.iter().enumerate() {
            for positive in [true, false] {
                let dir = Vec3::from_array(*d) * if positive { 1.0 } else { -1.0 };
                let v = view.transform_vector3(dir);
                hits.push((centre + vec2(v.x * R, -v.y * R), v.z, i, positive));
            }
        }
        hits.sort_by(|a, b| a.1.total_cmp(&b.1)); // far → near
        let pick = |p: Pos2| hits.iter().rev().find(|h| h.0.distance(p) <= (if h.3 { BALL } else { RING }) + 3.0).map(|h| (h.2, h.3));
        self.gizmo_hover = resp.hover_pos().and_then(pick);
        if resp.drag_started() {
            self.gizmo_drag = resp.interact_pointer_pos();
        }
        if resp.dragged() && resp.drag_delta().length() > 0.0 {
            let d = resp.drag_delta();
            self.orbit_by(-d.x as f64 * 0.012, -d.y as f64 * 0.012, now);
        }
        if resp.clicked() {
            if let Some((i, positive)) = resp.interact_pointer_pos().and_then(pick) {
                let dir = Vec3::from_array(AXES[i].1) * if positive { 1.0 } else { -1.0 };
                self.view_from(dir, now);
            }
        }
        let p = ui.painter();
        if resp.hovered() || resp.dragged() {
            p.circle_filled(centre, R + BALL + 4.0, Color32::from_white_alpha(23));
            ctx.set_cursor_icon(if self.gizmo_hover.is_some() { egui::CursorIcon::PointingHand } else { egui::CursorIcon::Grab });
        }
        for (pos, z, i, positive) in &hits {
            let (label, _, col) = AXES[*i];
            let hover = self.gizmo_hover == Some((*i, *positive));
            let fade = 0.55 + 0.45 * ((z + 1.0) / 2.0);
            if *positive {
                p.line_segment([centre, *pos], Stroke::new(2.0, col.gamma_multiply(fade)));
                p.circle_filled(*pos, BALL + if hover { 1.5 } else { 0.0 }, col.gamma_multiply(fade));
                if hover {
                    p.circle_stroke(*pos, BALL + 1.5, Stroke::new(1.5, Color32::WHITE));
                }
                p.text(*pos + vec2(0.0, 0.5), egui::Align2::CENTER_CENTER, label, FontId::new(11.0, crate::style::semibold()), Color32::WHITE);
            } else {
                let fill = if hover { col.gamma_multiply(0.9) } else { Color32::from_rgba_unmultiplied(20, 22, 30, (0.55 * fade * 255.0) as u8) };
                p.circle_filled(*pos, RING + if hover { 1.5 } else { 0.0 }, fill);
                p.circle_stroke(*pos, RING + if hover { 1.5 } else { 0.0 }, Stroke::new(2.0, col.gamma_multiply(fade)));
                if hover {
                    p.text(*pos + vec2(0.0, 0.5), egui::Align2::CENTER_CENTER, format!("-{label}"), FontId::new(9.0, crate::style::semibold()), Color32::WHITE);
                }
            }
        }
        // home button
        let home = Rect::from_center_size(centre + vec2(R + 8.0, R + 8.0), vec2(22.0, 22.0));
        let hr = ui.interact(home, ui.scope_id().with("gizmo-home"), Sense::click()).on_hover_text("Reset view");
        hr.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Reset view"));
        p.circle_filled(home.center(), 11.0, if hr.hovered() { crate::style::P.panel2 } else { crate::style::P.chip });
        p.text(home.center(), egui::Align2::CENTER_CENTER, crate::style::icon(lucide_icons::Icon::House), FontId::new(12.0, crate::style::icons()), Color32::WHITE);
        if hr.clicked() {
            self.reset_view(doc);
        }
    }

    /// camera distance from its target (cm) — for tests
    pub fn camera_distance(&self) -> f64 {
        self.cam.r
    }

    /// `geom` exists: the garment's realistic model in use
    pub fn active_scan(&self) -> Option<&'static str> {
        self.geom.as_ref().and_then(|g| g.scan)
    }
}

/// the snapshot over the backdrop (cover-fitted), as PNG bytes
fn composite_snapshot(doc: &Doc, w: u32, h: u32, rgba: Vec<u8>) -> Result<Vec<u8>, String> {
    use sb_print::tiny_skia::{FilterQuality, IntSize, Pixmap, PixmapPaint, Transform};
    let bg = sb_print::backdrop::render_backdrop(&doc.scene, &doc.fabric.color);
    let mut out = Pixmap::new(w, h).ok_or("snapshot size")?;
    let s = (w as f32 / bg.width() as f32).max(h as f32 / bg.height() as f32);
    let ts = Transform::from_row(s, 0.0, 0.0, s, (w as f32 - bg.width() as f32 * s) / 2.0, (h as f32 - bg.height() as f32 * s) / 2.0);
    out.draw_pixmap(0, 0, bg.as_ref(), &PixmapPaint { quality: FilterQuality::Bicubic, ..Default::default() }, ts, None);
    // the GPU image comes back straight-alpha; tiny-skia wants premultiplied
    let mut pre = rgba;
    for px in pre.chunks_exact_mut(4) {
        let a = px[3] as u32;
        for c in &mut px[..3] {
            *c = ((*c as u32 * a + 127) / 255) as u8;
        }
    }
    let shirt = Pixmap::from_vec(pre, IntSize::from_wh(w, h).ok_or("snapshot size")?).ok_or("snapshot data")?;
    out.draw_pixmap(0, 0, shirt.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
    sb_print::sheets::png(out)
}
