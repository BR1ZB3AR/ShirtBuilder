//! GPU side of the 3D preview (wgpu, through egui's paint callbacks): shadow maps, the shaded
//! scene rendered off-screen with 4× MSAA, then blitted into egui's render pass. Renders only
//! when something changed; otherwise the last image is blitted again.

use super::textures::{MipImage, Weave};
use bytemuck::{Pod, Zeroable};
use eframe::egui_wgpu::{self, wgpu, CallbackResources, CallbackTrait, ScreenDescriptor};
use sb_core::shirt3d::Mesh;
use std::sync::{Arc, Mutex};

pub const MSAA: u32 = 4;
const COLOR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
pub const KEY_SHADOW: u32 = 2048;
pub const GOBO_SHADOW: u32 = 1024;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Default, PartialEq)]
pub struct Globals {
    pub view_proj: [[f32; 4]; 4],
    pub key_vp: [[f32; 4]; 4],
    pub gobo_vp: [[f32; 4]; 4],
    pub cam_pos: [f32; 4],
    pub key: [f32; 4],
    pub fill: [f32; 4],
    pub rim: [f32; 4],
    pub sky: [f32; 4],
    pub ground: [f32; 4],
    pub gobo_pos: [f32; 4],
    pub gobo_dir: [f32; 4],
    pub gobo_col: [f32; 4],
    pub env: [f32; 4],
    pub flags: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Default, PartialEq)]
pub struct Material {
    pub tint_out: [f32; 4],
    pub tint_in: [f32; 4],
    pub emissive_in: [f32; 4],
    pub solid: [f32; 4],
    pub pbr: [f32; 4],
    pub sheen_col: [f32; 4],
    pub nmap: [f32; 4],
    pub misc: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    pos: [f32; 3],
    nrm: [f32; 3],
    uv: [f32; 2],
    ao: f32,
}

/// which object a draw belongs to (each has its own material)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Obj {
    /// generated shirt, both sides in one draw
    Cloth,
    /// realistic model, outside surface
    ScanOut,
    /// realistic model, inside surface
    ScanIn,
    Buttons,
    Hanger,
}
const OBJS: [Obj; 5] = [Obj::Cloth, Obj::ScanOut, Obj::ScanIn, Obj::Buttons, Obj::Hanger];
fn obj_index(o: Obj) -> usize {
    OBJS.iter().position(|x| *x == o).unwrap()
}

/// what one frame asks the GPU to do
#[derive(Clone, PartialEq)]
pub struct FrameParams {
    pub globals: Globals,
    pub materials: [Material; 5],
    pub draws: Vec<Obj>,
    /// objects casting real-time shadows
    pub casters: Vec<Obj>,
    pub key_shadows: bool,
    pub gobo: bool,
    pub weave: Weave,
    /// render target size in pixels
    pub size: [u32; 2],
}

struct GpuMesh {
    vbuf: wgpu::Buffer,
    ibuf: wgpu::Buffer,
    /// index ranges: outside / inside for the realistic model, everything for the rest
    ranges: Vec<std::ops::Range<u32>>,
}

struct Target {
    size: [u32; 2],
    msaa: wgpu::TextureView,
    resolve: wgpu::TextureView,
    depth: wgpu::TextureView,
    blit_bg: wgpu::BindGroup,
}

pub struct Gpu3d {
    main_none: wgpu::RenderPipeline,
    main_back: wgpu::RenderPipeline,
    shadow_none: wgpu::RenderPipeline,
    shadow_back: wgpu::RenderPipeline,
    blit: wgpu::RenderPipeline,
    blit_bgl: wgpu::BindGroupLayout,
    mat_bgl: wgpu::BindGroupLayout,
    globals_buf: wgpu::Buffer,
    globals_bg: wgpu::BindGroup,
    key_map: wgpu::TextureView,
    gobo_map: wgpu::TextureView,
    su_key: (wgpu::Buffer, wgpu::BindGroup),
    su_gobo: (wgpu::Buffer, wgpu::BindGroup),
    mat_bufs: Vec<wgpu::Buffer>,
    mat_bgs: Vec<Option<wgpu::BindGroup>>,
    atlas: [wgpu::TextureView; 2],
    atlas_size: [u32; 2],
    atlas_samp: wgpu::Sampler,
    nrm: Vec<(Weave, wgpu::TextureView)>,
    nrm_samp: wgpu::Sampler,
    lin_samp: wgpu::Sampler,
    white: wgpu::TextureView,
    cloth: Option<GpuMesh>,
    scan: Option<GpuMesh>,
    buttons: Option<GpuMesh>,
    hanger: Option<GpuMesh>,
    target: Option<Target>,
    last: Option<FrameParams>,
    weave_bound: Option<Weave>,
    /// geometry or textures changed since the last render
    dirty: bool,
    shadows_dirty: bool,
    pub snapshot: Arc<Mutex<Option<SnapshotState>>>,
}

