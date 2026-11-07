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
    pub id: String,
    #[serde(default)]
    pub explain: String,
    #[serde(default)]
    pub fix: String,
    #[serde(default)]
    pub action: String,
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

/// Guía para cada código de error del Administrador de dispositivos:
/// (qué significa, cómo arreglarlo, acción que ofrece la app).
fn explain(code: i64) -> (&'static str, &'static str, &'static str) {
    match code {
        1 => ("Configuración incorrecta del dispositivo", "Actualiza el driver desde Windows Update o la página del fabricante.", "update"),
        3 => ("El driver está dañado o el sistema tiene poca memoria", "Cierra programas y reinicia. Si sigue, desinstala el dispositivo en el Administrador de dispositivos y vuelve a buscar cambios de hardware.", "scan"),
        10 => ("El dispositivo no puede iniciar", "Suele ser un driver incorrecto. Instala el driver oficial del fabricante; si es USB, prueba otro puerto.", "update"),
        12 => ("Conflicto de recursos con otro dispositivo", "Actualiza la BIOS y los drivers de chipset. Desconecta dispositivos que no uses.", "devmgmt"),
        14 => ("Necesita reiniciar la PC", "Reinicia Windows para que el dispositivo empiece a funcionar.", "restart"),
        18 => ("Hay que reinstalar el driver", "Busca cambios de hardware para que Windows lo reinstale, o instala el driver del fabricante.", "scan"),
        19 => ("Información de configuración dañada en el registro", "Desinstala el dispositivo en el Administrador de dispositivos y vuelve a buscar cambios de hardware.", "devmgmt"),
        21 => ("Windows está quitando el dispositivo", "Espera unos segundos y reinicia la PC.", "restart"),
        22 => ("El dispositivo está deshabilitado", "Alguien lo desactivó a mano. Puedes volver a habilitarlo.", "enable"),
        24 => ("El dispositivo no está presente o no funciona bien", "Revisa que esté bien conectado y busca cambios de hardware.", "scan"),
        28 => ("No tiene driver instalado", "Búscalo en «Drivers en Windows Update» o en la página de soporte de tu PC o placa madre.", "update"),
        29 => ("Desactivado en la BIOS", "Actívalo desde la configuración de la BIOS/UEFI de tu placa.", "none"),
        31 => ("Windows no pudo cargar el driver", "Instala la versión más reciente del driver del fabricante.", "update"),
        32 => ("El servicio del driver está deshabilitado", "Reinstala el driver del fabricante para restaurar su servicio.", "update"),
        37 | 39 => ("El driver está dañado o falta", "Desinstala el dispositivo y vuelve a instalar el driver del fabricante.", "update"),
        43 => ("Windows detuvo el dispositivo porque reportó un problema", "En tarjetas gráficas: reinstala el driver con DDU (en Herramientas). En USB: prueba otro puerto o cable.", "update"),
        45 => ("El dispositivo no está conectado", "Es un dispositivo que usaste antes. Si no lo vas a usar, puedes ignorarlo.", "none"),
        48 => ("Driver bloqueado por problemas conocidos", "Instala una versión más nueva del driver desde el fabricante.", "update"),
        52 => ("El driver no tiene una firma digital válida", "Descarga el driver oficial del fabricante; evita drivers de páginas desconocidas.", "update"),
        _ => ("Error del dispositivo", "Revisa el dispositivo en el Administrador de dispositivos.", "devmgmt"),
    }
}

const PROBLEMS_SCRIPT: &str = r#"
$list = @(Get-PnpDevice -PresentOnly -ErrorAction SilentlyContinue |
  Where-Object { $_.Status -ne 'OK' -and $_.ConfigManagerErrorCode -ne 0 } |
  ForEach-Object {
    [pscustomobject]@{
      name = [string]$_.FriendlyName; class = [string]$_.Class
      status = [string]$_.Status; code = [int]$_.ConfigManagerErrorCode
      id = [string]$_.InstanceId
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
            let (what, fix, action) = explain(d.code);
            d.explain = what.into();
            d.fix = fix.into();
            d.action = action.into();
            d
        })
        .collect();

    DriversInfo { gpus, problems }
}

// ---------- Drivers obsoletos en el DriverStore ----------
//
// Cada vez que instalas un driver nuevo, Windows guarda una copia del paquete en
// C:\Windows\System32\DriverStore\FileRepository y NUNCA borra las versiones viejas.
// Con NVIDIA cada versión ocupa ~1-2 GB, así que se acumulan decenas de GB.

#[derive(Serialize, Deserialize, Clone)]
pub struct DriverPkg {
    #[serde(default)]
    pub inf: String,
    #[serde(default)]
    pub original: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub class: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub in_use: bool,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub newest: String,
}

#[derive(Serialize)]
pub struct StoreReport {
    pub obsolete: Vec<DriverPkg>,
    pub total_packages: usize,
    pub store_size: u64,
}

const STORE_SCRIPT: &str = r#"
$used = @{}
Get-CimInstance Win32_PnPSignedDriver -ErrorAction SilentlyContinue | ForEach-Object {
  if ($_.InfName) { $used[$_.InfName.ToLower()] = $true }
}
$list = @(Get-WindowsDriver -Online | ForEach-Object {
  [pscustomobject]@{
    inf = [string]$_.Driver; original = [string]$_.OriginalFileName
    provider = [string]$_.ProviderName; class = [string]$_.ClassName
    version = [string]$_.Version
    date = $(if ($_.Date) { $_.Date.ToString('yyyy-MM-dd') } else { '' })
    in_use = [bool]$used[([string]$_.Driver).ToLower()]
  }
})
ConvertTo-Json -InputObject $list -Compress -Depth 3
"#;

