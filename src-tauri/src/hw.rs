//! Perfil del equipo: se usa para recomendar ajustes según el hardware.

use crate::sysutil;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Gpu {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub vram: u64,
    #[serde(default)]
    pub driver_version: String,
    #[serde(default)]
    pub driver_date: String,
    #[serde(default)]
    pub vendor: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Hw {
    #[serde(default)]
    pub cpu: String,
    #[serde(default)]
    pub cores: u32,
    #[serde(default)]
    pub ram: u64,
    #[serde(default)]
    pub laptop: bool,
    /// "SSD", "HDD" o "Unspecified"
    #[serde(default)]
    pub system_disk: String,
    #[serde(default)]
    pub os: String,
    #[serde(default)]
    pub build: u32,
    #[serde(default)]
    pub gpus: Vec<Gpu>,
    /// Navegadores instalados (ids de browsers.rs). Se llena desde Rust.
    #[serde(default)]
    pub browsers: Vec<String>,
}

impl Hw {
    pub fn ram_gb(&self) -> f64 {
        self.ram as f64 / 1_073_741_824.0
    }
    pub fn is_ssd(&self) -> bool {
        self.system_disk.eq_ignore_ascii_case("SSD")
    }
    pub fn is_hdd(&self) -> bool {
        self.system_disk.eq_ignore_ascii_case("HDD")
    }
    /// ¿Tiene tarjeta gráfica dedicada (NVIDIA, AMD Radeon RX/Pro o Intel Arc)?
    pub fn has_dgpu(&self) -> bool {
        self.gpus.iter().any(|g| {
            let n = g.name.to_lowercase();
            n.contains("nvidia") || n.contains("radeon rx") || n.contains("radeon pro") || n.contains(" arc")
        })
    }
    pub fn low_end(&self) -> bool {
        self.ram_gb() <= 8.5 || self.cores <= 4
    }
    pub fn has_browser(&self, id: &str) -> bool {
        self.browsers.iter().any(|b| b == id)
    }
}

const SCRIPT: &str = r#"
$cs = Get-CimInstance Win32_ComputerSystem
$cpu = Get-CimInstance Win32_Processor | Select-Object -First 1
$os = Get-CimInstance Win32_OperatingSystem
$bat = @(Get-CimInstance Win32_Battery -ErrorAction SilentlyContinue).Count -gt 0
$disk = 'Unspecified'
try {
  $n = (Get-Partition -DriveLetter $env:SystemDrive[0] -ErrorAction Stop).DiskNumber
  $pd = Get-PhysicalDisk -ErrorAction Stop | Where-Object { $_.DeviceId -eq "$n" } | Select-Object -First 1
  if ($pd) { $disk = [string]$pd.MediaType }
} catch {}
$gpus = @()
Get-ChildItem 'HKLM:\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}' -ErrorAction SilentlyContinue |
  Where-Object { $_.PSChildName -match '^\d{4}$' } | ForEach-Object {
    $p = Get-ItemProperty $_.PSPath -ErrorAction SilentlyContinue
    if ($p -and $p.DriverDesc) {
      $mem = $p.'HardwareInformation.qwMemorySize'
      if (-not $mem) { $mem = $p.'HardwareInformation.MemorySize' }
      $vram = 0
      try { $vram = [uint64]$mem } catch { $vram = 0 }
      $gpus += [pscustomobject]@{
        name = [string]$p.DriverDesc; vram = $vram
        driver_version = [string]$p.DriverVersion; driver_date = [string]$p.DriverDate
        vendor = [string]$p.ProviderName
      }
    }
  }
[pscustomobject]@{
  cpu = ([string]$cpu.Name).Trim(); cores = [int]$cpu.NumberOfLogicalProcessors
  ram = [uint64]$cs.TotalPhysicalMemory; laptop = $bat; system_disk = $disk
  os = [string]$os.Caption; build = [int]$os.BuildNumber; gpus = [object[]]$gpus
} | ConvertTo-Json -Depth 4 -Compress
"#;

pub fn detect() -> Result<Hw, String> {
    let out = sysutil::powershell(SCRIPT)?;
    let mut hw: Hw = serde_json::from_str(out.trim()).map_err(|e| format!("No se pudo leer el hardware: {e}"))?;
    hw.gpus.retain(|g| {
        let n = g.name.to_lowercase();
        !n.contains("basic display") && !n.contains("basic render") && !n.contains("remote display")
    });
    hw.browsers = crate::browsers::installed_ids();
    Ok(hw)
}
