//! Drivers: estado del driver de video y dispositivos con problemas.

use crate::hw::Hw;
use crate::sysutil;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct GpuDriver {
    pub name: String,
    pub vendor: String,
    pub version: String,
    pub date: String,
    pub age_days: Option<i64>,
    pub vram: u64,
    pub url: String,
    pub tool: String,
}

#[derive(Serialize, Deserialize)]
pub struct ProblemDevice {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub class: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub code: i64,
    #[serde(default)]
    pub explain: String,
}

#[derive(Serialize)]
pub struct DriversInfo {
    pub gpus: Vec<GpuDriver>,
    pub problems: Vec<ProblemDevice>,
}

/// Convierte "M-D-YYYY" (formato del registro) en días de antigüedad.
fn age(date: &str) -> Option<i64> {
    let parts: Vec<i64> = date.split(['-', '/']).filter_map(|p| p.trim().parse().ok()).collect();
    if parts.len() != 3 {
        return None;
    }
    let (m, d, y) = if parts[0] > 1900 { (parts[1], parts[2], parts[0]) } else { (parts[0], parts[1], parts[2]) };
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some(sysutil::today_days() - sysutil::days_from_civil(y, m, d))
}

fn vendor_links(name: &str) -> (&'static str, String, String) {
    let n = name.to_lowercase();
    if n.contains("nvidia") {
        ("NVIDIA", "https://www.nvidia.com/es-la/software/nvidia-app/".into(), "NVIDIA App".into())
    } else if n.contains("amd") || n.contains("radeon") {
        ("AMD", "https://www.amd.com/es/support/download/drivers.html".into(), "AMD Software: Adrenalin".into())
    } else if n.contains("intel") {
        ("Intel", "https://www.intel.com/content/www/us/en/support/detect.html".into(), "Intel Driver & Support Assistant".into())
    } else {
        ("", String::new(), String::new())
    }
}

fn explain(code: i64) -> &'static str {
    match code {
        1 => "Configuración incorrecta del dispositivo",
        3 => "El driver está dañado o el sistema tiene poca memoria",
        10 => "El dispositivo no puede iniciar (driver incorrecto o falla de hardware)",
        12 => "Conflicto de recursos con otro dispositivo",
        14 => "Necesita reiniciar la PC",
        18 => "Hay que reinstalar el driver",
        19 => "Información de configuración dañada en el registro",
        22 => "El dispositivo está deshabilitado",
        24 => "El dispositivo no está presente o no funciona bien",
        28 => "No tiene driver instalado",
        31 => "Windows no pudo cargar el driver",
        39 => "El driver está dañado o falta",
        43 => "Windows detuvo el dispositivo porque reportó un problema",
        45 => "El dispositivo no está conectado",
        52 => "El driver no tiene una firma digital válida",
        _ => "Revisa el dispositivo en el Administrador de dispositivos",
    }
}

const PROBLEMS_SCRIPT: &str = r#"
$list = @(Get-PnpDevice -PresentOnly -ErrorAction SilentlyContinue |
  Where-Object { $_.Status -ne 'OK' -and $_.ConfigManagerErrorCode -ne 0 } |
  ForEach-Object {
    [pscustomobject]@{
      name = [string]$_.FriendlyName; class = [string]$_.Class
      status = [string]$_.Status; code = [int]$_.ConfigManagerErrorCode
    }
  })
ConvertTo-Json -InputObject $list -Compress -Depth 3
"#;

pub fn info(hw: &Hw) -> DriversInfo {
    let gpus = hw
        .gpus
        .iter()
        .map(|g| {
            let (vendor, url, tool) = vendor_links(&g.name);
            GpuDriver {
                name: g.name.clone(),
                vendor: vendor.into(),
                version: g.driver_version.clone(),
                date: g.driver_date.clone(),
                age_days: age(&g.driver_date),
                vram: g.vram,
                url,
                tool,
            }
        })
        .collect();

    let problems = sysutil::powershell(PROBLEMS_SCRIPT)
        .ok()
        .and_then(|s| {
            let s = s.trim();
            if s.is_empty() {
                return Some(Vec::new());
            }
            serde_json::from_str::<Vec<ProblemDevice>>(s).ok()
        })
        .unwrap_or_default()
        .into_iter()
        .map(|mut d| {
            if d.name.is_empty() {
                d.name = "Dispositivo desconocido".into();
            }
            d.explain = explain(d.code).into();
            d
        })
        .collect();

    DriversInfo { gpus, problems }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn antiguedad() {
        assert!(age("1-15-2020").unwrap() > 1000);
        assert!(age("basura").is_none());
        assert!(age("2024-05-01").is_some());
    }
}
