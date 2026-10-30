//! Planes de energía de Windows usando `powercfg`.

use serde::Serialize;

/// Plan "Máximo rendimiento" (Ultimate Performance) que Windows trae oculto.
const ULTIMATE_SOURCE: &str = "e9a42b02-d5df-448d-aa00-03f14749eb61";
/// GUID fijo para nuestra copia, así no se crean duplicados cada vez que se activa.
pub const ULTIMATE_COPY: &str = "6a3c1f0e-9b5d-4e2a-8c71-0f4d2b9e7a51";
pub const BALANCED: &str = "381b4222-f694-41f0-9685-ff5bb260df2e";

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct PowerPlan {
    pub guid: String,
    pub name: String,
    pub active: bool,
}

fn is_guid(s: &str) -> bool {
    s.len() == 36
        && s.chars().enumerate().all(|(i, c)| {
            if [8, 13, 18, 23].contains(&i) { c == '-' } else { c.is_ascii_hexdigit() }
        })
}

/// Interpreta la salida de `powercfg /list` (funciona en español e inglés).
/// Ejemplo de línea: `GUID del plan de energía: 381b4222-...  (Equilibrado) *`
pub fn parse_list(output: &str) -> Vec<PowerPlan> {
    let mut plans = Vec::new();
    for line in output.lines() {
        let Some(open) = line.find('(') else { continue };
        let Some(close) = line.rfind(')') else { continue };
        if close <= open {
            continue;
        }
        let Some(guid) = line[..open].split_whitespace().last() else { continue };
        if !is_guid(guid) {
            continue;
        }
        plans.push(PowerPlan {
            guid: guid.to_lowercase(),
            name: line[open + 1..close].trim().to_string(),
            active: line.trim_end().ends_with('*'),
        });
    }
    plans
}

#[cfg(windows)]
mod sys {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    /// Ejecuta un comando sin abrir ventana de consola y en UTF-8 (para que salgan bien las tildes).
    pub fn powercfg(args: &str) -> Result<String, String> {
        let out = Command::new("cmd")
            // raw_arg: se pasa tal cual a cmd. `args` solo contiene constantes o GUIDs ya validados.
            .raw_arg(format!("/C chcp 65001 >nul & powercfg {args}"))
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("No se pudo ejecutar powercfg: {e}"))?;
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        if out.status.success() {
            Ok(stdout)
        } else {
            let stderr = String::from_utf8_lossy(&out.stderr);
            Err(format!("powercfg falló: {} {}", stdout.trim(), stderr.trim()))
        }
    }
}

#[cfg(not(windows))]
mod sys {
    pub fn powercfg(_args: &str) -> Result<String, String> {
        Err("Esta función solo está disponible en Windows".into())
    }
}

pub fn list() -> Result<Vec<PowerPlan>, String> {
    Ok(parse_list(&sys::powercfg("/list")?))
}

pub fn set_active(guid: &str) -> Result<(), String> {
    if !is_guid(guid) {
        return Err("GUID inválido".into());
    }
    sys::powercfg(&format!("/setactive {guid}")).map(|_| ())
}

/// Crea (si no existe) y activa el plan de Máximo rendimiento.
pub fn enable_ultimate() -> Result<(), String> {
    let exists = list()?.iter().any(|p| p.guid == ULTIMATE_COPY);
    if !exists {
        sys::powercfg(&format!("/duplicatescheme {ULTIMATE_SOURCE} {ULTIMATE_COPY}")).map_err(|e| {
            format!(
                "Windows no permitió crear el plan de Máximo rendimiento. \
                 Algunas laptops con 'Modern Standby' solo permiten el plan Equilibrado. ({e})"
            )
        })?;
    }
    set_active(ULTIMATE_COPY)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpreta_powercfg() {
        let es = "Planes de energía existentes (* Activo)\n-----------------------------------\n\
                  GUID del plan de energía: 381b4222-f694-41f0-9685-ff5bb260df2e  (Equilibrado)\n\
                  GUID del plan de energía: 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c  (Alto rendimiento) *\n";
        let plans = parse_list(es);
        assert_eq!(plans.len(), 2);
        assert_eq!(plans[0].name, "Equilibrado");
        assert!(!plans[0].active);
        assert_eq!(plans[1].name, "Alto rendimiento");
        assert!(plans[1].active);

        let en = "Power Scheme GUID: 381b4222-f694-41f0-9685-ff5bb260df2e  (Balanced) *";
        assert_eq!(parse_list(en)[0].name, "Balanced");
        assert!(!is_guid("381b4222; del *"));
    }
}
