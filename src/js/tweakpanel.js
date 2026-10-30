// Tarjetas de ajustes reutilizables (Optimización y Navegadores).

import { app, invoke, esc, toast, helpBtn, helpPanel, recBadge, switchHtml, confirmDialog } from "./util.js";

let cache = null;
let loadingPromise = null;

/** Carga (una sola vez) la lista de ajustes con su estado y recomendación. */
export function loadTweaks(force = false) {
  if (force) cache = null;
  if (cache) return Promise.resolve(cache);
  if (!loadingPromise) {
    loadingPromise = invoke("tweaks_list")
      .then((list) => (cache = list))
      .finally(() => (loadingPromise = null));
  }
  return loadingPromise;
}

function cardHtml(t) {
  const on = t.status === "on";
  const blocked = t.rec === "no_aplica";
  const partial = t.status === "partial" ? `<span class="badge warn">Parcial</span>` : "";
  return `
    <div class="card tweak has-help ${on ? "is-on" : ""} ${blocked ? "is-na" : ""}" data-id="${esc(t.id)}">
      <div class="tweak-head">
        <div class="tweak-title">
          <h3>${esc(t.title)}</h3>
          <div class="badges">
            ${recBadge(t.rec)}
            ${t.admin ? `<span class="badge admin">Admin</span>` : ""}
            ${t.reboot ? `<span class="badge">Reiniciar</span>` : ""}
            ${partial}
          </div>
        </div>
        ${helpBtn()}
        ${switchHtml(on, { disabled: blocked && !on, attrs: `data-tweak="${esc(t.id)}"` })}
      </div>
      ${helpPanel(t)}
    </div>`;
}

/**
 * Dibuja los ajustes de las categorías indicadas dentro de `root` y maneja los interruptores.
 * @param {HTMLElement} root
 * @param {string[]} cats
 */
export function renderTweaks(root, cats, list) {
  const items = list.filter((t) => cats.includes(t.cat));
  root.innerHTML = items.map(cardHtml).join("") || `<p class="muted">No hay ajustes en esta sección.</p>`;
  if (!root.dataset.bound) {
    root.dataset.bound = "1";
    root.addEventListener("change", (e) => {
      const input = e.target.closest("input[data-tweak]");
      if (input) toggle(input, root, cats);
    });
  }
}

async function toggle(input, root, cats) {
  const id = input.dataset.tweak;
  const t = cache?.find((x) => x.id === id);
  const enabled = input.checked;
  if (t?.admin && !app.admin) {
    input.checked = !enabled;
    toast("Este ajuste necesita permisos de administrador. Usa «Reiniciar como admin» en la barra lateral.", true);
    return;
  }
  if (enabled && t?.rec === "no_recomendado") {
    const ok = await confirmDialog("No recomendado para tu PC", `${t.reason}\n\n¿Quieres aplicarlo de todas formas?`, "Aplicar igual");
    if (!ok) {
      input.checked = false;
      return;
    }
  }
  const card = input.closest(".tweak");
  card.classList.add("busy");
  input.disabled = true;
  try {
    const updated = await invoke("tweak_set", { id, enabled });
    const i = cache.findIndex((x) => x.id === id);
    if (i >= 0) cache[i] = updated;
    card.outerHTML = cardHtml(updated);
    toast(enabled ? `Aplicado: ${updated.title}${updated.reboot ? " (reinicia para que surta efecto)" : ""}` : `Deshecho: ${updated.title}`);
  } catch (e) {
    input.checked = !enabled;
    input.disabled = false;
    card.classList.remove("busy");
    toast(String(e), true);
  }
}

/** Aplica en lote los ajustes recomendados que aún no están activos. */
export async function applyRecommended(cats, onDone) {
  const list = await loadTweaks();
  const todo = list.filter((t) => cats.includes(t.cat) && t.rec === "recomendado" && t.status !== "on" && (!t.admin || app.admin));
  const skipped = list.filter((t) => cats.includes(t.cat) && t.rec === "recomendado" && t.status !== "on" && t.admin && !app.admin);
  if (!todo.length) {
    toast(skipped.length ? "Los recomendados que faltan necesitan permisos de administrador." : "Ya tienes aplicados todos los recomendados ✓");
    return;
  }
  const body =
    `<p>Se aplicarán estos ${todo.length} ajustes:</p><ul class="mini-list">${todo.map((t) => `<li>${esc(t.title)}</li>`).join("")}</ul>` +
    (skipped.length ? `<p class="muted">${skipped.length} más necesitan permisos de administrador y se saltarán.</p>` : "") +
    `<p class="muted">Consejo: crea antes un punto de restauración. Todo se puede deshacer desde aquí mismo.</p>`;
  const ok = await confirmDialog("Aplicar recomendados", body, "Aplicar", { html: true });
  if (!ok) return;
  let done = 0;
  const errors = [];
  for (const t of todo) {
    try {
      const updated = await invoke("tweak_set", { id: t.id, enabled: true });
      const i = cache.findIndex((x) => x.id === t.id);
      if (i >= 0) cache[i] = updated;
      done++;
    } catch (e) {
      errors.push(`${t.title}: ${e}`);
    }
  }
  toast(errors.length ? `Aplicados ${done}. Errores: ${errors.join(" · ")}` : `Listo: ${done} ajustes aplicados. Algunos requieren reiniciar.`, errors.length > 0);
  onDone?.();
}
