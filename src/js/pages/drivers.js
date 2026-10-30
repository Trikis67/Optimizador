// Drivers: driver de video y dispositivos con problemas.

import { $, invoke, esc, fmtSize, helpBtn, loading, openTarget } from "../util.js";

export function init(el) {
  el.innerHTML = `
    <header class="view-head">
      <div>
        <h1>Drivers</h1>
        <p class="muted">Revisa tus drivers y encuentra dónde actualizarlos de forma segura.</p>
      </div>
      <div class="head-actions">
        <button class="btn ghost" data-open="devmgmt.msc">Administrador de dispositivos</button>
        <button class="btn" data-open="ms-settings:windowsupdate-optionalupdates">Drivers en Windows Update</button>
      </div>
    </header>

    <div class="section-head has-help">
      <h2>Tarjeta gráfica</h2>${helpBtn()}
      <div class="help hidden">
        <p><b>Por qué importa:</b> el driver de video es el que más influye en el rendimiento de los juegos. Los nuevos
        traen optimizaciones para juegos recientes y corrigen errores.</p>
        <p><b>Cómo actualizar:</b> usa la app oficial del fabricante (NVIDIA App, AMD Adrenalin o Intel DSA). Evita
        programas de «actualizar todos los drivers» de terceros: suelen instalar versiones incorrectas o publicidad.</p>
        <p><b>Instalación limpia:</b> si tienes problemas después de actualizar, usa DDU (en Herramientas) para borrar el
        driver anterior y vuelve a instalar.</p>
      </div>
    </div>
    <div id="gpu-list" class="grid-2">${loading("Leyendo drivers…")}</div>

    <div class="section-head has-help">
      <h2>Dispositivos con problemas</h2>${helpBtn()}
      <div class="help hidden">
        <p>Dispositivos que Windows marca con error (el triángulo amarillo del Administrador de dispositivos). El caso
        más común es «No tiene driver instalado»: búscalo en «Drivers en Windows Update» o en la página del fabricante
        de tu PC o placa madre.</p>
      </div>
    </div>
    <div class="list" id="dev-list">${loading()}</div>

    <div class="note">
      <b>Otros drivers:</b> para chipset, audio, red y Bluetooth, lo más seguro es descargarlos de la página de soporte del
      fabricante de tu laptop o de tu placa madre, o desde «Drivers en Windows Update».
    </div>`;

  el.querySelectorAll("[data-open]").forEach((b) => b.addEventListener("click", () => openTarget(b.dataset.open)));
  $("gpu-list").addEventListener("click", (e) => {
    const b = e.target.closest("button[data-url]");
    if (b) openTarget(b.dataset.url);
  });
  load();
}

async function load() {
  try {
    const d = await invoke("drivers_info");
    $("gpu-list").innerHTML =
      d.gpus
        .map((g) => {
          const old = g.age_days != null && g.age_days > 180;
          const age =
            g.age_days == null ? "" : g.age_days < 60 ? `hace ${g.age_days} días` : `hace ${Math.round(g.age_days / 30)} meses`;
          return `
          <div class="card gpu-card">
            <h3>${esc(g.name)}</h3>
            <div class="kv"><span class="muted">Versión del driver</span><strong>${esc(g.version || "–")}</strong></div>
            <div class="kv"><span class="muted">Fecha</span><strong>${esc(g.date || "–")} ${age ? `<span class="muted">(${age})</span>` : ""}</strong></div>
            ${g.vram ? `<div class="kv"><span class="muted">Memoria de video</span><strong>${fmtSize(g.vram)}</strong></div>` : ""}
            ${old ? `<div class="note warn">Tu driver tiene más de 6 meses. Probablemente hay una versión más nueva.</div>` : ""}
            ${g.url ? `<button class="btn ${old ? "" : "ghost"}" data-url="${esc(g.url)}">Descargar ${esc(g.tool)}</button>` : ""}
          </div>`;
        })
        .join("") || `<p class="muted">No se detectó la tarjeta gráfica.</p>`;

    $("dev-list").innerHTML = d.problems.length
      ? d.problems
          .map((p) => `
          <div class="row">
            <div class="row-main">
              <div class="grow min0">
                <div class="name"><strong>${esc(p.name)}</strong> <span class="muted small">${esc(p.class)}</span></div>
                <div class="muted small">${esc(p.explain)} (código ${p.code})</div>
              </div>
              <span class="badge warn">${esc(p.status)}</span>
            </div>
          </div>`)
          .join("")
      : `<p class="pad ok-text">✓ Todos tus dispositivos funcionan correctamente.</p>`;
  } catch (e) {
    $("gpu-list").innerHTML = `<p class="msg">${esc(String(e))}</p>`;
    $("dev-list").innerHTML = "";
  }
}
