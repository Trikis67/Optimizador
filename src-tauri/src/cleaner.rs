//! Limpieza de archivos temporales y cachés del sistema.

use crate::sysutil::{self, env_path};
use serde::Serialize;
use std::path::PathBuf;

enum Target {
    /// Borrar el contenido de la carpeta
    Dir(PathBuf),
    /// Borrar archivos de una carpeta que empiecen con un prefijo
    Files(PathBuf, &'static str),
    /// Un archivo suelto
    File(PathBuf),
    RecycleBin,
}

struct Category {
    id: &'static str,
    title: &'static str,
    what: &'static str,
    note: &'static str,
    admin: bool,
    default_on: bool,
    targets: Vec<Target>,
}

fn local(rel: &str) -> PathBuf {
    env_path("LOCALAPPDATA").unwrap_or_default().join(rel)
}
fn windir(rel: &str) -> PathBuf {
    env_path("SystemRoot").unwrap_or_else(|| PathBuf::from(r"C:\Windows")).join(rel)
}
fn progdata(rel: &str) -> PathBuf {
    env_path("ProgramData").unwrap_or_else(|| PathBuf::from(r"C:\ProgramData")).join(rel)
}

fn categories() -> Vec<Category> {
    vec![
        Category {
            id: "temp_user",
            title: "Archivos temporales del usuario",
            what: "Archivos que los programas crean mientras trabajan y olvidan borrar (carpeta %TEMP%).",
            note: "Seguro. Los archivos que estén en uso se saltan automáticamente.",
            admin: false,
            default_on: true,
            targets: vec![Target::Dir(env_path("TEMP").unwrap_or_else(|| local("Temp")))],
        },
        Category {
            id: "temp_windows",
            title: "Archivos temporales de Windows",
            what: "Temporales del sistema y de instaladores (C:\\Windows\\Temp).",
            note: "Seguro. Necesita permisos de administrador.",
            admin: true,
            default_on: true,
            targets: vec![Target::Dir(windir("Temp"))],
        },
        Category {
            id: "recycle",
            title: "Papelera de reciclaje",
            what: "Archivos que ya borraste y siguen ocupando espacio en la papelera.",
            note: "Después de vaciarla no se pueden recuperar.",
            admin: false,
            default_on: false,
            targets: vec![Target::RecycleBin],
        },
        Category {
            id: "update_cache",
            title: "Descargas de Windows Update",
            what: "Instaladores de actualizaciones que ya se aplicaron (SoftwareDistribution\\Download).",
            note: "Seguro: si Windows necesita algo, lo vuelve a descargar. Necesita administrador.",
            admin: true,
            default_on: true,
            targets: vec![Target::Dir(windir(r"SoftwareDistribution\Download"))],
        },
        Category {
            id: "shader_cache",
            title: "Caché de shaders de la GPU",
            what: "Shaders que DirectX, NVIDIA y AMD guardan para no tener que compilarlos de nuevo.",
            note: "Útil si un juego da errores gráficos o tirones raros después de actualizar el driver. Los juegos \
                   pueden ir con tirones la primera vez mientras se vuelven a generar.",
            admin: false,
            default_on: false,
            targets: vec![
                Target::Dir(local("D3DSCache")),
                Target::Dir(local(r"NVIDIA\DXCache")),
                Target::Dir(local(r"NVIDIA\GLCache")),
                Target::Dir(local(r"AMD\DxCache")),
                Target::Dir(local(r"AMD\DxcCache")),
                Target::Dir(local(r"AMD\VkCache")),
                Target::Dir(local(r"AMD\GLCache")),
            ],
        },
        Category {
            id: "thumbnails",
            title: "Caché de miniaturas",
            what: "Vistas previas de imágenes y videos que guarda el Explorador.",
            note: "Seguro. Se regeneran al abrir las carpetas. Si el Explorador está abierto, algunas se saltan.",
            admin: false,
            default_on: true,
            targets: vec![
                Target::Files(local(r"Microsoft\Windows\Explorer"), "thumbcache_"),
                Target::Files(local(r"Microsoft\Windows\Explorer"), "iconcache_"),
            ],
        },
        Category {
            id: "crash_dumps",
            title: "Volcados de errores",
            what: "Archivos que se generan cuando un programa o Windows se cierran con error (pantallazos azules).",
            note: "Solo sirven para diagnosticar fallos. Si estás investigando un pantallazo azul, no los borres aún.",
            admin: false,
            default_on: true,
            targets: vec![
                Target::Dir(local("CrashDumps")),
                Target::Dir(windir("Minidump")),
                Target::File(windir("MEMORY.DMP")),
            ],
        },
        Category {
            id: "error_reports",
            title: "Informes de errores de Windows",
            what: "Reportes de fallos de programas guardados en ProgramData\\Microsoft\\Windows\\WER.",
            note: "Seguro. Necesita administrador para los del sistema.",
            admin: true,
            default_on: true,
            targets: vec![
                Target::Dir(progdata(r"Microsoft\Windows\WER\ReportArchive")),
                Target::Dir(progdata(r"Microsoft\Windows\WER\ReportQueue")),
                Target::Dir(local(r"Microsoft\Windows\WER\ReportArchive")),
            ],
        },
        Category {
            id: "delivery_cache",
            title: "Caché de Optimización de distribución",
            what: "Partes de actualizaciones guardadas para compartirlas con otras PCs.",
            note: "Seguro. Necesita administrador.",
            admin: true,
            default_on: true,
            targets: vec![Target::Dir(windir(r"ServiceProfiles\NetworkService\AppData\Local\Microsoft\Windows\DeliveryOptimization\Cache"))],
        },
        Category {
            id: "driver_leftovers",
            title: "Restos de instaladores de drivers",
            what: "Carpetas que dejan los instaladores de NVIDIA y AMD al descomprimirse (C:\\NVIDIA, C:\\AMD) y las \
                   descargas de drivers de la app de NVIDIA.",
            note: "Seguro: son copias de instaladores ya usados, no el driver instalado. Si quieres reinstalar, \
                   descargas el instalador de nuevo. Necesita administrador.",
            admin: true,
            default_on: true,
            targets: vec![
                Target::Dir(PathBuf::from(r"C:\NVIDIA")),
                Target::Dir(PathBuf::from(r"C:\AMD")),
                Target::Dir(progdata(r"NVIDIA Corporation\Downloader")),
            ],
        },
        Category {
            id: "windows_logs",
            title: "Registros (logs) de Windows",
            what: "Archivos de registro de actualizaciones y reparaciones (Windows\\Logs\\CBS y DISM).",
            note: "Seguro. Solo sirven para diagnosticar problemas de actualización. Los que estén en uso se saltan.",
            admin: true,
            default_on: false,
            targets: vec![Target::Dir(windir(r"Logs\CBS")), Target::Dir(windir(r"Logs\DISM"))],
        },
        Category {
            id: "inet_cache",
            title: "Caché de Internet de Windows",
            what: "Archivos que guardan apps antiguas y componentes de Windows que usan el motor de Internet Explorer.",
            note: "Seguro. No afecta a Chrome, Edge ni Brave (esos están en Navegadores).",
            admin: false,
            default_on: false,
            targets: vec![Target::Dir(local(r"Microsoft\Windows\INetCache"))],
        },
    ]
}

// --- Papelera (shell32) ---

#[repr(C)]
struct ShQueryRbInfo {
    size: u32,
    bytes: i64,
    items: i64,
}

#[link(name = "shell32")]
extern "system" {
    fn SHQueryRecycleBinW(root: *const u16, info: *mut ShQueryRbInfo) -> i32;
    fn SHEmptyRecycleBinW(hwnd: isize, root: *const u16, flags: u32) -> i32;
}

fn recycle_size() -> (u64, u64) {
    let mut info = ShQueryRbInfo { size: std::mem::size_of::<ShQueryRbInfo>() as u32, bytes: 0, items: 0 };
    let hr = unsafe { SHQueryRecycleBinW(std::ptr::null(), &mut info) };
    if hr == 0 {
        (info.bytes.max(0) as u64, info.items.max(0) as u64)
    } else {
        (0, 0)
    }
}

fn empty_recycle() {
    // SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND
    unsafe {
        SHEmptyRecycleBinW(0, std::ptr::null(), 0x1 | 0x2 | 0x4);
    }
}

// --- Medir y limpiar ---

/// Protección: solo rutas absolutas y nunca la raíz de una unidad.
fn safe(t: &Target) -> bool {
    match t {
        Target::Dir(p) | Target::File(p) | Target::Files(p, _) => p.is_absolute() && p.components().count() > 2,
        Target::RecycleBin => true,
    }
}

fn measure(t: &Target) -> (u64, u64) {
    if !safe(t) {
        return (0, 0);
    }
    match t {
        Target::Dir(p) => sysutil::dir_size(p),
        Target::File(p) => std::fs::metadata(p).map(|m| (m.len(), 1)).unwrap_or((0, 0)),
        Target::Files(dir, prefix) => {
            let mut s = (0, 0);
            if let Ok(rd) = std::fs::read_dir(dir) {
                for e in rd.flatten() {
                    if e.file_name().to_string_lossy().to_lowercase().starts_with(prefix) {
                        s.0 += e.metadata().map(|m| m.len()).unwrap_or(0);
                        s.1 += 1;
                    }
                }
            }
            s
        }
        Target::RecycleBin => recycle_size(),
    }
}

fn wipe(t: &Target) -> (u64, u64) {
    if !safe(t) {
        return (0, 0);
    }
    match t {
        Target::Dir(p) => sysutil::clear_dir(p),
        Target::File(p) => {
            let size = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
            if size == 0 && !p.exists() {
                (0, 0)
            } else if std::fs::remove_file(p).is_ok() {
                (size, 0)
            } else {
                (0, 1)
            }
        }
        Target::Files(dir, prefix) => {
            let mut r = (0, 0);
            if let Ok(rd) = std::fs::read_dir(dir) {
                for e in rd.flatten() {
                    if e.file_name().to_string_lossy().to_lowercase().starts_with(prefix) {
                        let size = e.metadata().map(|m| m.len()).unwrap_or(0);
                        if std::fs::remove_file(e.path()).is_ok() {
                            r.0 += size;
                        } else {
                            r.1 += 1;
                        }
                    }
                }
            }
            r
        }
        Target::RecycleBin => {
            let (size, _) = recycle_size();
            empty_recycle();
            let (after, _) = recycle_size();
            (size.saturating_sub(after), 0)
        }
    }
}

#[derive(Serialize)]
pub struct CleanItem {
    pub id: String,
    pub title: String,
    pub what: String,
    pub note: String,
    pub admin: bool,
    pub default_on: bool,
    pub size: u64,
    pub files: u64,
}

pub fn scan() -> Vec<CleanItem> {
    categories()
        .into_iter()
        .map(|c| {
            let (size, files) = c.targets.iter().map(measure).fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
            CleanItem {
                id: c.id.into(),
                title: c.title.into(),
                what: c.what.into(),
                note: c.note.into(),
                admin: c.admin,
                default_on: c.default_on,
                size,
                files,
            }
        })
        .collect()
}

#[derive(Serialize)]
pub struct CleanReport {
    pub freed: u64,
    pub skipped: u64,
}

pub fn clean(ids: &[String]) -> CleanReport {
    let mut rep = CleanReport { freed: 0, skipped: 0 };
    for c in categories().into_iter().filter(|c| ids.iter().any(|i| i == c.id)) {
        for t in &c.targets {
            let (f, s) = wipe(t);
            rep.freed += f;
            rep.skipped += s;
        }
    }
    rep
}
