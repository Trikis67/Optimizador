// Planes de energía.

import { $, invoke, esc, toast, helpBtn } from "../util.js";

export function init(el) {
  el.innerHTML = `
    <header class="view-head">
      <div>
        <h1>Plan de energía</h1>
        <p class="muted">Cambia cómo Windows administra el procesador y la energía.</p>
      </div>
    </header>

    <div class="grid-2">
      <div class="card power-card highlight has-help">
        <div class="card-title"><h3>Máximo rendimiento</h3>${helpBtn()}</div>
        <p class="muted">El procesador no baja su velocidad y se quitan las micro-pausas de ahorro. Ideal para juegos y edición.</p>
        <div class="help hidden">
          <p><b>Qué hace:</b> activa el plan oculto «Máximo rendimiento» (Ultimate Performance) de Windows.</p>
          <p><b>Ventaja:</b> menos latencia y la CPU responde al instante, sin tiempos de «despertar».</p>
          <p><b>Ten en cuenta:</b> en laptops gasta más batería y calienta más. En PCs de escritorio el consumo extra es pequeño.</p>
        </div>
        <button id="ultimate-btn" class="btn">Activar</button>
      </div>
      <div class="card power-card has-help">
        <div class="card-title"><h3>Equilibrado</h3>${helpBtn()}</div>
        <p class="muted">El plan recomendado por Windows. Úsalo para volver a la normalidad.</p>
        <div class="help hidden">
          <p><b>Qué hace:</b> la CPU sube su velocidad cuando hace falta y la baja cuando no.</p>
          <p><b>Para tu PC:</b> es la mejor opción en laptops que usan batería.</p>
        </div>
        <button id="balanced-btn" class="btn ghost">Restaurar</button>
      </div>
    </div>

    <h2>Planes instalados</h2>
    <div id="plan-list" class="list"></div>
    <p id="power-msg" class="msg"></p>`;

  $("ultimate-btn").addEventListener("click", () => action("enable_ultimate_plan", {}, "Máximo rendimiento activado"));
  $("balanced-btn").addEventListener("click", () => action("restore_balanced_plan", {}, "Plan Equilibrado restaurado"));
  $("plan-list").addEventListener("click", (e) => {
    const b = e.target.closest("button[data-guid]");
    if (b) action("set_power_plan", { guid: b.dataset.guid }, "Plan cambiado");
  });
}

export function show() {
  load();
}

async function load() {
  try {
    const plans = await invoke("get_power_plans");
    $("plan-list").innerHTML = plans
      .map((p) => `
        <div class="row plan ${p.active ? "active" : ""}">
          <div class="row-main">
            <span class="name">${esc(p.name)}</span>
            ${p.active ? `<span class="badge ok">Activo</span>` : `<button class="btn small ghost" data-guid="${esc(p.guid)}">Usar este</button>`}
          </div>
        </div>`)
      .join("");
  } catch (e) {
    $("plan-list").innerHTML = "";
    $("power-msg").textContent = String(e);
  }
}

async function action(cmd, args, okMsg) {
  try {
    await invoke(cmd, args);
    toast(okMsg);
    $("power-msg").textContent = "";
  } catch (e) {
    $("power-msg").textContent = String(e);
    toast("No se pudo cambiar el plan", true);
  }
  load();
}
