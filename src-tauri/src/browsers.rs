//! Navegadores: detecta los instalados, mide y limpia su caché (no toca historial, contraseñas ni cookies).

use crate::sysutil;
use serde::Serialize;
use std::path::{Path, PathBuf};

struct Browser {
    id: &'static str,
    name: &'static str,
    exe: &'static str,
    /// Variable de entorno + ruta relativa donde guarda su caché
    base: (&'static str, &'static str),
}

const BROWSERS: &[Browser] = &[
    Browser { id: "chrome", name: "Google Chrome", exe: "chrome.exe", base: ("LOCALAPPDATA", r"Google\Chrome\User Data") },
    Browser { id: "edge", name: "Microsoft Edge", exe: "msedge.exe", base: ("LOCALAPPDATA", r"Microsoft\Edge\User Data") },
    Browser { id: "brave", name: "Brave", exe: "brave.exe", base: ("LOCALAPPDATA", r"BraveSoftware\Brave-Browser\User Data") },
    Browser { id: "opera", name: "Opera", exe: "opera.exe", base: ("LOCALAPPDATA", r"Opera Software\Opera Stable") },
    Browser { id: "operagx", name: "Opera GX", exe: "opera.exe", base: ("LOCALAPPDATA", r"Opera Software\Opera GX Stable") },
    Browser { id: "vivaldi", name: "Vivaldi", exe: "vivaldi.exe", base: ("LOCALAPPDATA", r"Vivaldi\User Data") },
    Browser { id: "firefox", name: "Mozilla Firefox", exe: "firefox.exe", base: ("LOCALAPPDATA", r"Mozilla\Firefox\Profiles") },
];

/// Carpetas de caché conocidas (Chromium y Firefox). Solo archivos temporales que el navegador regenera.
const CACHE_NAMES: &[&str] = &[
    "Cache",
    "Code Cache",
    "GPUCache",
    "GrShaderCache",
    "ShaderCache",
    "GraphiteDawnCache",
    "DawnCache",
    "DawnGraphiteCache",
    "DawnWebGPUCache",
    "cache2",
];

fn base(b: &Browser) -> Option<PathBuf> {
    sysutil::env_path(b.base.0).map(|p| p.join(b.base.1))
}

pub fn installed_ids() -> Vec<String> {
    BROWSERS
        .iter()
        .filter(|b| base(b).map_or(false, |p| p.is_dir()))
        .map(|b| b.id.to_string())
        .collect()
}

/// Busca las carpetas de caché hasta 3 niveles de profundidad (perfiles incluidos).
fn cache_dirs(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![(root.to_path_buf(), 0u8)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            if !ft.is_dir() || ft.is_symlink() {
                continue;
            }
            let name = e.file_name().to_string_lossy().into_owned();
            if CACHE_NAMES.iter().any(|c| c.eq_ignore_ascii_case(&name)) {
                out.push(e.path());
            } else if depth < 3 && !name.eq_ignore_ascii_case("Extensions") {
                stack.push((e.path(), depth + 1));
            }
        }
    }
    out
}

#[derive(Serialize)]
pub struct BrowserInfo {
    pub id: String,
    pub name: String,
    pub running: bool,
    pub cache: u64,
    pub files: u64,
}

pub fn list() -> Vec<BrowserInfo> {
    let running = crate::perf::running_names();
    BROWSERS
        .iter()
        .filter_map(|b| {
            let root = base(b).filter(|p| p.is_dir())?;
            let (mut cache, mut files) = (0, 0);
            for d in cache_dirs(&root) {
                let (s, f) = sysutil::dir_size(&d);
                cache += s;
                files += f;
            }
            Some(BrowserInfo {
                id: b.id.into(),
                name: b.name.into(),
                running: running.contains(b.exe),
                cache,
                files,
            })
        })
        .collect()
}

pub fn clean(id: &str) -> Result<u64, String> {
    let b = BROWSERS.iter().find(|b| b.id == id).ok_or("Navegador desconocido")?;
    if crate::perf::running_names().contains(b.exe) {
        return Err(format!("Cierra {} antes de limpiar su caché (revisa que no quede abierto en la bandeja).", b.name));
    }
    let root = base(b).ok_or("No se encontró la carpeta del navegador")?;
    let mut freed = 0;
    for d in cache_dirs(&root) {
        freed += sysutil::clear_dir(&d).0;
    }
    Ok(freed)
}