pub enum SnapshotState {
    Requested { w: u32, h: u32, params: Box<FrameParams> },
    Done(Result<(u32, u32, Vec<u8>), String>),
}

fn tex_view(device: &wgpu::Device, queue: &wgpu::Queue, img: &MipImage, format: wgpu::TextureFormat, label: &str) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: img.w, height: img.h, depth_or_array_layers: 1 },
        mip_level_count: img.levels.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    write_mips(queue, &tex, img);
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

fn write_mips(queue: &wgpu::Queue, tex: &wgpu::Texture, img: &MipImage) {
    let (mut w, mut h) = (img.w, img.h);
    for (level, data) in img.levels.iter().enumerate() {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: tex, mip_level: level as u32, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            data,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * w), rows_per_image: Some(h) },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        w = (w / 2).max(1);
        h = (h / 2).max(1);
    }
}

fn depth_target(device: &wgpu::Device, size: u32, label: &str) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

fn uniform_entry(binding: u32, vis: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry { binding, visibility: vis, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None }
}
fn tex_entry(binding: u32, sample: wgpu::TextureSampleType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture { sample_type: sample, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
        count: None,
    }
}
fn samp_entry(binding: u32, ty: wgpu::SamplerBindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(ty), count: None }
}
fn buffer(device: &wgpu::Device, label: &str, size: u64, usage: wgpu::BufferUsages) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false })
}

