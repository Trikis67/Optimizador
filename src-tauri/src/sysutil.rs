//! Utilidades comunes: ejecutar programas sin ventana, PowerShell, permisos de administrador.

use std::process::Command;

/// Crea un comando que no abre ventana de consola.
pub fn hidden(program: &str) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd
}

/// Ejecuta un programa y devuelve su salida. Falla si el código de salida no es 0.
pub fn run(program: &str, args: &[&str]) -> Result<String, String> {
    let out = hidden(program)
        .args(args)
        .output()
        .map_err(|e| format!("No se pudo ejecutar {program}: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    if out.status.success() {
        Ok(stdout)
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        Err(friendly_error(&format!("{} {}", stdout.trim(), stderr.trim())))
    }
}

/// Ejecuta un script de PowerShell y devuelve lo que imprime (UTF-8).
/// El script se manda codificado en Base64 para evitar problemas con comillas.
pub fn powershell(script: &str) -> Result<String, String> {
    let full = format!(
        "$ProgressPreference='SilentlyContinue';$ErrorActionPreference='Stop';\
         [Console]::OutputEncoding=[System.Text.Encoding]::UTF8;\
         try {{\n{script}\n}} catch {{ Write-Output ('ERR:' + $_.Exception.Message); exit 1 }}"
    );
    let utf16: Vec<u8> = full.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    let encoded = base64(&utf16);
    let out = hidden("powershell")
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand", &encoded])
        .output()
        .map_err(|e| format!("No se pudo ejecutar PowerShell: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim_start_matches('\u{feff}').to_string();
    if out.status.success() {
        Ok(stdout)
    } else if let Some(pos) = stdout.find("ERR:") {
        Err(friendly_error(stdout[pos + 4..].trim()))
    } else {
        Err(friendly_error(&format!("PowerShell falló: {}", stdout.trim())))
    }
}

/// Traduce los errores más comunes a algo entendible.
pub fn friendly_error(msg: &str) -> String {
    let lower = msg.to_lowercase();
    if lower.contains("access is denied")
        || lower.contains("acceso denegado")
        || lower.contains("denied")
        || lower.contains("failed 5")
        || lower.contains("requires elevation")
        || lower.contains("privilegios")
    {
        "Esta acción necesita permisos de administrador. Usa «Reiniciar como administrador».".to_string()
    } else {
        msg.trim().to_string()
    }
}

/// Codificador Base64 estándar.
pub fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

/// Convierte texto a UTF-16 terminado en cero (para la API de Windows).
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Lee una cadena UTF-16 terminada en cero.
///
/// # Safety
/// `p` debe apuntar a una cadena válida terminada en cero (o ser nulo).
pub unsafe fn from_wide_ptr(p: *const u16) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut len = 0usize;
    while *p.add(len) != 0 {
        len += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(p, len))
}

#[link(name = "shell32")]
extern "system" {
    fn IsUserAnAdmin() -> i32;
}

/// ¿La app se está ejecutando como administrador?
pub fn is_admin() -> bool {
    unsafe { IsUserAnAdmin() != 0 }
}

/// Vuelve a abrir la app pidiendo permisos de administrador (UAC).
pub fn relaunch_as_admin() -> Result<(), String> {
    if cfg!(debug_assertions) {
        return Err("En modo desarrollo no se puede reiniciar sola. Cierra VS Code, ábrelo con clic derecho → \
                    «Ejecutar como administrador» y vuelve a correr npm run dev."
            .into());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let path = exe.to_string_lossy().replace('\'', "''");
    powershell(&format!("Start-Process -FilePath '{path}' -Verb RunAs"))
        .map_err(|_| "Se canceló el permiso de administrador".to_string())?;
    Ok(())
}

/// Abre una página web, una sección de Configuración de Windows o una herramienta del sistema.
/// Solo se permiten destinos seguros conocidos.
pub fn open_target(target: &str) -> Result<(), String> {
    const TOOLS: &[&str] = &["devmgmt.msc", "cleanmgr.exe", "taskmgr.exe", "msconfig.exe", "rstrui.exe", "services.msc"];
    let t = target.trim();
    if t.starts_with("https://") || t.starts_with("ms-settings:") {
        hidden("explorer").arg(t).spawn().map_err(|e| e.to_string())?;
        Ok(())
    } else if TOOLS.contains(&t) {
        hidden("cmd").args(["/C", "start", "", t]).spawn().map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("Destino no permitido".into())
    }
}

/// Días transcurridos desde 1970-01-01 para una fecha civil (algoritmo de Howard Hinnant).
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub fn today_days() -> i64 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    (secs / 86_400) as i64
}

/// Tamaño de una carpeta (bytes, archivos). Ignora lo que no se puede leer.
pub fn dir_size(path: &std::path::Path) -> (u64, u64) {
    let mut bytes = 0;
    let mut files = 0;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            if ft.is_dir() {
                stack.push(entry.path());
            } else {
                bytes += entry.metadata().map(|m| m.len()).unwrap_or(0);
                files += 1;
            }
        }
    }
    (bytes, files)
}

/// Borra el contenido de una carpeta (no la carpeta en sí). Devuelve (bytes liberados, archivos que no se pudieron borrar).
/// Los archivos en uso se saltan.
pub fn clear_dir(path: &std::path::Path) -> (u64, u64) {
    let mut freed = 0;
    let mut failed = 0;
    let Ok(rd) = std::fs::read_dir(path) else { return (0, 0) };
    for entry in rd.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        let p = entry.path();
        if ft.is_symlink() {
            // Solo quitamos el enlace, nunca lo que apunta
            if std::fs::remove_file(&p).is_err() {
                let _ = std::fs::remove_dir(&p);
            }
        } else if ft.is_dir() {
            let (f, x) = clear_dir(&p);
            freed += f;
            failed += x;
            let _ = std::fs::remove_dir(&p);
        } else {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            if std::fs::remove_file(&p).is_ok() {
                freed += size;
            } else {
                failed += 1;
            }
        }
    }
    (freed, failed)
}

/// Variable de entorno como ruta.
pub fn env_path(var: &str) -> Option<std::path::PathBuf> {
    std::env::var_os(var).map(std::path::PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_ok() {
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
        assert_eq!(base64(b"hola mundo"), "aG9sYSBtdW5kbw==");
    }

    #[test]
    fn fechas() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
        assert_eq!(days_from_civil(2026, 10, 30) - days_from_civil(2026, 1, 1), 302);
    }
}
