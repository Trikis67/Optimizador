// Algoritmo "squarified treemap": reparte un rectángulo en rectángulos
// proporcionales al tamaño de cada elemento, intentando que queden lo más cuadrados posible.

function worst(row, side) {
  let sum = 0, max = -Infinity, min = Infinity;
  for (const r of row) {
    sum += r.area;
    if (r.area > max) max = r.area;
    if (r.area < min) min = r.area;
  }
  const s2 = sum * sum, side2 = side * side;
  return Math.max((side2 * max) / s2, s2 / (side2 * min));
}

function layoutRow(row, rect, out) {
  const sum = row.reduce((a, r) => a + r.area, 0);
  if (rect.w >= rect.h) {
    // columna a la izquierda
    const colW = sum / rect.h;
    let y = rect.y;
    for (const r of row) {
      const h = r.area / colW;
      out.push({ item: r.item, x: rect.x, y, w: colW, h });
      y += h;
    }
    return { x: rect.x + colW, y: rect.y, w: rect.w - colW, h: rect.h };
  }
  // fila arriba
  const rowH = sum / rect.w;
  let x = rect.x;
  for (const r of row) {
    const w = r.area / rowH;
    out.push({ item: r.item, x, y: rect.y, w, h: rowH });
    x += w;
  }
  return { x: rect.x, y: rect.y + rowH, w: rect.w, h: rect.h - rowH };
}

/**
 * @param {Array<{size:number}>} items ordenados de mayor a menor
 * @param {{x:number,y:number,w:number,h:number}} rect
 * @returns {Array<{item:any,x:number,y:number,w:number,h:number}>}
 */
export function squarify(items, rect) {
  const out = [];
  const valid = items.filter((i) => i.size > 0);
  const total = valid.reduce((a, i) => a + i.size, 0);
  if (!total || rect.w <= 0 || rect.h <= 0) return out;

  const scale = (rect.w * rect.h) / total;
  const areas = valid.map((item) => ({ item, area: item.size * scale }));
  let row = [];
  let r = { ...rect };
  let i = 0;
  while (i < areas.length) {
    const side = Math.min(r.w, r.h);
    const next = areas[i];
    if (row.length === 0 || worst([...row, next], side) <= worst(row, side)) {
      row.push(next);
      i++;
    } else {
      r = layoutRow(row, r, out);
      row = [];
    }
  }
  if (row.length) layoutRow(row, r, out);
  return out;
}
