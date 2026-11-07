// Salud del sistema: diagnóstico rápido con puntaje y reparaciones de Windows.

import { $, invoke, esc, toast, helpBtn, loading, withBusy, confirmDialog, adminNote, openTarget } from "../util.js";

const STATUS = { ok: "Bien", warn: "Revisar", bad: "Atención", info: "Info" };

export function init(el) {
  el.innerHTML = `
    <header class="view-head">
      <div>
        <h1>Salud del sistema</h1>
        <p class="muted">Un chequeo de discos, batería, errores y actualizaciones, con qué hacer en cada caso.</p>
      </div>
      <div class="head-actions">
        <button class="btn" id="hl-run">Volver a revisar</button>
      </div>
    </header>
    <div id="hl-admin"></div>

    <div class="health-top">
      <div class="card score-card">
        <div class="gauge" id="hl-gauge"><svg viewBox="0 0 120 70" aria-hidden="true">
          <path d="M10 64 A50 50 0 0 1 110 64" class="gauge-bg" />
          <path d="M10 64 A50 50 0 0 1 110 64" class="gauge-fg" id="hl-arc" pathLength="100" />
        </svg><strong id="hl-score">–</strong></div>
        <div>
          <h3 id="hl-title">Revisando…</h3>
          <p class="muted small" id="hl-sub">Esto tarda unos segundos.</p>
        </div>
      </div>
      <div class="card repair-card has-help">
        <div class="card-title"><h3>Reparar Windows</h3>${helpBtn()}</div>
        <div class="help hidden">
          <p><b>DISM</b> descarga de Windows Update los componentes dañados de Windows. <b>SFC</b> revisa los archivos
          del sistema y reemplaza los dañados. Juntos arreglan muchos cierres raros, errores de actualización y
          pantallazos azules causados por archivos corruptos.</p>
          <p><b>Ten en cuenta:</b> tarda entre 10 y 30 minutos. Se abre una ventana para que veas el progreso; puedes seguir
          usando la PC.</p>
        </div>
        <p class="muted small">Se abre una ventana con el progreso. Puedes seguir usando la PC.</p>
        <div class="link-group">
          <button class="btn small" data-repair="full">Reparación completa (DISM + SFC)</button>
          <button class="btn small ghost" data-repair="chkdsk">Revisar disco</button>
          <button class="btn small ghost hidden" id="hl-battery">Informe de batería</button>
        </div>
      </div>
    </div>

    <div class="section-head"><h2>Resultados</h2></div>
    <div class="list" id="hl-list">${loading("Revisando tu PC…")}</div>`;

  $("hl-admin").innerHTML = adminNote("Sin permisos de administrador algunos datos de los discos no se pueden leer.");
  $("hl-run").addEventListener("click", (e) => withBusy(e.currentTarget, run, "Revisando…"));
  el.querySelectorAll("[data-repair]").forEach((b) => b.addEventListener("click", () => repair(b.dataset.repair)));
  $("hl-battery").addEventListener("click", async () => {
    try {
      await invoke("health_battery_report");
    } catch (e) {
      toast(String(e), true);
    }
  });
  $("hl-list").addEventListener("click", (e) => {
    const b = e.target.closest("[data-open]");
    if (b) openTarget(b.dataset.open);
  });
  run();
}

const LINKS = {
  reboot: ["Windows Update", "ms-settings:windowsupdate"],
  defender: ["Seguridad de Windows", "windowsdefender:"],
  space: ["Sensor de almacenamiento", "ms-settings:storagesense"],
};

async function run() {
  try {
    const r = await invoke("health_report");
    const s = r.score;
    $("hl-score").textContent = s;
    $("hl-arc").style.strokeDasharray = `${s} 100`;
    $("hl-gauge").dataset.level = s >= 85 ? "ok" : s >= 60 ? "warn" : "bad";
    const bad = r.checks.filter((c) => c.status === "bad").length;
    const warn = r.checks.filter((c) => c.status === "warn").length;
    $("hl-title").textContent = s >= 85 ? "Tu PC está en buena forma" : s >= 60 ? "Hay cosas por revisar" : "Tu PC necesita atención";
    $("hl-sub").textContent = `${r.checks.length} revisiones · ${bad} con atención · ${warn} por revisar`;
    $("hl-battery").classList.toggle("hidden", !r.checks.some((c) => c.id === "battery"));

    const order = { bad: 0, warn: 1, info: 2, ok: 3 };
    $("hl-list").innerHTML = [...r.checks]
      .sort((a, b) => order[a.status] - order[b.status])
      .map((c) => {
        const link = c.status !== "ok" && LINKS[c.id];
        return `
        <div class="row health-row">
          <div class="row-main">
            <span class="dot dot-${c.status}"></span>
            <div class="grow min0">
              <div class="name"><strong>${esc(c.title)}</strong></div>
              <div class="muted small">${esc(c.detail)}</div>
              ${c.status !== "ok" && c.fix ? `<div class="small fix">→ ${esc(c.fix)}</div>` : ""}
            </div>
            ${link ? `<button class="btn tiny ghost" data-open="${link[1]}">${link[0]}</button>` : ""}
            <span class="badge st-${c.status}">${STATUS[c.status] || c.status}</span>
          </div>
        </div>`;
      })
      .join("");
  } catch (e) {
    $("hl-title").textContent = "No se pudo revisar";
    $("hl-sub").textContent = "";
    $("hl-list").innerHTML = `<p class="pad msg">${esc(String(e))}</p>`;
  }
}

async function repair(kind) {
  const text = {
    full: "Se abrirá una ventana que ejecuta DISM y luego SFC. Tarda entre 10 y 30 minutos. No apagues la PC mientras trabaja.",
    chkdsk: "Se abrirá una ventana que revisa el disco del sistema en busca de errores (solo lectura, sin reiniciar).",
  }[kind];
  if (!(await confirmDialog("¿Iniciar reparación?", text, "Iniciar"))) return;
  try {
    const title = await invoke("health_repair", { kind });
    toast(`${title}: se abrió la ventana con el progreso`);
  } catch (e) {
    toast(String(e), true);
  }
}
