//! Salud del sistema: estado de los discos (SMART), batería, apagones y pantallazos azules,
//! reinicios pendientes, espacio libre y archivo de paginación. También lanza las reparaciones
//! de Windows (DISM, SFC, CHKDSK) en una ventana visible para que el usuario vea el progreso.

use crate::sysutil;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DiskHealth {
    pub name: String,
    pub media: String,
    pub bus: String,
    pub health: String,
    pub size: u64,
    pub temperature: Option<i64>,
    pub wear: Option<i64>,
    pub hours: Option<i64>,
    pub read_errors: Option<i64>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Battery {
    pub charge: Option<i64>,
    pub design: Option<i64>,
    pub full: Option<i64>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CrashInfo {
    pub bsod: i64,
    pub power_loss: i64,
    pub app_crashes: i64,
    pub last_bsod: String,
    pub top_apps: Vec<String>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Raw {
    pub disks: Vec<DiskHealth>,
    pub battery: Option<Battery>,
    pub crashes: CrashInfo,
    pub pending_reboot: bool,
    pub boot: String,
    pub uptime_h: f64,
    pub pagefile_mb: i64,
    pub pagefile_peak_mb: i64,
    pub sys_free: u64,
    pub sys_size: u64,
    pub defender: Option<bool>,
}

#[derive(Serialize)]
pub struct Check {
    pub id: String,
    pub title: String,
    /// ok | warn | bad | info
    pub status: String,
    pub detail: String,
    pub fix: String,
}

#[derive(Serialize)]
pub struct Report {
    pub score: u32,
    pub checks: Vec<Check>,
    pub raw: Raw,
}

const SCRIPT: &str = r#"
$ErrorActionPreference = 'SilentlyContinue'
$disks = @(Get-PhysicalDisk | ForEach-Object {
  $r = $_ | Get-StorageReliabilityCounter
  [pscustomobject]@{
    name = [string]$_.FriendlyName; media = [string]$_.MediaType; bus = [string]$_.BusType
    health = [string]$_.HealthStatus; size = [uint64]$_.Size
    temperature = $(if ($r.Temperature) { [int]$r.Temperature } else { $null })
    wear = $(if ($r.Wear -ne $null) { [int]$r.Wear } else { $null })
    hours = $(if ($r.PowerOnHours) { [int]$r.PowerOnHours } else { $null })
    read_errors = $(if ($r.ReadErrorsUncorrected -ne $null) { [int]$r.ReadErrorsUncorrected } else { $null })
  }
})
$bat = $null
$b = Get-CimInstance Win32_Battery | Select-Object -First 1
if ($b) {
  $d = (Get-CimInstance -Namespace root\wmi BatteryStaticData | Select-Object -First 1).DesignedCapacity
  $f = (Get-CimInstance -Namespace root\wmi BatteryFullChargedCapacity | Select-Object -First 1).FullChargedCapacity
  $bat = [pscustomobject]@{ charge = [int]$b.EstimatedChargeRemaining; design = $(if ($d) { [int]$d } else { $null }); full = $(if ($f) { [int]$f } else { $null }) }
}
$since = (Get-Date).AddDays(-30)
$bsod = @(Get-WinEvent -FilterHashtable @{ LogName = 'System'; ProviderName = 'Microsoft-Windows-WER-SystemErrorReporting'; Id = 1001; StartTime = $since })
$power = @(Get-WinEvent -FilterHashtable @{ LogName = 'System'; ProviderName = 'Microsoft-Windows-Kernel-Power'; Id = 41; StartTime = $since })
$apps = @(Get-WinEvent -FilterHashtable @{ LogName = 'Application'; ProviderName = 'Application Error'; Id = 1000; StartTime = (Get-Date).AddDays(-7) })
$top = @($apps | ForEach-Object { $_.Properties[0].Value } | Group-Object | Sort-Object Count -Descending | Select-Object -First 3 | ForEach-Object { "$($_.Name) ($($_.Count))" })
$pending = (Test-Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\WindowsUpdate\Auto Update\RebootRequired') -or
           (Test-Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Component Based Servicing\RebootPending')
$os = Get-CimInstance Win32_OperatingSystem
$pf = Get-CimInstance Win32_PageFileUsage | Measure-Object -Property AllocatedBaseSize, PeakUsage -Sum
$sys = Get-CimInstance Win32_LogicalDisk -Filter "DeviceID='$($env:SystemDrive)'"
$mp = Get-MpComputerStatus
[pscustomobject]@{
  disks = $disks; battery = $bat
  crashes = [pscustomobject]@{
    bsod = $bsod.Count; power_loss = $power.Count; app_crashes = $apps.Count
    last_bsod = $(if ($bsod.Count) { $bsod[0].TimeCreated.ToString('yyyy-MM-dd HH:mm') } else { '' })
    top_apps = $top
  }
  pending_reboot = [bool]$pending
  boot = $os.LastBootUpTime.ToString('yyyy-MM-dd HH:mm')
  uptime_h = [math]::Round(((Get-Date) - $os.LastBootUpTime).TotalHours, 1)
  pagefile_mb = [int]($pf | Where-Object Property -eq 'AllocatedBaseSize').Sum
  pagefile_peak_mb = [int]($pf | Where-Object Property -eq 'PeakUsage').Sum
  sys_free = [uint64]$sys.FreeSpace; sys_size = [uint64]$sys.Size
  defender = $(if ($mp) { [bool]$mp.RealTimeProtectionEnabled } else { $null })
} | ConvertTo-Json -Compress -Depth 4
"#;

fn check(id: &str, title: &str, status: &str, detail: String, fix: &str) -> Check {
    Check { id: id.into(), title: title.into(), status: status.into(), detail, fix: fix.into() }
}

fn gb(b: u64) -> String {
    format!("{:.0} GB", b as f64 / 1_073_741_824.0)
}

pub fn evaluate(raw: Raw) -> Report {
    let mut c = Vec::new();

    for d in &raw.disks {
        let h = d.health.to_lowercase();
        let ssd = d.media.eq_ignore_ascii_case("SSD");
        let mut extra = Vec::new();
        if let Some(t) = d.temperature {
            extra.push(format!("{t} °C"));
        }
        if let Some(w) = d.wear {
            extra.push(format!("desgaste {w}%"));
        }
        if let Some(h) = d.hours {
            extra.push(format!("{} h encendido", h));
        }
        let detail = format!("{} · {} {}{}", gb(d.size), if ssd { "SSD" } else { &d.media }, d.bus,
            if extra.is_empty() { String::new() } else { format!(" · {}", extra.join(" · ")) });
        let (status, fix) = if h.contains("unhealthy") || d.read_errors.unwrap_or(0) > 0 {
            ("bad", "Haz una copia de seguridad cuanto antes y revisa el disco con CrystalDiskInfo (en Herramientas).")
        } else if h.contains("warning") || d.wear.unwrap_or(0) >= 80 || d.temperature.unwrap_or(0) >= 65 {
            ("warn", "El disco muestra señales de desgaste o calor. Revisa la ventilación y ten respaldos al día.")
        } else {
            ("ok", "")
        };
        c.push(check("disk", &format!("Disco: {}", d.name), status, detail, fix));
    }

    if raw.sys_size > 0 {
        let pct = raw.sys_free as f64 / raw.sys_size as f64 * 100.0;
        let status = if pct < 8.0 { "bad" } else if pct < 15.0 { "warn" } else { "ok" };
        c.push(check("space", "Espacio libre en el disco del sistema", status,
            format!("{} libres de {} ({pct:.0}%)", gb(raw.sys_free), gb(raw.sys_size)),
            "Usa Limpieza y el Analizador de disco. Windows necesita al menos un 10-15% libre para ir fluido."));
    }

    let cr = &raw.crashes;
    let status = if cr.bsod >= 2 { "bad" } else if cr.bsod == 1 || cr.power_loss >= 3 { "warn" } else { "ok" };
    let mut detail = format!("{} pantallazos azules y {} apagados inesperados en 30 días", cr.bsod, cr.power_loss);
    if !cr.last_bsod.is_empty() {
        detail.push_str(&format!(" · último: {}", cr.last_bsod));
    }
    c.push(check("crashes", "Pantallazos azules y apagones", status, detail,
        "Suelen venir de drivers (sobre todo el de video), RAM inestable u overclock. Actualiza drivers, quita el \
         overclock y ejecuta «Reparar Windows». No borres los volcados de errores si quieres investigar."));

    let status = if cr.app_crashes >= 10 { "warn" } else { "ok" };
    let detail = if cr.top_apps.is_empty() {
        format!("{} cierres inesperados en 7 días", cr.app_crashes)
    } else {
        format!("{} cierres inesperados en 7 días · {}", cr.app_crashes, cr.top_apps.join(", "))
    };
    c.push(check("apps", "Programas que se cierran solos", status, detail,
        "Reinstala o actualiza los programas que más fallan."));

    if let Some(b) = &raw.battery {
        if let (Some(d), Some(f)) = (b.design, b.full) {
            if d > 0 {
                let pct = (f as f64 / d as f64 * 100.0).min(100.0);
                let status = if pct < 50.0 { "bad" } else if pct < 70.0 { "warn" } else { "ok" };
                c.push(check("battery", "Salud de la batería", status,
                    format!("Conserva el {pct:.0}% de su capacidad original"),
                    "Una batería por debajo del 70% dura bastante menos. Puedes generar el informe de batería de Windows."));
            }
        }
    }

    c.push(check("reboot", "Reinicio pendiente", if raw.pending_reboot { "warn" } else { "ok" },
        if raw.pending_reboot { "Windows tiene actualizaciones esperando un reinicio".into() } else { "Nada pendiente".into() },
        "Reinicia la PC para terminar de instalar las actualizaciones."));

    let status = if raw.uptime_h > 24.0 * 7.0 { "warn" } else { "ok" };
    c.push(check("uptime", "Tiempo encendida", status,
        format!("Desde {} ({:.0} h)", raw.boot, raw.uptime_h),
        "«Apagar» con inicio rápido no reinicia Windows del todo. Reinicia de vez en cuando para liberar memoria."));

    if raw.pagefile_mb > 0 {
        let status = if raw.pagefile_peak_mb as f64 > raw.pagefile_mb as f64 * 0.9 { "warn" } else { "ok" };
        c.push(check("pagefile", "Archivo de paginación", status,
            format!("{} MB asignados · pico de uso {} MB", raw.pagefile_mb, raw.pagefile_peak_mb),
            "Se está llenando: deja que Windows lo administre automáticamente o agrega más RAM."));
    } else {
        c.push(check("pagefile", "Archivo de paginación", "warn", "Está desactivado".into(),
            "Sin archivo de paginación algunos juegos y programas se cierran al quedarse sin memoria. Actívalo en modo automático."));
    }

    if let Some(on) = raw.defender {
        c.push(check("defender", "Protección antivirus en tiempo real", if on { "ok" } else { "warn" },
            if on { "Microsoft Defender está activo".into() } else { "La protección en tiempo real de Defender está apagada".into() },
            "Si no usas otro antivirus, actívala en Seguridad de Windows."));
    }

    let penalty: u32 = c.iter().map(|x| match x.status.as_str() { "bad" => 20, "warn" => 7, _ => 0 }).sum();
    Report { score: 100u32.saturating_sub(penalty), checks: c, raw }
}

pub fn report() -> Result<Report, String> {
    let out = sysutil::powershell(SCRIPT)?;
    let raw: Raw = serde_json::from_str(out.trim()).map_err(|e| format!("No se pudo leer el diagnóstico: {e}"))?;
    Ok(evaluate(raw))
}

/// Abre una consola visible que ejecuta una reparación conocida.
pub fn run_repair(kind: &str) -> Result<String, String> {
    let (title, cmd) = match kind {
        "dism" => ("Reparar imagen de Windows (DISM)", "DISM /Online /Cleanup-Image /RestoreHealth"),
        "sfc" => ("Reparar archivos del sistema (SFC)", "sfc /scannow"),
        "full" => ("Reparación completa", "DISM /Online /Cleanup-Image /RestoreHealth && sfc /scannow"),
        "chkdsk" => ("Revisar el disco del sistema", "chkdsk %SystemDrive% /scan"),
        _ => return Err("Reparación desconocida".into()),
    };
    if !sysutil::is_admin() {
        return Err("Necesita permisos de administrador.".into());
    }
    let full = format!("title {title} && echo {title} && echo. && {cmd} & echo. & echo Listo. Puedes cerrar esta ventana. & pause");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("cmd.exe")
            .creation_flags(0x0000_0010) // CREATE_NEW_CONSOLE: ventana visible con el progreso
            .raw_arg(format!("/c {full}"))
            .spawn()
            .map_err(|e| format!("No se pudo abrir la consola: {e}"))?;
    }
    #[cfg(not(windows))]
    let _ = full;
    Ok(title.into())
}

/// Genera el informe de batería de Windows y lo abre en el navegador.
pub fn battery_report() -> Result<(), String> {
    let path = crate::history::folder().join("informe-bateria.html");
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let p = path.to_string_lossy().into_owned();
    sysutil::run("powercfg", &["/batteryreport", "/output", &p])?;
    std::process::Command::new("explorer").arg(&p).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn puntaje() {
        let mut raw = Raw { sys_free: 50, sys_size: 100, pagefile_mb: 4096, pagefile_peak_mb: 100, ..Default::default() };
        assert_eq!(evaluate(Raw { ..raw_clone(&raw) }).score, 100);
        raw.crashes.bsod = 3;
        raw.sys_free = 5;
        let r = evaluate(raw);
        assert_eq!(r.score, 60);
        assert!(r.checks.iter().any(|c| c.id == "crashes" && c.status == "bad"));
    }

    fn raw_clone(r: &Raw) -> Raw {
        serde_json::from_str(&serde_json::to_string(r).unwrap()).unwrap()
    }
}
