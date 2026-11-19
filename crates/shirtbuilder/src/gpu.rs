//! Graphics card: which GPU draws the app and its 3D preview.
//!
//! At start-up the app picks the adapter itself (desktop): with "Auto" the strongest real
//! graphics card wins — a discrete GPU over an integrated one, a software rasteriser (llvmpipe,
//! WARP) only when nothing else works. A specific card can be chosen in Scene › Graphics card;
//! the choice is saved next to the app's settings and applies on the next start (the device
//! can't be swapped under a running window), so the panel offers a one-click restart.
//! On the web the browser decides (WebGPU, else WebGL); the panel shows what it picked.

use eframe::wgpu;
use serde::{Deserialize, Serialize};

/// a graphics adapter as shown in the UI
#[derive(Clone, Debug, PartialEq)]
pub struct GpuInfo {
    pub name: String,
    pub backend: String,
    pub kind: wgpu::DeviceType,
    pub driver: String,
}

impl GpuInfo {
    pub fn of(info: &wgpu::AdapterInfo) -> GpuInfo {
        let driver = [info.driver.as_str(), info.driver_info.as_str()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" ");
        GpuInfo { name: info.name.clone(), backend: backend_name(info.backend).into(), kind: info.device_type, driver }
    }
    /// drawn by the CPU, not a graphics card
    pub fn software(&self) -> bool {
        self.kind == wgpu::DeviceType::Cpu || self.name.to_lowercase().contains("llvmpipe") || self.name.to_lowercase().contains("swiftshader")
    }
    pub fn kind_label(&self) -> &'static str {
        match self.kind {
            wgpu::DeviceType::DiscreteGpu => "dedicated graphics card",
            wgpu::DeviceType::IntegratedGpu => "integrated graphics",
            wgpu::DeviceType::VirtualGpu => "virtual GPU",
            wgpu::DeviceType::Cpu => "software rendering (CPU)",
            wgpu::DeviceType::Other => "graphics adapter",
        }
    }
    pub fn key(&self) -> GpuChoice {
        GpuChoice::Adapter { name: self.name.clone(), backend: self.backend.clone() }
    }
}

pub fn backend_name(b: wgpu::Backend) -> &'static str {
    match b {
        wgpu::Backend::Vulkan => "Vulkan",
        wgpu::Backend::Metal => "Metal",
        wgpu::Backend::Dx12 => "DirectX 12",
        wgpu::Backend::Gl => "OpenGL",
        wgpu::Backend::BrowserWebGpu => "WebGPU",
        _ => "No backend",
    }
}

/// the user's pick in Scene › Graphics card
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GpuChoice {
    #[default]
    Auto,
    Adapter {
        name: String,
        backend: String,
    },
}

