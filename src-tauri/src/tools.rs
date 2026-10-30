//! Herramientas extra que se instalan con winget (el gestor de paquetes de Windows).

use crate::sysutil;
use serde::{Deserialize, Serialize};

struct Tool {
    id: &'static str,
    name: &'static str,
    cat: &'static str,
    what: &'static str,
}

const TOOLS: &[Tool] = &[
    Tool { id: "Henry++.MemReduct", name: "Mem Reduct", cat: "RAM", what: "Limpia la RAM desde la bandeja del sistema, con limpieza automática por porcentaje o tiempo." },
    Tool { id: "Wagnardsoft.ISLC", name: "Intelligent Standby List Cleaner", cat: "RAM", what: "Vacía la lista standby solo cuando hace falta. Popular para evitar tirones en juegos con poca RAM." },
    Tool { id: "REALiX.HWiNFO", name: "HWiNFO", cat: "Monitoreo", what: "Muestra temperaturas, voltajes y velocidades de todo el hardware. Ideal para revisar si algo se calienta." },
    Tool { id: "Guru3D.Afterburner", name: "MSI Afterburner", cat: "Gaming", what: "Overlay de FPS y temperaturas en juegos, control de ventiladores y overclock de la GPU." },
    Tool { id: "Guru3D.RTSS", name: "RivaTuner Statistics Server", cat: "Gaming", what: "Limita FPS con precisión y muestra el overlay de Afterburner." },
    Tool { id: "CPUID.CPU-Z", name: "CPU-Z", cat: "Monitoreo", what: "Información detallada del procesador, la RAM y la placa madre." },
    Tool { id: "TechPowerUp.GPU-Z", name: "GPU-Z", cat: "Monitoreo", what: "Información y sensores de la tarjeta gráfica." },
    Tool { id: "CrystalDewWorld.CrystalDiskInfo", name: "CrystalDiskInfo", cat: "Discos", what: "Revisa la salud de tus discos (SMART) y avisa si alguno está por fallar." },
    Tool { id: "CrystalDewWorld.CrystalDiskMark", name: "CrystalDiskMark", cat: "Discos", what: "Mide la velocidad real de lectura y escritura de tus discos." },
    Tool { id: "Wagnardsoft.DisplayDriverUninstaller", name: "Display Driver Uninstaller (DDU)", cat: "Drivers", what: "Desinstala por completo el driver de video para hacer una instalación limpia." },
    Tool { id: "TechPowerUp.NVCleanstall", name: "NVCleanstall", cat: "Drivers", what: "Instala el driver de NVIDIA solo con lo necesario, sin telemetría ni extras." },
    Tool { id: "Intel.IntelDriverAndSupportAssistant", name: "Intel Driver & Support Assistant", cat: "Drivers", what: "Busca e instala drivers actualizados para componentes Intel." },
    Tool { id: "Microsoft.Sysinternals.Autoruns", name: "Autoruns", cat: "Sistema", what: "Muestra absolutamente todo lo que arranca con Windows (avanzado)." },
    Tool { id: "Microsoft.Sysinternals.ProcessExplorer", name: "Process Explorer", cat: "Sistema", what: "Administrador de tareas avanzado de Microsoft." },
    Tool { id: "BleachBit.BleachBit", name: "BleachBit", cat: "Limpieza", what: "Limpieza profunda de archivos temporales de cientos de programas." },
    Tool { id: "7zip.7zip", name: "7-Zip", cat: "Utilidades", what: "Compresor de archivos gratuito y ligero (ZIP, RAR, 7z…)." },
];

#[derive(Serialize)]
pub struct ToolInfo {
    pub id: String,
    pub name: String,
    pub cat: String,
    pub what: String,
    pub installed: bool,
}

#[derive(Deserialize)]
struct Export {
    #[serde(rename = "Sources", default)]
    sources: Vec<ExportSource>,
}

#[derive(Deserialize)]
struct ExportSource {
    #[serde(rename = "Packages", default)]
    packages: Vec<ExportPackage>,
}

#[derive(Deserialize)]
struct ExportPackage {
    #[serde(rename = "PackageIdentifier", default)]
    id: String,
}

/// IDs de winget instalados (en minúsculas).
fn installed() -> Result<Vec<String>, String> {
    let file = std::env::temp_dir().join("optimizador_winget.json");
    let path = file.to_string_lossy().into_owned();
    // `winget export` puede terminar con código de error si algún paquete no está en el catálogo,
    // así que solo nos importa que el archivo se haya creado.
    let _ = sysutil::hidden("winget")
        .args(["export", "-o", &path, "--accept-source-agreements", "--disable-interactivity"])
        .output()
        .map_err(|_| "winget no está disponible en este equipo".to_string())?;
    let text = std::fs::read_to_string(&file).map_err(|_| "No se pudo consultar winget".to_string())?;
    let _ = std::fs::remove_file(&file);
    let export: Export = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())?;
    Ok(export.sources.into_iter().flat_map(|s| s.packages).map(|p| p.id.to_lowercase()).collect())
}

pub fn list() -> Result<Vec<ToolInfo>, String> {
    let ids = installed().unwrap_or_default();
    Ok(TOOLS
        .iter()
        .map(|t| ToolInfo {
            id: t.id.into(),
            name: t.name.into(),
            cat: t.cat.into(),
            what: t.what.into(),
            installed: ids.contains(&t.id.to_lowercase()),
        })
        .collect())
}

fn known(id: &str) -> Result<&'static Tool, String> {
    TOOLS.iter().find(|t| t.id == id).ok_or_else(|| "Herramienta desconocida".to_string())
}

fn winget(args: &[&str]) -> Result<(), String> {
    let out = sysutil::hidden("winget")
        .args(args)
        .output()
        .map_err(|_| "winget no está disponible en este equipo".to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        let text = String::from_utf8_lossy(&out.stdout);
        // Nos quedamos con la última línea con texto, que suele ser el motivo del error.
        let last = text
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty() && l.chars().any(|c| c.is_alphabetic()))
            .last()
            .unwrap_or("error desconocido");
        Err(format!("winget: {last}"))
    }
}

pub fn install(id: &str) -> Result<(), String> {
    let t = known(id)?;
    winget(&[
        "install", "--id", t.id, "-e", "--silent",
        "--accept-source-agreements", "--accept-package-agreements", "--disable-interactivity",
    ])
}

pub fn uninstall(id: &str) -> Result<(), String> {
    let t = known(id)?;
    winget(&["uninstall", "--id", t.id, "-e", "--silent", "--disable-interactivity"])
}
