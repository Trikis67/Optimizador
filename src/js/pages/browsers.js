// Navegadores: caché y ajustes de rendimiento.

import { $, invoke, esc, fmtSize, fmtNum, toast, helpBtn, withBusy, adminNote, loading } from "../util.js";
import { loadTweaks, renderTweaks, applyRecommended } from "../tweakpanel.js";

export function init(el) {
  el.innerHTML = `
    <header class="view-head">
      <div>
        <h1>Navegadores</h1>
        <p class="muted">Haz que tus navegadores usen menos RAM y libera su caché.</p>
      </div>
      <div class="head-actions">
        <button class="btn" id="br-rec-btn">Aplicar recomendados</button>
      </div>
    </header>

    <div class="section-head has-help">
      <h2>Caché</h2>${helpBtn()}
      <button class="btn small ghost push" id="br-refresh">Volver a medir</button>
      <div class="help hidden">
        <p><b>Qué es:</b> copias de imágenes, scripts y shaders de las páginas que visitas, para cargarlas más rápido.</p>
        <p><b>Cuándo limpiarla:</b> si ocupa varios GB, si una página se ve rota o si el navegador va lento.</p>
        <p><b>Qué NO se borra:</b> historial, contraseñas, cookies (no se cierran tus sesiones), marcadores ni extensiones.</p>
        <p><b>Ten en cuenta:</b> las páginas cargarán un poco más lento la primera vez. El navegador debe estar cerrado.</p>
      </div>
    </div>
    <div class="list" id="br-list">${loading("Midiendo cachés…")}</div>

    <div class="section-head">
      <h2>Ajustes de rendimiento</h2>
    </div>
    <div id="br-admin"></div>
    <div class="tweak-grid" id="br-tweaks">${loading()}</div>

    <div class="note">
      <b>Más consejos:</b> quita extensiones que no uses (cada una consume RAM), mantén el navegador actualizado y deja
      activada la «aceleración por hardware» para que la GPU ayude con videos y páginas pesadas.
    </div>`;

  $("br-admin").innerHTML = adminNote("Estos ajustes se aplican como políticas y necesitan permisos de administrador.");
  $("br-refresh").addEventListener("click", (e) => withBusy(e.currentTarget, loadCaches, "Midiendo…"));
  $("br-rec-btn").addEventListener("click", () => applyRecommended(["navegador"], drawTweaks));
  $("br-list").addEventListener("click", async (e) => {
    const b = e.target.closest("button[data-clean]");
    if (!b) return;
    await withBusy(b, async () => {
      try {
        const freed = await invoke("browser_clean", { id: b.dataset.clean });
        toast(`Caché limpiada: ${fmtSize(freed)} liberados`);
      } catch (err) {
        toast(String(err), true);
      }
    }, "Limpiando…");
    loadCaches();
  });
  loadCaches();
  drawTweaks();
}

async function loadCaches() {
  try {
    const list = await invoke("browsers_list");
    $("br-list").innerHTML =
      list
        .map((b) => `
        <div class="row">
          <div class="row-main">
            <span class="name"><strong>${esc(b.name)}</strong></span>
            ${b.running ? `<span class="badge warn">Abierto</span>` : ""}
            <span class="muted sub">${fmtNum(b.files)} archivos</span>
            <span class="size">${fmtSize(b.cache)}</span>
            <button class="btn small ${b.running ? "ghost" : ""}" data-clean="${esc(b.id)}" ${b.running ? 'title="Ciérralo primero"' : ""}>
              Limpiar caché
            </button>
          </div>
        </div>`)
        .join("") || `<p class="muted pad">No se encontraron navegadores conocidos.</p>`;
  } catch (e) {
    $("br-list").innerHTML = `<p class="msg pad">${esc(String(e))}</p>`;
  }
}

async function drawTweaks() {
  try {
    renderTweaks($("br-tweaks"), ["navegador"], await loadTweaks());
  } catch (e) {
    $("br-tweaks").innerHTML = `<p class="msg">${esc(String(e))}</p>`;
  }
}

export function show() {
  if (document.querySelector("#br-tweaks .tweak")) drawTweaks();
}