fn version_key(v: &str) -> Vec<u64> {
    v.split('.').map(|p| p.trim().parse().unwrap_or(0)).collect()
}

fn inf_name(original: &str) -> String {
    original.rsplit(['\\', '/']).next().map(|f| f.to_lowercase()).unwrap_or_default()
}

/// Un paquete es obsoleto si hay otro del mismo driver (mismo .inf y fabricante) con versión
/// más nueva y él no lo usa ningún dispositivo conectado.
pub fn find_obsolete(pkgs: &[DriverPkg]) -> Vec<DriverPkg> {
    use std::collections::HashMap;
    let mut groups: HashMap<(String, String), Vec<&DriverPkg>> = HashMap::new();
    for p in pkgs {
        let name = inf_name(&p.original);
        if name.is_empty() {
            continue;
        }
        groups.entry((name, p.provider.to_lowercase())).or_default().push(p);
    }
    let mut out = Vec::new();
    for list in groups.values() {
        if list.len() < 2 {
            continue;
        }
        let newest = list.iter().map(|p| version_key(&p.version)).max().unwrap_or_default();
        let newest_str = list
            .iter()
            .find(|p| version_key(&p.version) == newest)
            .map(|p| p.version.clone())
            .unwrap_or_default();
        for p in list {
            if !p.in_use && version_key(&p.version) < newest {
                let mut q = (*p).clone();
                q.newest = newest_str.clone();
                out.push(q);
            }
        }
    }
    out.sort_by(|a, b| a.provider.cmp(&b.provider).then(version_key(&a.version).cmp(&version_key(&b.version))));
    out
}

fn pkg_dir(original: &str) -> Option<std::path::PathBuf> {
    let p = std::path::Path::new(original).parent()?.to_path_buf();
    // Solo carpetas dentro del DriverStore
    p.to_string_lossy().to_lowercase().contains(r"driverstore\filerepository").then_some(p)
}

pub fn scan_store() -> Result<StoreReport, String> {
    if !sysutil::is_admin() {
        return Err("Se necesitan permisos de administrador para revisar los drivers guardados.".into());
    }
    let raw = sysutil::powershell(STORE_SCRIPT)?;
    let pkgs: Vec<DriverPkg> =
        serde_json::from_str(raw.trim()).map_err(|e| format!("No se pudo leer la lista de drivers: {e}"))?;
    let mut obsolete = find_obsolete(&pkgs);
    for p in &mut obsolete {
        p.size = pkg_dir(&p.original).map(|d| sysutil::dir_size(&d).0).unwrap_or(0);
    }
    let store = sysutil::env_path("SystemRoot")
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Windows"))
        .join(r"System32\DriverStore\FileRepository");
    Ok(StoreReport { obsolete, total_packages: pkgs.len(), store_size: sysutil::dir_size(&store).0 })
}

#[derive(Serialize)]
pub struct DeleteReport {
    pub deleted: Vec<String>,
    pub failed: Vec<String>,
    pub freed: u64,
}

fn valid_oem(inf: &str) -> bool {
    let l = inf.to_lowercase();
    l.len() > 7 && l.starts_with("oem") && l.ends_with(".inf") && l[3..l.len() - 4].chars().all(|c| c.is_ascii_digit())
}

/// Borra paquetes con `pnputil /delete-driver` (sin /force: Windows se niega si alguno está en uso).
pub fn delete_packages(items: &[DriverPkg]) -> DeleteReport {
    let mut rep = DeleteReport { deleted: vec![], failed: vec![], freed: 0 };
    for p in items {
        if !valid_oem(&p.inf) {
            rep.failed.push(format!("{}: nombre no válido", p.inf));
            continue;
        }
        let size = pkg_dir(&p.original).map(|d| sysutil::dir_size(&d).0).unwrap_or(0);
        match sysutil::run("pnputil", &["/delete-driver", &p.inf]) {
            Ok(_) => {
                rep.freed += size;
                rep.deleted.push(format!("{} {} ({})", p.provider, p.version, p.inf));
            }
            Err(e) => rep.failed.push(format!("{}: {e}", p.inf)),
        }
    }
    rep
}

/// Pide a Windows que vuelva a detectar el hardware (como «Buscar cambios de hardware»).
pub fn scan_devices() -> Result<(), String> {
    sysutil::run("pnputil", &["/scan-devices"]).map(|_| ())
}

pub fn enable_device(id: &str) -> Result<(), String> {
    if id.is_empty() || id.contains('"') {
        return Err("Dispositivo no válido".into());
    }
    sysutil::run("pnputil", &["/enable-device", id]).map(|_| ())
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

    fn pkg(inf: &str, ver: &str, used: bool) -> DriverPkg {
        DriverPkg {
            inf: inf.into(),
            original: format!(r"C:\Windows\System32\DriverStore\FileRepository\nv_dispi.inf_{inf}\nv_dispi.inf"),
            provider: "NVIDIA".into(),
            class: "Display".into(),
            version: ver.into(),
            date: String::new(),
            in_use: used,
            size: 0,
            newest: String::new(),
        }
    }

    #[test]
    fn obsoletos() {
        let list = vec![
            pkg("oem1.inf", "32.0.15.6094", false),
            pkg("oem2.inf", "32.0.15.6614", true),
            pkg("oem3.inf", "31.0.15.5222", false),
        ];
        let o = find_obsolete(&list);
        assert_eq!(o.len(), 2);
        assert!(o.iter().all(|p| p.inf != "oem2.inf"));
        assert!(o.iter().all(|p| p.newest == "32.0.15.6614"));
        // un paquete único nunca es obsoleto
        assert!(find_obsolete(&list[..1]).is_empty());
        assert!(valid_oem("oem12.inf") && !valid_oem("nv_dispi.inf") && !valid_oem("oem.inf"));
    }
}
