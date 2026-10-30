// Optimización: ajustes de gaming, sistema, privacidad y servicios.

import { $, invoke, esc, fmtSize, toast, withBusy, adminNote, loading, helpBtn } from "../util.js";
import { loadTweaks, renderTweaks, applyRecommended } from "../tweakpanel.js";

const TABS = [
  { id: "gaming", label: "Gaming", desc: "Ajustes para que los juegos tengan prioridad y menos interrupciones." },
  { id: "sistema", label: "Sistema", desc: "Hacen que Windows se sienta más rápido y use menos recursos." },
  { id: "privacidad", label: "Privacidad", desc: "Menos datos enviados a Microsoft y menos tareas en segundo plano." },
  { id: "servicios", label: "Servicios", desc: "Servicios de Windows que puedes desactivar si no los usas. Revisa el «?» de cada uno." },
];
let tab = "gaming";

export function init(el) {
  el.innerHTML = `
    <header class="view-head">
      <div>
        <h1>Optimización</h1>
        <p class="muted">Todo es a tu elección. Pulsa <b>?</b> en cada ajuste para saber qué hace y si conviene en tu PC.</p>
      </div>
      <div class="head-actions">
        <button class="btn ghost" id="restore-btn">Crear punto de restauración</button>
        <button class="btn" id="rec-btn">Aplicar recomendados</button>
      </div>
    </header>

    <div class="card hw-card has-help" id="hw-card">${loading("Analizando tu equipo…")}</div>
    <div id="tw-admin"></div>

    <div class="tabs" id="tw-tabs">
      ${TABS.map((t) => `<button class="tab ${t.id === tab ? "active" : ""}" data-tab="${t.id}">${t.label}<span class="tab-count" id="cnt-${t.id}"></span></button>`).join("")}
    </div>
    <p class="muted tab-desc" id="tw-desc"></p>
    <div class="tweak-grid" id="tw-list">${loading("Revisando el estado de cada ajuste…")}</div>`;

  $("tw-admin").innerHTML = adminNote("Los ajustes marcados como «Admin» necesitan permisos de administrador.");
  $("tw-tabs").addEventListener("click", (e) => {
    const b = e.target.closest(".tab");
    if (!b) return;
    tab = b.dataset.tab;
    $("tw-tabs").querySelectorAll(".tab").forEach((x) => x.classList.toggle("active", x === b));
    draw();
  });
  $("rec-btn").addEventListener("click", () => applyRecommended(TABS.map((t) => t.id), draw));
  $("restore-btn").addEventListener("click", (e) =>
    withBusy(e.currentTarget, async () => {
      try {
        await invoke("create_restore_point");
        toast("Punto de restauración creado ✓");
      } catch (err) {
        toast(String(err), true);
      }
    }, "Creando…")
  );
  loadHardware();
  loadTweaks()
    .then(draw)
    .catch((e) => ($("tw-list").innerHTML = `<p class="msg">${esc(String(e))}</p>`));
}

export function show() {
  if (document.querySelector("#tw-list .tweak")) draw();
}

async function draw() {
  const list = await loadTweaks();
  $("tw-desc").textContent = TABS.find((t) => t.id === tab).desc;
  for (const t of TABS) {
    const pending = list.filter((x) => x.cat === t.id && x.rec === "recomendado" && x.status !== "on").length;
    $(`cnt-${t.id}`).textContent = pending ? pending : "";
  }
  renderTweaks($("tw-list"), [tab], list);
}

async function loadHardware() {
  try {
    const h = await invoke("hardware");
    const gpu = h.gpus.map((g) => `${esc(g.name)}${g.vram ? ` (${fmtSize(g.vram)})` : ""}`).join("<br>") || "–";
    const disk = h.system_disk === "SSD" ? "SSD" : h.system_disk === "HDD" ? "Disco duro (HDD)" : "Desconocido";
    $("hw-card").innerHTML = `
      <div class="card-title"><h3>Tu equipo</h3>${helpBtn()}</div>
      <div class="help hidden"><p>Las recomendaciones de cada ajuste se calculan con estos datos: cantidad de RAM,
      tipo de disco, tarjeta gráfica y si es laptop o escritorio.</p></div>
      <div class="hw-grid">
        <div><span class="muted">Procesador</span><strong>${esc(h.cpu)}</strong><small class="muted">${h.cores} hilos</small></div>
        <div><span class="muted">Tarjeta gráfica</span><strong>${gpu}</strong></div>
        <div><span class="muted">RAM</span><strong>${fmtSize(h.ram)}</strong></div>
        <div><span class="muted">Disco del sistema</span><strong>${disk}</strong></div>
        <div><span class="muted">Tipo</span><strong>${h.laptop ? "Laptop" : "PC de escritorio"}</strong></div>
        <div><span class="muted">Sistema</span><strong>${esc(h.os)}</strong><small class="muted">Compilación ${h.build}</small></div>
      </div>`;
  } catch (e) {
    $("hw-card").innerHTML = `<p class="msg">No se pudo leer el hardware: ${esc(String(e))}</p>`;
  }
}