impl Gpu3d {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, target_format: wgpu::TextureFormat) -> Gpu3d {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("sb-scene"), source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()) });
        let blit_module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("sb-blit"), source: wgpu::ShaderSource::Wgsl(include_str!("blit.wgsl").into()) });
        let vf = wgpu::ShaderStages::VERTEX_FRAGMENT;
        let globals_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sb-globals"),
            entries: &[
                uniform_entry(0, vf),
                tex_entry(1, wgpu::TextureSampleType::Depth),
                tex_entry(2, wgpu::TextureSampleType::Depth),
                samp_entry(3, wgpu::SamplerBindingType::Comparison),
                tex_entry(4, wgpu::TextureSampleType::Float { filterable: true }),
                samp_entry(5, wgpu::SamplerBindingType::Filtering),
            ],
        });
        let mat_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sb-material"),
            entries: &[
                uniform_entry(0, vf),
                tex_entry(1, wgpu::TextureSampleType::Float { filterable: true }),
                tex_entry(2, wgpu::TextureSampleType::Float { filterable: true }),
                samp_entry(3, wgpu::SamplerBindingType::Filtering),
                tex_entry(4, wgpu::TextureSampleType::Float { filterable: true }),
                samp_entry(5, wgpu::SamplerBindingType::Filtering),
            ],
        });
        let su_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("sb-shadow-u"), entries: &[uniform_entry(0, wgpu::ShaderStages::VERTEX)] });
        let blit_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sb-blit"),
            entries: &[tex_entry(0, wgpu::TextureSampleType::Float { filterable: true }), samp_entry(1, wgpu::SamplerBindingType::Filtering)],
        });
        let main_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("sb-main"), bind_group_layouts: &[Some(&globals_bgl), Some(&mat_bgl)], immediate_size: 0 });
        let shadow_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("sb-shadow"), bind_group_layouts: &[None, Some(&mat_bgl), Some(&su_bgl)], immediate_size: 0 });
        let blit_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("sb-blit"), bind_group_layouts: &[Some(&blit_bgl)], immediate_size: 0 });

        let attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32];
        let vlayout = [Some(wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &attrs })];
        let main = |cull: Option<wgpu::Face>, label: &str| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&main_layout),
                vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_main"), compilation_options: Default::default(), buffers: &vlayout },
                primitive: wgpu::PrimitiveState { cull_mode: cull, front_face: wgpu::FrontFace::Ccw, ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState { count: MSAA, mask: !0, alpha_to_coverage_enabled: true },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format: COLOR, blend: None, write_mask: wgpu::ColorWrites::ALL })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let shadow = |cull: Option<wgpu::Face>, label: &str| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&shadow_layout),
                vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_shadow"), compilation_options: Default::default(), buffers: &vlayout },
                primitive: wgpu::PrimitiveState { cull_mode: cull, ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some("fs_shadow"), compilation_options: Default::default(), targets: &[] }),
                multiview_mask: None,
                cache: None,
            })
        };
        let srgb_target = target_format.is_srgb();
        let blit = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sb-blit"),
            layout: Some(&blit_layout),
            vertex: wgpu::VertexState { module: &blit_module, entry_point: Some("vs_blit"), compilation_options: Default::default(), buffers: &[] },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &blit_module,
                entry_point: Some(if srgb_target { "fs_blit_srgb_target" } else { "fs_blit" }),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: target_format, blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let lin = |addr: wgpu::AddressMode, aniso: u16, label: &str| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some(label),
                address_mode_u: addr,
                address_mode_v: addr,
                address_mode_w: addr,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                anisotropy_clamp: aniso,
                ..Default::default()
            })
        };
        let atlas_samp = lin(wgpu::AddressMode::ClampToEdge, 8, "sb-atlas");
        let nrm_samp = lin(wgpu::AddressMode::Repeat, 8, "sb-normal");
        let lin_samp = lin(wgpu::AddressMode::ClampToEdge, 1, "sb-linear");
        let cmp_samp = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sb-shadow"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        let key_map = depth_target(device, KEY_SHADOW, "sb-key-shadow");
        let gobo_map = depth_target(device, GOBO_SHADOW, "sb-gobo-shadow");
        let (_, cookie) = tex_view(device, queue, &super::textures::gobo(), wgpu::TextureFormat::Rgba8Unorm, "sb-cookie");
        let white_img = MipImage { w: 1, h: 1, levels: vec![vec![255, 255, 255, 255]] };
        let (_, white) = tex_view(device, queue, &white_img, wgpu::TextureFormat::Rgba8Unorm, "sb-white");
        let globals_buf = buffer(device, "sb-globals", std::mem::size_of::<Globals>() as u64, wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST);
        let globals_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sb-globals"),
            layout: &globals_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&key_map) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&gobo_map) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&cmp_samp) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&cookie) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::Sampler(&lin_samp) },
            ],
        });
        let su = |label: &str| {
            let b = buffer(device, label, 64, wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST);
            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some(label), layout: &su_bgl, entries: &[wgpu::BindGroupEntry { binding: 0, resource: b.as_entire_binding() }] });
            (b, bg)
        };
        let mat_bufs = OBJS.iter().map(|o| buffer(device, &format!("sb-mat-{o:?}"), std::mem::size_of::<Material>() as u64, wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST)).collect();
        let nrm = [Weave::Knit, Weave::Weave, Weave::Fleece]
            .into_iter()
            .map(|k| (k, tex_view(device, queue, &super::textures::fabric_normal(k), wgpu::TextureFormat::Rgba8Unorm, "sb-fabric-normal").1))
            .collect();
        Gpu3d {
            main_none: main(None, "sb-main-2sided"),
            main_back: main(Some(wgpu::Face::Back), "sb-main"),
            shadow_none: shadow(None, "sb-shadow-2sided"),
            shadow_back: shadow(Some(wgpu::Face::Back), "sb-shadow"),
            blit,
            blit_bgl,
            mat_bgl,
            globals_buf,
            globals_bg,
            key_map,
            gobo_map,
            su_key: su("sb-su-key"),
            su_gobo: su("sb-su-gobo"),
            mat_bufs,
            mat_bgs: vec![None; OBJS.len()],
            atlas: [white.clone(), white.clone()],
            atlas_size: [0, 0],
            atlas_samp,
            nrm,
            nrm_samp,
            lin_samp,
            white,
            cloth: None,
            scan: None,
            buttons: None,
            hanger: None,
            target: None,
            last: None,
            weave_bound: None,
            dirty: true,
            shadows_dirty: true,
            snapshot: Default::default(),
        }
    }

    /// the atlases (outside, inside) for the pattern editor: stored sRGB-encoded in a plain UNORM
    /// texture, which is exactly how egui samples its own textures (works on WebGL2 too)
    pub fn atlas_views_for_egui(&self) -> [wgpu::TextureView; 2] {
        self.atlas.clone()
    }

    /// new texture atlases (outside, inside), with mip chains (straight alpha, sRGB)
    pub fn set_atlas(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, out: &MipImage, inn: &MipImage) {
        let same = self.atlas_size == [out.w, out.h];
        if same {
            // reuse the textures: just overwrite the pixels
            for (view, img) in self.atlas.iter().zip([out, inn]) {
                write_mips(queue, view.texture(), img);
            }
        } else {
            self.atlas = [tex_view(device, queue, out, wgpu::TextureFormat::Rgba8Unorm, "sb-atlas-out").1, tex_view(device, queue, inn, wgpu::TextureFormat::Rgba8Unorm, "sb-atlas-in").1];
            self.atlas_size = [out.w, out.h];
            self.mat_bgs = vec![None; OBJS.len()];
        }
        self.dirty = true;
        self.shadows_dirty = true;
    }

    fn upload(device: &wgpu::Device, queue: &wgpu::Queue, m: &Mesh, ranges: Vec<std::ops::Range<u32>>, idx: &[u32]) -> Option<GpuMesh> {
        if m.pos.is_empty() || idx.is_empty() {
            return None;
        }
        let verts: Vec<Vertex> = (0..m.pos.len()).map(|i| Vertex { pos: m.pos[i], nrm: m.nrm[i], uv: m.uv[i], ao: m.ao[i] }).collect();
        let vbuf = buffer(device, "sb-vertices", (verts.len() * std::mem::size_of::<Vertex>()) as u64, wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST);
        queue.write_buffer(&vbuf, 0, bytemuck::cast_slice(&verts));
        let ibuf = buffer(device, "sb-indices", (idx.len() * 4) as u64, wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST);
        queue.write_buffer(&ibuf, 0, bytemuck::cast_slice(idx));
        Some(GpuMesh { vbuf, ibuf, ranges })
    }

    /// new scene geometry
    pub fn set_geometry(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, g: &super::worker::GeomOut) {
        self.cloth = None;
        self.scan = None;
        match &g.cloth {
            super::worker::Cloth::Generated(m) => self.cloth = Self::upload(device, queue, m, vec![0..m.idx.len() as u32], &m.idx),
            super::worker::Cloth::Scan { mesh, out, inn } => {
                let mut idx = out.clone();
                idx.extend_from_slice(inn);
                let a = out.len() as u32;
                self.scan = Self::upload(device, queue, mesh, vec![0..a, a..idx.len() as u32], &idx);
            }
        }
        self.buttons = Self::upload(device, queue, &g.buttons, vec![0..g.buttons.idx.len() as u32], &g.buttons.idx);
        self.hanger = g.hanger.as_ref().and_then(|h| Self::upload(device, queue, h, vec![0..h.idx.len() as u32], &h.idx));
        self.dirty = true;
        self.shadows_dirty = true;
    }

    fn ensure_material_bgs(&mut self, device: &wgpu::Device, weave: Weave) {
        if self.weave_bound != Some(weave) {
            self.mat_bgs = vec![None; OBJS.len()];
            self.weave_bound = Some(weave);
        }
        let nrm = self.nrm.iter().find(|(k, _)| *k == weave).map(|(_, v)| v.clone()).unwrap_or_else(|| self.white.clone());
        for (i, o) in OBJS.iter().enumerate() {
            if self.mat_bgs[i].is_some() {
                continue;
            }
            let solid = matches!(o, Obj::Buttons | Obj::Hanger);
            let (a, b) = if solid { (&self.white, &self.white) } else { (&self.atlas[0], &self.atlas[1]) };
            self.mat_bgs[i] = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("sb-material"),
                layout: &self.mat_bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.mat_bufs[i].as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(a) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(b) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&self.atlas_samp) },
                    wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&nrm) },
                    wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::Sampler(&self.nrm_samp) },
                ],
            }));
        }
    }

    fn make_target(&self, device: &wgpu::Device, size: [u32; 2]) -> Target {
        let tex = |samples: u32, format: wgpu::TextureFormat, usage: wgpu::TextureUsages, label: &str| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&wgpu::TextureViewDescriptor::default())
        };
        let msaa = tex(MSAA, COLOR, wgpu::TextureUsages::RENDER_ATTACHMENT, "sb-msaa");
        let resolve = tex(1, COLOR, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC, "sb-resolve");
        let depth = tex(MSAA, DEPTH, wgpu::TextureUsages::RENDER_ATTACHMENT, "sb-depth");
        let blit_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sb-blit"),
            layout: &self.blit_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&resolve) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.lin_samp) }],
        });
        Target { size, msaa, resolve, depth, blit_bg }
    }

    fn mesh_of(&self, o: Obj) -> Option<(&GpuMesh, std::ops::Range<u32>, bool)> {
        // (mesh, index range, two-sided)
        match o {
            Obj::Cloth => self.cloth.as_ref().map(|m| (m, m.ranges[0].clone(), true)),
            Obj::ScanOut => self.scan.as_ref().map(|m| (m, m.ranges[0].clone(), false)),
            Obj::ScanIn => self.scan.as_ref().map(|m| (m, m.ranges[1].clone(), false)),
            Obj::Buttons => self.buttons.as_ref().map(|m| (m, m.ranges[0].clone(), false)),
            Obj::Hanger => self.hanger.as_ref().map(|m| (m, m.ranges[0].clone(), false)),
        }
    }

    /// record the shadow passes and the main pass for `p` into `enc`, rendering into `t`
    fn record(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, enc: &mut wgpu::CommandEncoder, p: &FrameParams, t: &Target, shadows: bool) {
        self.ensure_material_bgs(device, p.weave);
        queue.write_buffer(&self.globals_buf, 0, bytemuck::bytes_of(&p.globals));
        for (i, m) in p.materials.iter().enumerate() {
            queue.write_buffer(&self.mat_bufs[i], 0, bytemuck::bytes_of(m));
        }
        if shadows {
            queue.write_buffer(&self.su_key.0, 0, bytemuck::cast_slice(&p.globals.key_vp));
            queue.write_buffer(&self.su_gobo.0, 0, bytemuck::cast_slice(&p.globals.gobo_vp));
            for (on, view, su) in [(p.key_shadows, &self.key_map, &self.su_key.1), (p.gobo, &self.gobo_map, &self.su_gobo.1)] {
                let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("sb-shadow"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }), stencil_ops: None }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                if !on {
                    continue;
                }
                pass.set_bind_group(2, su, &[]);
                for o in &p.casters {
                    let Some((mesh, range, two)) = self.mesh_of(*o) else { continue };
                    pass.set_pipeline(if two || *o == Obj::ScanOut { &self.shadow_none } else { &self.shadow_back });
                    pass.set_bind_group(1, self.mat_bgs[obj_index(*o)].as_ref(), &[]);
                    pass.set_vertex_buffer(0, mesh.vbuf.slice(..));
                    pass.set_index_buffer(mesh.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(range, 0, 0..1);
                }
            }
        }
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("sb-scene"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &t.msaa,
                depth_slice: None,
                resolve_target: Some(&t.resolve),
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Discard },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &t.depth,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(0, &self.globals_bg, &[]);
        for o in &p.draws {
            let Some((mesh, range, two)) = self.mesh_of(*o) else { continue };
            pass.set_pipeline(if two { &self.main_none } else { &self.main_back });
            pass.set_bind_group(1, self.mat_bgs[obj_index(*o)].as_ref(), &[]);
            pass.set_vertex_buffer(0, mesh.vbuf.slice(..));
            pass.set_index_buffer(mesh.ibuf.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(range, 0, 0..1);
        }
    }

    /// render a high-resolution still and read it back (straight RGBA, sRGB)
    fn snapshot(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, w: u32, h: u32, p: &FrameParams) {
        let t = self.make_target(device, [w, h]);
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("sb-snapshot") });
        self.record(device, queue, &mut enc, p, &t, true);
        let row = (4 * w).div_ceil(256) * 256;
        let buf = buffer(device, "sb-snapshot-read", (row * h) as u64, wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ);
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: t.resolve.texture(), mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) } },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        queue.submit([enc.finish()]);
        let slot = self.snapshot.clone();
        let buf2 = buf.clone();
        buf.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let res = r.map_err(|e| e.to_string()).map(|_| {
                let Ok(data) = buf2.slice(..).get_mapped_range() else { return (0, 0, Vec::new()) };
                let mut out = Vec::with_capacity((w * h * 4) as usize);
                for y in 0..h {
                    let s = (y * row) as usize;
                    out.extend_from_slice(&data[s..s + (w * 4) as usize]);
                }
                drop(data);
                buf2.unmap();
                sb_print::unpremultiply(&mut out);
                (w, h, out)
            });
            *slot.lock().unwrap() = Some(SnapshotState::Done(res));
        });
        #[cfg(not(target_arch = "wasm32"))]
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        // the last frame's image is gone with the old bindings of this target; draw afresh next time
        self.dirty = true;
        self.shadows_dirty = true;
    }
}

