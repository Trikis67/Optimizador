//! Catálogo de optimizaciones. Cada ajuste se puede aplicar y deshacer, y antes de aplicarlo se
//! guarda un respaldo de los valores originales en %LOCALAPPDATA%\Optimizador\respaldo.json.

use crate::hw::Hw;
use crate::sysutil;
use crate::winreg::{self, Hive, RegValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

const CU: Hive = Hive::Hkcu;
const LM: Hive = Hive::Hklm;

#[derive(Clone, Copy)]
pub enum Want {
    D(u32),
    S(&'static str),
    B(&'static [u8]),
}

impl Want {
    fn value(self) -> RegValue {
        match self {
            Want::D(d) => RegValue::Dword(d),
            Want::S(s) => RegValue::Sz(s.to_string()),
            Want::B(b) => RegValue::Binary(b.to_vec()),
        }
    }
    fn matches(self, v: &RegValue) -> bool {
        match (self, v) {
            (Want::D(a), RegValue::Dword(b)) => a == *b,
            (Want::S(a), RegValue::Sz(b)) => a.eq_ignore_ascii_case(b.trim()),
            (Want::B(a), RegValue::Binary(b)) => b.len() >= a.len() && &b[..a.len()] == a,
            _ => false,
        }
    }
}

pub enum Step {
    /// Valor del registro. `off = None` significa borrar el valor al deshacer.
    Reg { hive: Hive, key: &'static str, name: &'static str, on: Want, off: Option<Want> },
    /// Tipo de inicio de un servicio: auto, delayed-auto, demand, disabled.
    Service { name: &'static str, on: &'static str, off: &'static str },
    /// Tarea programada que se desactiva.
    Task { path: &'static str },
    /// Comando a ejecutar al aplicar / deshacer.
    Cmd { on: &'static [&'static str], off: &'static [&'static str] },
    /// Solo para saber si el ajuste está activo.
    Check { hive: Hive, key: &'static str, name: &'static str, on: Want },
}

#[derive(Serialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Rec {
    Recomendado,
    Opcional,
    NoRecomendado,
    NoAplica,
}

pub struct Tweak {
    pub id: &'static str,
    pub cat: &'static str,
    pub title: &'static str,
    pub what: &'static str,
    pub pros: &'static str,
    pub cons: &'static str,
    pub reboot: bool,
    pub steps: Vec<Step>,
    pub rec: fn(&Hw) -> (Rec, String),
}

impl Tweak {
    pub fn admin(&self) -> bool {
        self.steps.iter().any(|s| match s {
            // HKCU\Software\Policies también está protegido y solo un administrador puede escribirlo.
            Step::Reg { hive, key, .. } => *hive == LM || key.to_ascii_lowercase().starts_with(r"software\policies"),
            Step::Check { .. } => false,
            _ => true,
        })
    }
}

#[derive(Serialize)]
pub struct TweakInfo {
    pub id: String,
    pub cat: String,
    pub title: String,
    pub what: String,
    pub pros: String,
    pub cons: String,
    pub admin: bool,
    pub reboot: bool,
    pub rec: Rec,
    pub reason: String,
    /// "on", "off", "partial" o "na"
    pub status: String,
}

// ---------------------------------------------------------------------------------------------
// Ayudantes para escribir el catálogo
// ---------------------------------------------------------------------------------------------

fn reg(hive: Hive, key: &'static str, name: &'static str, on: Want, off: Option<Want>) -> Step {
    Step::Reg { hive, key, name, on, off }
}
fn svc(name: &'static str, on: &'static str, off: &'static str) -> Step {
    Step::Service { name, on, off }
}
fn task(path: &'static str) -> Step {
    Step::Task { path }
}
fn r(level: Rec, why: &str) -> (Rec, String) {
    (level, why.to_string())
}

#[allow(clippy::too_many_arguments)]
fn t(
    id: &'static str,
    cat: &'static str,
    title: &'static str,
    what: &'static str,
    pros: &'static str,
    cons: &'static str,
    reboot: bool,
    steps: Vec<Step>,
    rec: fn(&Hw) -> (Rec, String),
) -> Tweak {
    Tweak { id, cat, title, what, pros, cons, reboot, steps, rec }
}

const GAMES_KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile\Tasks\Games";
const SYSPROFILE: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile";
const CDM: &str = r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager";
const BROWSER_POLICY_NOTE: &str = "El navegador mostrará «Administrado por tu organización». Es normal: así se ven \
                                   los ajustes aplicados por política. Al deshacerlo desaparece.";

pub fn catalog() -> Vec<Tweak> {
    vec![
        // ================================ GAMING ================================
        t(
            "game_mode", "gaming", "Modo de juego de Windows",
            "Cuando detecta un juego, Windows le da prioridad de CPU y GPU, pausa la instalación de drivers por \
             Windows Update y evita notificaciones de reinicio.",
            "FPS más estables, sobre todo si tienes programas abiertos en segundo plano.",
            "En algunos equipos muy justos puede dar tirones al transmitir o grabar. Si lo notas, desactívalo.",
            false,
            vec![
                reg(CU, r"Software\Microsoft\GameBar", "AutoGameModeEnabled", Want::D(1), None),
                reg(CU, r"Software\Microsoft\GameBar", "AllowAutoGameMode", Want::D(1), None),
            ],
            |_| r(Rec::Recomendado, "Útil en cualquier PC que se use para jugar."),
        ),
        t(
            "game_dvr", "gaming", "Desactivar grabación en segundo plano (Game DVR)",
            "Apaga la función de Xbox Game Bar que graba continuamente los últimos minutos de juego para que \
             puedas guardar un clip.",
            "Libera algo de GPU, CPU y disco mientras juegas.",
            "Ya no podrás guardar clips con Win + Alt + G / Win + Alt + R.",
            false,
            vec![
                reg(CU, r"System\GameConfigStore", "GameDVR_Enabled", Want::D(0), Some(Want::D(1))),
                reg(CU, r"Software\Microsoft\Windows\CurrentVersion\GameDVR", "AppCaptureEnabled", Want::D(0), Some(Want::D(1))),
            ],
            |_| r(Rec::Recomendado, "Recomendado, salvo que grabes clips con la Game Bar."),
        ),
        t(
            "hags", "gaming", "Programación de GPU acelerada por hardware (HAGS)",
            "La tarjeta gráfica administra su propia memoria de video en lugar de que lo haga el procesador.",
            "Menos latencia y algo menos de carga en la CPU. Es obligatorio para DLSS Frame Generation en NVIDIA.",
            "En algunos juegos viejos o con drivers desactualizados puede causar problemas. Requiere reiniciar.",
            true,
            vec![reg(LM, r"SYSTEM\CurrentControlSet\Control\GraphicsDrivers", "HwSchMode", Want::D(2), Some(Want::D(1)))],
            |h| {
                let n: String = h.gpus.iter().map(|g| g.name.to_lowercase()).collect::<Vec<_>>().join(" ");
                if n.contains("nvidia") || n.contains("radeon rx") {
                    r(Rec::Recomendado, "Tu tarjeta gráfica dedicada lo soporta (NVIDIA GTX 1000+ / AMD RX 5000+).")
                } else if h.has_dgpu() {
                    r(Rec::Opcional, "Tu GPU podría soportarlo; pruébalo y compara.")
                } else {
                    r(Rec::NoRecomendado, "Con gráficos integrados el beneficio es mínimo.")
                }
            },
        ),
        t(
            "mouse_accel", "gaming", "Desactivar aceleración del mouse",
            "Apaga «Mejorar precisión del puntero». Así el puntero se mueve siempre la misma distancia por cada \
             centímetro que mueves el mouse, sin importar la velocidad.",
            "Puntería más consistente en juegos de disparos (memoria muscular).",
            "Al principio el puntero se siente más lento en el escritorio. Se aplica al cerrar sesión.",
            true,
            vec![
                reg(CU, r"Control Panel\Mouse", "MouseSpeed", Want::S("0"), Some(Want::S("1"))),
                reg(CU, r"Control Panel\Mouse", "MouseThreshold1", Want::S("0"), Some(Want::S("6"))),
                reg(CU, r"Control Panel\Mouse", "MouseThreshold2", Want::S("0"), Some(Want::S("10"))),
            ],
            |_| r(Rec::Recomendado, "Recomendado si juegas con mouse (FPS, MOBA)."),
        ),
        t(
            "game_priority", "gaming", "Más prioridad de CPU y GPU para juegos",
            "Ajusta el servicio multimedia de Windows (MMCSS) para que los juegos tengan más prioridad que las \
             tareas de fondo.",
            "Puede reducir tirones cuando hay programas trabajando en segundo plano.",
            "La mejora suele ser pequeña. Requiere reiniciar.",
            true,
            vec![
                reg(LM, GAMES_KEY, "GPU Priority", Want::D(8), Some(Want::D(8))),
                reg(LM, GAMES_KEY, "Priority", Want::D(6), Some(Want::D(2))),
                reg(LM, GAMES_KEY, "Scheduling Category", Want::S("High"), Some(Want::S("Medium"))),
                reg(LM, GAMES_KEY, "SFIO Priority", Want::S("High"), Some(Want::S("Normal"))),
                reg(LM, SYSPROFILE, "SystemResponsiveness", Want::D(10), Some(Want::D(20))),
            ],
            |h| {
                if h.low_end() {
                    r(Rec::Recomendado, "En equipos con pocos núcleos o poca RAM ayuda a que el juego no compita con el fondo.")
                } else {
                    r(Rec::Opcional, "Tu equipo tiene recursos de sobra; el efecto será pequeño.")
                }
            },
        ),
        t(
            "network_throttling", "gaming", "Quitar el límite de red multimedia",
            "Windows limita el tráfico de red mientras reproduce audio o video. Este ajuste quita ese límite.",
            "Puede reducir picos de ping cuando juegas en línea con música o un video abierto.",
            "Efecto pequeño en la mayoría de conexiones. Requiere reiniciar.",
            true,
            vec![reg(LM, SYSPROFILE, "NetworkThrottlingIndex", Want::D(0xffff_ffff), Some(Want::D(10)))],
            |_| r(Rec::Opcional, "Pruébalo si juegas en línea y notas picos de ping."),
        ),
        // ================================ SISTEMA ================================
        t(
            "visual_effects", "sistema", "Efectos visuales en modo rendimiento",
            "Quita animaciones de ventanas, sombras y efectos de desvanecido (igual que «Ajustar para obtener el \
             mejor rendimiento»). Mantiene el suavizado de letras.",
            "Ventanas y menús más rápidos y menos uso de GPU y RAM.",
            "Windows se ve más simple. Se aplica al cerrar sesión.",
            true,
            vec![
                reg(CU, r"Software\Microsoft\Windows\CurrentVersion\Explorer\VisualEffects", "VisualFXSetting", Want::D(2), Some(Want::D(0))),
                reg(
                    CU, r"Control Panel\Desktop", "UserPreferencesMask",
                    Want::B(&[0x90, 0x12, 0x03, 0x80, 0x10, 0x00, 0x00, 0x00]),
                    Some(Want::B(&[0x9E, 0x1E, 0x07, 0x80, 0x12, 0x00, 0x00, 0x00])),
                ),
                reg(CU, r"Control Panel\Desktop\WindowMetrics", "MinAnimate", Want::S("0"), Some(Want::S("1"))),
            ],
            |h| {
                if h.low_end() || !h.has_dgpu() {
                    r(Rec::Recomendado, "Tu equipo tiene recursos limitados o gráficos integrados; se notará.")
                } else {
                    r(Rec::Opcional, "Tu equipo es potente; la diferencia será poca. Es cuestión de gusto.")
                }
            },
        ),
        t(
            "transparency", "sistema", "Desactivar transparencias",
            "Quita el efecto de vidrio translúcido de la barra de tareas, el menú Inicio y las ventanas.",
            "Menos trabajo para la GPU, sobre todo en gráficos integrados.",
            "Solo cambia la apariencia.",
            false,
            vec![reg(CU, r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize", "EnableTransparency", Want::D(0), Some(Want::D(1)))],
            |h| {
                if !h.has_dgpu() || h.low_end() {
                    r(Rec::Recomendado, "Con gráficos integrados o poca RAM ayuda un poco.")
                } else {
                    r(Rec::Opcional, "Con tu tarjeta gráfica el impacto es casi nulo.")
                }
            },
        ),
        t(
            "startup_delay", "sistema", "Quitar el retraso de los programas de inicio",
            "Windows espera unos segundos antes de abrir los programas de inicio. Esto quita esa espera.",
            "Los programas de inicio abren antes.",
            "En discos duros mecánicos puede hacer que el escritorio tarde más en estar listo.",
            false,
            vec![reg(CU, r"Software\Microsoft\Windows\CurrentVersion\Explorer\Serialize", "StartupDelayInMSec", Want::D(0), None)],
            |h| {
                if h.is_ssd() {
                    r(Rec::Recomendado, "Tu disco del sistema es SSD; no hace falta esperar.")
                } else if h.is_hdd() {
                    r(Rec::NoRecomendado, "Tu disco del sistema es mecánico (HDD); el retraso ayuda a que cargue el escritorio primero.")
                } else {
                    r(Rec::Opcional, "No se pudo saber si tu disco es SSD o HDD.")
                }
            },
        ),
        t(
            "menu_delay", "sistema", "Menús más rápidos",
            "Reduce la espera antes de que se abran los submenús (de 400 a 100 milisegundos).",
            "La interfaz se siente más ágil.",
            "Ninguna importante. Se aplica al cerrar sesión.",
            true,
            vec![reg(CU, r"Control Panel\Desktop", "MenuShowDelay", Want::S("100"), Some(Want::S("400")))],
            |_| r(Rec::Recomendado, "Mejora la sensación de rapidez en cualquier PC."),
        ),
        t(
            "background_apps", "sistema", "Bloquear apps de la Tienda en segundo plano",
            "Impide que las aplicaciones de Microsoft Store sigan funcionando cuando están cerradas.",
            "Menos RAM y CPU usadas en segundo plano.",
            "Apps de la Tienda como Correo, Teléfono o WhatsApp no avisarán de mensajes si están cerradas.",
            false,
            vec![
                reg(CU, r"Software\Microsoft\Windows\CurrentVersion\BackgroundAccessApplications", "GlobalUserDisabled", Want::D(1), Some(Want::D(0))),
                reg(CU, r"Software\Microsoft\Windows\CurrentVersion\Search", "BackgroundAppGlobalToggle", Want::D(0), Some(Want::D(1))),
            ],
            |h| {
                if h.ram_gb() <= 8.5 {
                    r(Rec::Recomendado, "Tienes 8 GB de RAM o menos; cada MB cuenta.")
                } else {
                    r(Rec::Opcional, "Con tu RAM el ahorro es pequeño. Depende de si usas apps de la Tienda.")
                }
            },
        ),
        t(
            "suggestions", "sistema", "Quitar sugerencias, anuncios y apps automáticas",
            "Desactiva los consejos, anuncios del menú Inicio y la instalación silenciosa de apps promocionadas \
             (juegos y apps que aparecen solos).",
            "Menos procesos y descargas en segundo plano, y un menú Inicio más limpio.",
            "Ninguna.",
            false,
            vec![
                reg(CU, CDM, "SilentInstalledAppsEnabled", Want::D(0), Some(Want::D(1))),
                reg(CU, CDM, "SystemPaneSuggestionsEnabled", Want::D(0), Some(Want::D(1))),
                reg(CU, CDM, "SubscribedContent-338388Enabled", Want::D(0), Some(Want::D(1))),
                reg(CU, CDM, "SubscribedContent-338389Enabled", Want::D(0), Some(Want::D(1))),
                reg(CU, CDM, "SubscribedContent-353694Enabled", Want::D(0), Some(Want::D(1))),
                reg(CU, CDM, "SubscribedContent-353696Enabled", Want::D(0), Some(Want::D(1))),
                reg(CU, CDM, "SoftLandingEnabled", Want::D(0), Some(Want::D(1))),
            ],
            |_| r(Rec::Recomendado, "Sin desventajas para nadie."),
        ),
        t(
            "search_web", "sistema", "Quitar resultados de Bing en el menú Inicio",
            "La búsqueda del menú Inicio deja de consultar internet y solo busca en tu PC.",
            "Búsquedas más rápidas y sin anuncios web.",
            "Ya no verás resultados web al buscar desde Inicio. Se aplica al reiniciar el Explorador o la PC.",
            true,
            vec![reg(CU, r"Software\Policies\Microsoft\Windows\Explorer", "DisableSearchBoxSuggestions", Want::D(1), None)],
            |_| r(Rec::Recomendado, "Hace la búsqueda de Windows más rápida."),
        ),
        t(
            "hibernation", "sistema", "Desactivar hibernación",
            "Borra el archivo hiberfil.sys, que reserva en el disco C: alrededor del 40% de tu RAM. También \
             desactiva el «Inicio rápido».",
            "Libera varios GB en el disco del sistema. Apagar y encender deja el sistema más limpio.",
            "No podrás hibernar. El encendido puede tardar unos segundos más.",
            false,
            vec![
                Step::Cmd { on: &["powercfg", "/hibernate", "off"], off: &["powercfg", "/hibernate", "on"] },
                Step::Check { hive: LM, key: r"SYSTEM\CurrentControlSet\Control\Power", name: "HibernateEnabled", on: Want::D(0) },
            ],
            |h| {
                let gb = h.ram_gb() * 0.4;
                if h.laptop {
                    r(Rec::NoRecomendado, "Tienes una laptop: la hibernación protege tu trabajo si se acaba la batería.")
                } else {
                    (Rec::Recomendado, format!("Es una PC de escritorio; liberarías unos {gb:.0} GB en C:."))
                }
            },
        ),
        // ================================ PRIVACIDAD ================================
        t(
            "telemetry", "privacidad", "Reducir telemetría de Windows",
            "Desactiva el servicio que envía datos de uso a Microsoft (DiagTrack) y las tareas de \
             «Compatibility Appraiser» y «Experiencia del cliente».",
            "Menos uso de CPU, disco y red en segundo plano. El Compatibility Appraiser es conocido por \
             causar picos de uso de disco.",
            "Microsoft recibe menos información para diagnosticar problemas. Windows Update sigue funcionando.",
            false,
            vec![
                svc("DiagTrack", "disabled", "auto"),
                svc("dmwappushservice", "disabled", "demand"),
                task(r"\Microsoft\Windows\Application Experience\Microsoft Compatibility Appraiser"),
                task(r"\Microsoft\Windows\Application Experience\ProgramDataUpdater"),
                task(r"\Microsoft\Windows\Customer Experience Improvement Program\Consolidator"),
                task(r"\Microsoft\Windows\Customer Experience Improvement Program\UsbCeip"),
                reg(LM, r"SOFTWARE\Policies\Microsoft\Windows\DataCollection", "AllowTelemetry", Want::D(0), None),
            ],
            |_| r(Rec::Recomendado, "Recomendado para cualquier PC."),
        ),
        t(
            "advertising_id", "privacidad", "Desactivar el ID de publicidad",
            "Windows asigna un identificador para mostrarte anuncios personalizados en apps. Esto lo apaga.",
            "Más privacidad.",
            "Ninguna.",
            false,
            vec![reg(CU, r"Software\Microsoft\Windows\CurrentVersion\AdvertisingInfo", "Enabled", Want::D(0), Some(Want::D(1)))],
            |_| r(Rec::Recomendado, "Sin desventajas."),
        ),
        t(
            "activity_history", "privacidad", "Desactivar historial de actividad",
            "Windows deja de guardar y subir la lista de programas y archivos que usas.",
            "Más privacidad y algo menos de trabajo en segundo plano.",
            "Se pierde la función de retomar actividades en otros dispositivos.",
            false,
            vec![
                reg(LM, r"SOFTWARE\Policies\Microsoft\Windows\System", "PublishUserActivities", Want::D(0), None),
                reg(LM, r"SOFTWARE\Policies\Microsoft\Windows\System", "UploadUserActivities", Want::D(0), None),
            ],
            |_| r(Rec::Opcional, "Cuestión de privacidad; el impacto en rendimiento es pequeño."),
        ),
        t(
            "delivery_opt", "privacidad", "No compartir actualizaciones con otras PCs (P2P)",
            "Windows Update puede subir partes de las actualizaciones a otras computadoras por internet. Esto \
             hace que solo descargue de Microsoft.",
            "No se usa tu internet de subida en segundo plano; útil si juegas en línea o tienes datos limitados.",
            "Ninguna para uso en casa.",
            false,
            vec![reg(LM, r"SOFTWARE\Policies\Microsoft\Windows\DeliveryOptimization", "DODownloadMode", Want::D(0), None)],
            |_| r(Rec::Recomendado, "Evita que se use tu ancho de banda sin que lo sepas."),
        ),
        // ================================ SERVICIOS ================================
        t(
            "svc_sysmain", "servicios", "SysMain (Superfetch)",
            "Precarga en la RAM los programas que más usas para que abran más rápido.",
            "Desactivarlo puede quitar el «disco al 100%» en discos duros mecánicos lentos.",
            "Los programas pueden tardar un poco más en abrir.",
            false,
            vec![svc("SysMain", "disabled", "auto")],
            |h| {
                if h.is_ssd() {
                    r(Rec::NoRecomendado, "Con SSD funciona bien y casi no consume. Mejor dejarlo activo.")
                } else if h.is_hdd() && h.ram_gb() <= 8.5 {
                    r(Rec::Opcional, "Disco mecánico y poca RAM: desactívalo si ves el disco al 100% con frecuencia.")
                } else {
                    r(Rec::Opcional, "Solo si notas uso de disco alto sin motivo.")
                }
            },
        ),
        t(
            "svc_wsearch", "servicios", "Indexador de búsqueda (Windows Search)",
            "Mantiene un índice de tus archivos y correos para que la búsqueda sea instantánea.",
            "Desactivarlo ahorra disco y CPU en segundo plano.",
            "La búsqueda en el menú Inicio, el Explorador y Outlook se vuelve mucho más lenta.",
            false,
            vec![svc("WSearch", "disabled", "delayed-auto")],
            |h| {
                if h.is_hdd() && h.low_end() {
                    r(Rec::Opcional, "Disco mecánico en equipo modesto: puede ayudar si casi no usas la búsqueda.")
                } else {
                    r(Rec::NoRecomendado, "En tu equipo el indexador apenas afecta; mejor conservar la búsqueda rápida.")
                }
            },
        ),
        t(
            "svc_xbox", "servicios", "Servicios de Xbox",
            "Servicios de inicio de sesión y partidas guardadas de Xbox Live.",
            "Menos servicios cargados.",
            "Juegos de Game Pass o Microsoft Store pueden no iniciar ni guardar en la nube. Ya están en modo manual, \
             así que casi no consumen si no los usas.",
            false,
            vec![
                svc("XblAuthManager", "disabled", "demand"),
                svc("XblGameSave", "disabled", "demand"),
                svc("XboxNetApiSvc", "disabled", "demand"),
            ],
            |_| r(Rec::Opcional, "Solo si NO usas la app Xbox, Game Pass ni juegos de Microsoft Store."),
        ),
        t(
            "svc_spooler", "servicios", "Cola de impresión",
            "Administra los trabajos de impresión.",
            "Un servicio menos y mayor seguridad (ha tenido vulnerabilidades conocidas).",
            "No podrás imprimir ni usar «Microsoft Print to PDF».",
            false,
            vec![svc("Spooler", "disabled", "auto")],
            |_| r(Rec::Opcional, "Solo si no tienes impresora ni imprimes a PDF."),
        ),
        t(
            "svc_fax", "servicios", "Fax",
            "Permite enviar y recibir faxes con un módem.",
            "Un servicio menos.",
            "Ninguna para la mayoría.",
            false,
            vec![svc("Fax", "disabled", "demand")],
            |_| r(Rec::Recomendado, "Casi nadie usa fax hoy en día."),
        ),
        t(
            "svc_maps", "servicios", "Administrador de mapas descargados",
            "Descarga y actualiza mapas sin conexión para la app Mapas de Windows.",
            "Evita descargas en segundo plano.",
            "La app Mapas no tendrá mapas sin conexión.",
            false,
            vec![svc("MapsBroker", "disabled", "delayed-auto")],
            |_| r(Rec::Recomendado, "La app Mapas de Windows casi no se usa."),
        ),
        t(
            "svc_retaildemo", "servicios", "Modo demostración de tienda",
            "Servicio para las PCs de exhibición en tiendas.",
            "Un servicio menos.",
            "Ninguna.",
            false,
            vec![svc("RetailDemo", "disabled", "demand")],
            |_| r(Rec::Recomendado, "No se usa en una PC personal."),
        ),
        t(
            "svc_wer", "servicios", "Informe de errores de Windows",
            "Envía a Microsoft un reporte cuando un programa se cierra inesperadamente.",
            "Ahorra un poco de disco y red cuando algo falla.",
            "Microsoft no recibe tus reportes de errores para corregirlos.",
            false,
            vec![svc("WerSvc", "disabled", "demand")],
            |_| r(Rec::Opcional, "Impacto pequeño; cuestión de preferencia."),
        ),
        // ================================ NAVEGADORES ================================
        t(
            "chrome_background", "navegador", "Chrome: no seguir abierto al cerrarlo",
            "Desactiva «Seguir ejecutando aplicaciones en segundo plano al cerrar Google Chrome».",
            "Al cerrar Chrome se cierra de verdad y libera su RAM.",
            BROWSER_POLICY_NOTE,
            false,
            vec![reg(LM, r"SOFTWARE\Policies\Google\Chrome", "BackgroundModeEnabled", Want::D(0), None)],
            |h| {
                if h.has_browser("chrome") {
                    r(Rec::Recomendado, "Tienes Chrome instalado.")
                } else {
                    r(Rec::NoAplica, "Chrome no está instalado.")
                }
            },
        ),
        t(
            "chrome_memsaver", "navegador", "Chrome: Ahorro de memoria siempre activo",
            "Chrome «duerme» las pestañas que no usas para liberar memoria. Al volver a ellas se recargan.",
            "Mucha menos RAM usada cuando tienes muchas pestañas abiertas.",
            BROWSER_POLICY_NOTE,
            false,
            vec![reg(LM, r"SOFTWARE\Policies\Google\Chrome", "HighEfficiencyModeEnabled", Want::D(1), None)],
            |h| {
                if !h.has_browser("chrome") {
                    r(Rec::NoAplica, "Chrome no está instalado.")
                } else if h.ram_gb() <= 16.5 {
                    r(Rec::Recomendado, "Con 16 GB de RAM o menos ayuda bastante.")
                } else {
                    r(Rec::Opcional, "Tienes mucha RAM; útil solo si abres decenas de pestañas.")
                }
            },
        ),
        t(
            "edge_boost", "navegador", "Edge: no precargar al iniciar Windows",
            "Desactiva el «Aumento de inicio» y el modo en segundo plano de Microsoft Edge, que lo dejan cargado \
             aunque no lo uses.",
            "Libera RAM y hace que Windows inicie un poco más rápido.",
            BROWSER_POLICY_NOTE,
            false,
            vec![
                reg(LM, r"SOFTWARE\Policies\Microsoft\Edge", "StartupBoostEnabled", Want::D(0), None),
                reg(LM, r"SOFTWARE\Policies\Microsoft\Edge", "BackgroundModeEnabled", Want::D(0), None),
            ],
            |_| r(Rec::Recomendado, "Edge viene instalado en todos los Windows y se precarga aunque uses otro navegador."),
        ),
        t(
            "edge_sleeping", "navegador", "Edge: pestañas en suspensión",
            "Edge suspende las pestañas que no usas para ahorrar memoria y CPU.",
            "Menos consumo con muchas pestañas abiertas.",
            BROWSER_POLICY_NOTE,
            false,
            vec![reg(LM, r"SOFTWARE\Policies\Microsoft\Edge", "SleepingTabsEnabled", Want::D(1), None)],
            |h| {
                if h.ram_gb() <= 16.5 {
                    r(Rec::Recomendado, "Útil si usas Edge y tienes 16 GB de RAM o menos.")
                } else {
                    r(Rec::Opcional, "Útil solo si usas Edge con muchas pestañas.")
                }
            },
        ),
        t(
            "brave_background", "navegador", "Brave: no seguir abierto al cerrarlo",
            "Brave deja de ejecutarse en segundo plano cuando cierras todas sus ventanas.",
            "Libera la RAM que usaba Brave.",
            BROWSER_POLICY_NOTE,
            false,
            vec![reg(LM, r"SOFTWARE\Policies\BraveSoftware\Brave", "BackgroundModeEnabled", Want::D(0), None)],
            |h| {
                if h.has_browser("brave") {
                    r(Rec::Recomendado, "Tienes Brave instalado.")
                } else {
                    r(Rec::NoAplica, "Brave no está instalado.")
                }
            },
        ),
    ]
}

// ---------------------------------------------------------------------------------------------
// Estado actual
// ---------------------------------------------------------------------------------------------

fn service_start(name: &str) -> Option<String> {
    let key = format!(r"SYSTEM\CurrentControlSet\Services\{name}");
    match winreg::get(LM, &key, "Start") {
        Ok(Some(RegValue::Dword(v))) => {
            let delayed = matches!(winreg::get(LM, &key, "DelayedAutostart"), Ok(Some(RegValue::Dword(1))));
            Some(
                match v {
                    0 => "boot",
                    1 => "system",
                    2 if delayed => "delayed-auto",
                    2 => "auto",
                    4 => "disabled",
                    _ => "demand",
                }
                .to_string(),
            )
        }
        _ => None,
    }
}

/// Estado de todas las tareas programadas del catálogo, en una sola llamada a PowerShell.
fn task_states(tweaks: &[Tweak]) -> HashMap<String, String> {
    let paths: Vec<&str> = tweaks
        .iter()
        .flat_map(|t| t.steps.iter())
        .filter_map(|s| if let Step::Task { path } = s { Some(*path) } else { None })
        .collect();
    if paths.is_empty() {
        return HashMap::new();
    }
    let list = paths.iter().map(|p| format!("'{}'", p.replace('\'', "''"))).collect::<Vec<_>>().join(",");
    let script = format!(
        "$o = @{{}}\n\
         foreach ($t in @({list})) {{\n\
           $i = $t.LastIndexOf('\\'); $p = $t.Substring(0, $i + 1); $n = $t.Substring($i + 1)\n\
           $x = Get-ScheduledTask -TaskPath $p -TaskName $n -ErrorAction SilentlyContinue\n\
           if ($x) {{ $o[$t] = [string]$x.State }}\n\
         }}\n\
         $o | ConvertTo-Json -Compress"
    );
    sysutil::powershell(&script)
        .ok()
        .and_then(|s| serde_json::from_str::<HashMap<String, String>>(s.trim()).ok())
        .unwrap_or_default()
}

fn step_state(step: &Step, tasks: &HashMap<String, String>) -> Option<bool> {
    match step {
        Step::Reg { hive, key, name, on, .. } | Step::Check { hive, key, name, on } => {
            Some(matches!(winreg::get(*hive, key, name), Ok(Some(v)) if on.matches(&v)))
        }
        Step::Service { name, on, .. } => service_start(name).map(|s| s == *on),
        Step::Task { path } => tasks.get(*path).map(|s| s.eq_ignore_ascii_case("Disabled")),
        Step::Cmd { .. } => None,
    }
}

fn status_of(t: &Tweak, tasks: &HashMap<String, String>) -> String {
    let states: Vec<bool> = t.steps.iter().filter_map(|s| step_state(s, tasks)).collect();
    if states.is_empty() {
        "na".into()
    } else if states.iter().all(|&b| b) {
        "on".into()
    } else if states.iter().any(|&b| b) {
        "partial".into()
    } else {
        "off".into()
    }
}

fn info(t: &Tweak, hw: &Hw, tasks: &HashMap<String, String>) -> TweakInfo {
    let (rec, reason) = (t.rec)(hw);
    TweakInfo {
        id: t.id.into(),
        cat: t.cat.into(),
        title: t.title.into(),
        what: t.what.into(),
        pros: t.pros.into(),
        cons: t.cons.into(),
        admin: t.admin(),
        reboot: t.reboot,
        rec,
        reason,
        status: status_of(t, tasks),
    }
}

pub fn list(hw: &Hw) -> Vec<TweakInfo> {
    let cat = catalog();
    let tasks = task_states(&cat);
    cat.iter().map(|t| info(t, hw, &tasks)).collect()
}

// ---------------------------------------------------------------------------------------------
// Respaldo
// ---------------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "k")]
enum Prev {
    Reg { hive: Hive, key: String, name: String, value: Option<RegValue> },
    Service { name: String, start: String },
}

#[derive(Serialize, Deserialize, Default)]
struct Backup {
    #[serde(default)]
    tweaks: HashMap<String, Vec<Prev>>,
}

static LOCK: Mutex<()> = Mutex::new(());

fn backup_path() -> std::path::PathBuf {
    let base = sysutil::env_path("LOCALAPPDATA").unwrap_or_else(std::env::temp_dir);
    base.join("Optimizador").join("respaldo.json")
}

fn load_backup() -> Backup {
    std::fs::read_to_string(backup_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_backup(b: &Backup) -> Result<(), String> {
    let path = backup_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(b).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------------------------
// Aplicar y deshacer
// ---------------------------------------------------------------------------------------------

fn set_service(name: &str, start: &str) -> Result<(), String> {
    if service_start(name).is_none() {
        return Ok(()); // el servicio no existe en esta versión de Windows
    }
    sysutil::run("sc.exe", &["config", name, "start=", start])?;
    if start == "disabled" {
        let _ = sysutil::run("sc.exe", &["stop", name]);
    }
    Ok(())
}

fn set_task(path: &str, enabled: bool) -> Result<(), String> {
    let i = path.rfind('\\').unwrap_or(0);
    let (p, n) = (&path[..=i], &path[i + 1..]);
    let verb = if enabled { "Enable" } else { "Disable" };
    let script = format!(
        "$x = Get-ScheduledTask -TaskPath '{}' -TaskName '{}' -ErrorAction SilentlyContinue\n\
         if ($x) {{ $x | {verb}-ScheduledTask | Out-Null }}",
        p.replace('\'', "''"),
        n.replace('\'', "''")
    );
    sysutil::powershell(&script).map(|_| ())
}

fn find(id: &str) -> Result<Tweak, String> {
    catalog().into_iter().find(|t| t.id == id).ok_or_else(|| format!("Ajuste desconocido: {id}"))
}

pub fn apply(id: &str, hw: &Hw) -> Result<TweakInfo, String> {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tw = find(id)?;
    if tw.admin() && !sysutil::is_admin() {
        return Err("Este ajuste necesita permisos de administrador. Usa «Reiniciar como administrador».".into());
    }

    // Guardamos los valores originales solo la primera vez.
    let mut backup = load_backup();
    if !backup.tweaks.contains_key(id) {
        let mut prev = Vec::new();
        for s in &tw.steps {
            match s {
                Step::Reg { hive, key, name, .. } => prev.push(Prev::Reg {
                    hive: *hive,
                    key: key.to_string(),
                    name: name.to_string(),
                    value: winreg::get(*hive, key, name).ok().flatten(),
                }),
                Step::Service { name, .. } => {
                    if let Some(start) = service_start(name) {
                        prev.push(Prev::Service { name: name.to_string(), start });
                    }
                }
                _ => {}
            }
        }
        backup.tweaks.insert(id.to_string(), prev);
        save_backup(&backup)?;
    }

    for s in &tw.steps {
        match s {
            Step::Reg { hive, key, name, on, .. } => winreg::set(*hive, key, name, &on.value())?,
            Step::Service { name, on, .. } => set_service(name, on)?,
            Step::Task { path } => set_task(path, false)?,
            Step::Cmd { on, .. } => {
                sysutil::run(on[0], &on[1..])?;
            }
            Step::Check { .. } => {}
        }
    }
    let tasks = task_states(std::slice::from_ref(&tw));
    Ok(info(&tw, hw, &tasks))
}

pub fn revert(id: &str, hw: &Hw) -> Result<TweakInfo, String> {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tw = find(id)?;
    if tw.admin() && !sysutil::is_admin() {
        return Err("Deshacer este ajuste necesita permisos de administrador. Usa «Reiniciar como administrador».".into());
    }
    let mut backup = load_backup();
    let prev = backup.tweaks.get(id).cloned().unwrap_or_default();

    for s in &tw.steps {
        match s {
            Step::Reg { hive, key, name, off, .. } => {
                let saved = prev.iter().find_map(|p| match p {
                    Prev::Reg { hive: h, key: k, name: n, value } if h == hive && k == key && n == name => Some(value.clone()),
                    _ => None,
                });
                let target: Option<RegValue> = match saved {
                    Some(v) => v,
                    None => off.map(|w| w.value()),
                };
                winreg::put(*hive, key, name, target.as_ref())?;
            }
            Step::Service { name, off, .. } => {
                let saved = prev.iter().find_map(|p| match p {
                    Prev::Service { name: n, start } if n == name => Some(start.clone()),
                    _ => None,
                });
                let start = saved.unwrap_or_else(|| off.to_string());
                if start != "boot" && start != "system" {
                    set_service(name, &start)?;
                }
            }
            Step::Task { path } => set_task(path, true)?,
            Step::Cmd { off, .. } => {
                sysutil::run(off[0], &off[1..])?;
            }
            Step::Check { .. } => {}
        }
    }
    backup.tweaks.remove(id);
    save_backup(&backup)?;
    let tasks = task_states(std::slice::from_ref(&tw));
    Ok(info(&tw, hw, &tasks))
}

/// Crea un punto de restauración del sistema (necesita admin).
pub fn create_restore_point() -> Result<(), String> {
    if !sysutil::is_admin() {
        return Err("Crear un punto de restauración necesita permisos de administrador.".into());
    }
    let script = r#"
$k = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\SystemRestore'
Enable-ComputerRestore -Drive "$env:SystemDrive\" -ErrorAction SilentlyContinue
New-ItemProperty -Path $k -Name SystemRestorePointCreationFrequency -Value 0 -PropertyType DWord -Force | Out-Null
try {
  Checkpoint-Computer -Description 'Optimizador - antes de cambios' -RestorePointType MODIFY_SETTINGS
} finally {
  Remove-ItemProperty -Path $k -Name SystemRestorePointCreationFrequency -ErrorAction SilentlyContinue
}
'OK'
"#;
    sysutil::powershell(script).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogo_valido() {
        let cat = catalog();
        let mut ids = std::collections::HashSet::new();
        for t in &cat {
            assert!(ids.insert(t.id), "id repetido: {}", t.id);
            assert!(!t.steps.is_empty());
            assert!(["gaming", "sistema", "privacidad", "servicios", "navegador"].contains(&t.cat));
        }
        let hw = Hw { ram: 8 << 30, cores: 4, system_disk: "SSD".into(), ..Default::default() };
        for t in &cat {
            let (_, why) = (t.rec)(&hw);
            assert!(!why.is_empty());
        }
        assert!(Want::S("100").matches(&RegValue::Sz("100".into())));
        assert!(Want::B(&[1, 2]).matches(&RegValue::Binary(vec![1, 2, 3])));
        assert!(!Want::D(1).matches(&RegValue::Dword(0)));
    }
}
