// Utilidades compartidas por todas las páginas.

export const invoke = (...args) => window.__TAURI__.core.invoke(...args);
export const listen = (...args) => window.__TAURI__.event.listen(...args);
export const $ = (id) => document.getElementById(id);

export const app = { admin: false, dev: false };

export function fmtSize(bytes) {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let i = 0;
  let n = Math.max(0, Number(bytes) || 0);
  while (n >= 1024 && i < units.length - 1) {
    n /= 1024;
    i++;
  }
  return `${n.toFixed(n < 10 && i > 0 ? 1 : 0)} ${units[i]}`;
}

export const fmtRate = (bytesPerSec) => `${fmtSize(bytesPerSec)}/s`;
export const fmtNum = (n) => Number(n || 0).toLocaleString("es-GT");
export const fmtPct = (n, d = 0) => `${Number(n || 0).toFixed(d)}%`;

export function esc(s) {
  return String(s ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
}

let toastTimer;
export function toast(msg, isError = false) {
  const t = $("toast");
  t.textContent = msg;
  t.className = "toast" + (isError ? " error" : "");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => t.classList.add("hidden"), isError ? 6000 : 4000);
}

/** Diálogo de confirmación propio. `body` puede ser texto o HTML seguro. */
export function confirmDialog(title, body, okLabel = "Aceptar", { danger = false, html = false } = {}) {
  return new Promise((resolve) => {
    const back = document.createElement("div");
    back.className = "modal-back";
    back.innerHTML = `
      <div class="modal">
        <h3>${esc(title)}</h3>
        <div class="modal-body">${html ? body : `<p>${esc(body)}</p>`}</div>
        <div class="modal-actions">
          <button class="btn ghost" data-r="0">Cancelar</button>
          <button class="btn ${danger ? "danger" : ""}" data-r="1">${esc(okLabel)}</button>
        </div>
      </div>`;
    back.addEventListener("click", (e) => {
      const r = e.target.dataset?.r;
      if (r !== undefined || e.target === back) {
        back.remove();
        resolve(r === "1");
      }
    });
    document.body.appendChild(back);
  });
}

/** Botón "?" que muestra/oculta el panel de ayuda que esté dentro del mismo `.has-help`. */
export const helpBtn = () => `<button class="help-btn" title="¿Qué es esto?" aria-label="Ayuda">?</button>`;

export function helpPanel({ what, pros, cons, reason, rec }) {
  const rows = [];
  if (what) rows.push(`<p><b>Qué hace:</b> ${esc(what)}</p>`);
  if (pros) rows.push(`<p><b>Ventaja:</b> ${esc(pros)}</p>`);
  if (cons) rows.push(`<p><b>Ten en cuenta:</b> ${esc(cons)}</p>`);
  if (reason) rows.push(`<p class="for-you ${rec || ""}"><b>Para tu PC:</b> ${esc(reason)}</p>`);
  return `<div class="help hidden">${rows.join("")}</div>`;
}

const REC_LABELS = {
  recomendado: "Recomendado",
  opcional: "Opcional",
  no_recomendado: "No recomendado",
  no_aplica: "No aplica",
};
export const recBadge = (rec) => `<span class="badge rec-${rec}">${REC_LABELS[rec] || rec}</span>`;

export function switchHtml(checked, { disabled = false, attrs = "" } = {}) {
  return `<label class="switch ${disabled ? "disabled" : ""}">
    <input type="checkbox" ${checked ? "checked" : ""} ${disabled ? "disabled" : ""} ${attrs} />
    <span class="slider"></span>
  </label>`;
}

export async function openTarget(target) {
  try {
    await invoke("open_target", { target });
  } catch (e) {
    toast(String(e), true);
  }
}

/** Ejecuta `fn` mostrando un estado de carga en el botón. */
export async function withBusy(btn, fn, busyText = "Trabajando…") {
  const old = btn.innerHTML;
  btn.disabled = true;
  btn.innerHTML = `<span class="mini-spin"></span>${esc(busyText)}`;
  try {
    return await fn();
  } finally {
    btn.disabled = false;
    btn.innerHTML = old;
  }
}

export const adminNote = (text = "Algunas opciones necesitan permisos de administrador.") =>
  app.admin
    ? ""
    : `<div class="note warn">${esc(text)} <button class="link-btn" data-action="relaunch-admin">Reiniciar como administrador</button></div>`;

export function loading(text = "Cargando…") {
  return `<div class="loading"><div class="spinner"></div><span>${esc(text)}</span></div>`;
}