/// the per-frame paint callback
pub struct Paint3d {
    pub params: FrameParams,
    /// the lights moved / geometry changed: shadow maps need redrawing
    pub shadows: bool,
}

impl CallbackTrait for Paint3d {
    fn prepare(&self, device: &wgpu::Device, queue: &wgpu::Queue, _sd: &ScreenDescriptor, enc: &mut wgpu::CommandEncoder, res: &mut CallbackResources) -> Vec<wgpu::CommandBuffer> {
        let Some(gpu) = res.get_mut::<Gpu3d>() else { return vec![] };
        // a pending snapshot first (renders off-screen at its own size)
        let req = {
            let mut s = gpu.snapshot.lock().unwrap();
            match s.take() {
                Some(SnapshotState::Requested { w, h, params }) => Some((w, h, params)),
                other => {
                    *s = other;
                    None
                }
            }
        };
        if let Some((w, h, params)) = req {
            gpu.snapshot(device, queue, w, h, &params);
        }
        let size = [self.params.size[0].max(1), self.params.size[1].max(1)];
        if gpu.target.as_ref().map(|t| t.size) != Some(size) {
            gpu.target = Some(gpu.make_target(device, size));
            gpu.dirty = true;
        }
        let changed = gpu.dirty || gpu.last.as_ref() != Some(&self.params);
        if !changed {
            return vec![];
        }
        let shadows = self.shadows || gpu.shadows_dirty;
        let t = gpu.target.take().unwrap();
        gpu.record(device, queue, enc, &self.params, &t, shadows);
        gpu.target = Some(t);
        gpu.last = Some(self.params.clone());
        gpu.dirty = false;
        if shadows {
            gpu.shadows_dirty = false;
        }
        vec![]
    }

    fn paint(&self, _info: egui::PaintCallbackInfo, pass: &mut wgpu::RenderPass<'static>, res: &CallbackResources) {
        let Some(gpu) = res.get::<Gpu3d>() else { return };
        let Some(t) = &gpu.target else { return };
        pass.set_pipeline(&gpu.blit);
        pass.set_bind_group(0, &t.blit_bg, &[]);
        pass.draw(0..3, 0..1);
    }
}

/// put the GPU state into egui's renderer (once, at start-up)
pub fn install(rs: &egui_wgpu::RenderState) {
    let gpu = Gpu3d::new(&rs.device, &rs.queue, rs.target_format);
    rs.renderer.write().callback_resources.insert(gpu);
}