/// higher is better: real GPUs first (discrete > integrated > virtual), then the faster API
pub fn score(info: &wgpu::AdapterInfo) -> i32 {
    let gi = GpuInfo::of(info);
    let kind = match info.device_type {
        wgpu::DeviceType::DiscreteGpu => 400,
        wgpu::DeviceType::IntegratedGpu => 300,
        wgpu::DeviceType::VirtualGpu => 200,
        wgpu::DeviceType::Other => 100,
        wgpu::DeviceType::Cpu => 0,
    };
    let api = match info.backend {
        wgpu::Backend::Vulkan | wgpu::Backend::Metal | wgpu::Backend::Dx12 => 20,
        wgpu::Backend::Gl => 10,
        _ => 0,
    };
    if gi.software() {
        api
    } else {
        kind + api
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Arc;

    fn choice_path() -> Option<PathBuf> {
        eframe::storage_dir("shirtbuilder").map(|d| d.join("graphics-card.json"))
    }

    pub fn load_choice() -> GpuChoice {
        choice_path().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default()
    }

    pub fn save_choice(c: &GpuChoice) -> std::io::Result<()> {
        let p = choice_path().ok_or_else(|| std::io::Error::other("no settings folder"))?;
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(p, serde_json::to_string(c).unwrap_or_default())
    }

    /// wgpu set-up for eframe: our own adapter choice (see the module docs)
    pub fn wgpu_options(choice: GpuChoice) -> eframe::egui_wgpu::WgpuConfiguration {
        use eframe::egui_wgpu::{WgpuSetup, WgpuSetupCreateNew};
        let mut setup = WgpuSetupCreateNew::without_display_handle();
        setup.native_adapter_selector = Some(Arc::new(move |adapters: &[wgpu::Adapter], surface: Option<&wgpu::Surface<'_>>| {
            let usable: Vec<&wgpu::Adapter> = adapters.iter().filter(|a| surface.is_none_or(|s| a.is_surface_supported(s))).collect();
            for a in &usable {
                let i = a.get_info();
                log::info!("graphics adapter: {} · {} · {:?} · score {}", i.name, backend_name(i.backend), i.device_type, score(&i));
            }
            if let GpuChoice::Adapter { name, backend } = &choice {
                if let Some(a) = usable.iter().find(|a| {
                    let i = a.get_info();
                    &i.name == name && backend_name(i.backend) == backend
                }) {
                    return Ok((*a).clone());
                }
                log::warn!("chosen graphics card {name} ({backend}) not found — picking automatically");
            }
            usable.iter().max_by_key(|a| score(&a.get_info())).map(|a| (*a).clone()).ok_or_else(|| "no graphics adapter can draw to this window".to_string())
        }));
        eframe::egui_wgpu::WgpuConfiguration { wgpu_setup: WgpuSetup::CreateNew(setup), ..Default::default() }
    }

    /// start a fresh copy of the app (the AppImage if we run from one) — the caller then closes this one
    pub fn relaunch() -> std::io::Result<()> {
        let exe = std::env::var_os("APPIMAGE").map(PathBuf::from).map_or_else(std::env::current_exe, Ok)?;
        std::process::Command::new(exe).spawn().map(|_| ())
    }

    /// first `a.b` / `a.b.c` version number in `s`
    pub fn version_in(s: &str) -> Option<String> {
        s.split(|c: char| !(c.is_ascii_digit() || c == '.'))
            .map(|t| t.trim_matches('.'))
            .find(|t| t.contains('.') && t.split('.').all(|p| !p.is_empty()))
            .map(str::to_string)
    }

    /// NVIDIA driver loaded in the kernel vs. the driver libraries installed on disk.
    /// After a driver update they differ until the computer restarts, and until then no new
    /// program can use the card (Vulkan and OpenGL both fail) — the most common reason for
    /// landing on the CPU with an NVIDIA card in the machine.
    pub fn nvidia_versions() -> Option<(String, String)> {
        let kernel = std::fs::read_to_string("/proc/driver/nvidia/version").ok()?;
        let kernel = version_in(kernel.lines().next()?.split_once("Kernel Module")?.1)?;
        let user = ["/usr/lib", "/usr/lib64", "/usr/lib/x86_64-linux-gnu"].iter().find_map(|dir| {
            std::fs::read_dir(dir).ok()?.flatten().find_map(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                n.strip_prefix("libnvidia-glcore.so.").filter(|v| v.contains('.')).map(str::to_string)
            })
        })?;
        Some((kernel, user))
    }

    /// why no graphics card could be used, when we can tell
    pub fn driver_problem() -> Option<String> {
        let (kernel, user) = nvidia_versions()?;
        (kernel != user).then(|| {
            format!(
                "The NVIDIA driver was updated to {user}, but the computer is still running the old {kernel} driver, so no program started since the update can use the graphics card. Restart the computer and ShirtBuilder will use it automatically."
            )
        })
    }
}
#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

/// what the running app knows about graphics cards
pub struct GpuState {
    /// the adapter drawing this window
    pub active: Option<GpuInfo>,
    /// every adapter found (desktop)
    pub all: Vec<GpuInfo>,
    /// saved choice (applies at the next start)
    pub choice: GpuChoice,
    /// the choice the running app started with
    pub started_with: GpuChoice,
    /// why the app ended up on the CPU, when known (e.g. a driver update waiting for a restart)
    pub problem: Option<String>,
}

impl GpuState {
    pub fn new(rs: Option<&eframe::egui_wgpu::RenderState>) -> GpuState {
        let active = rs.map(|r| GpuInfo::of(&r.adapter.get_info()));
        #[cfg(not(target_arch = "wasm32"))]
        let (all, choice) = {
            let mut all: Vec<GpuInfo> = Vec::new();
            if let Some(r) = rs {
                let mut infos: Vec<_> = r.available_adapters.iter().map(|a| a.get_info()).collect();
                infos.sort_by_key(|i| -score(i));
                for i in infos {
                    let g = GpuInfo::of(&i);
                    if !all.contains(&g) {
                        all.push(g);
                    }
                }
            }
            (all, load_choice())
        };
        #[cfg(target_arch = "wasm32")]
        let (all, choice) = (active.clone().into_iter().collect(), GpuChoice::Auto);
        #[cfg(not(target_arch = "wasm32"))]
        let problem = active.as_ref().filter(|a| a.software()).and_then(|_| driver_problem());
        #[cfg(target_arch = "wasm32")]
        let problem = None;
        if let Some(p) = &problem {
            log::warn!("{p}");
        }
        GpuState { active, all, started_with: choice.clone(), choice, problem }
    }

    /// the 3D is drawn by the CPU
    pub fn on_cpu(&self) -> bool {
        self.active.as_ref().is_some_and(GpuInfo::software)
    }

    /// the best real graphics card found, if the app is not already on it
    pub fn better_than_active(&self) -> Option<&GpuInfo> {
        let a = self.active.as_ref()?;
        self.all.iter().find(|g| !g.software() && a.software())
    }
}
