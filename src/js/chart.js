// Gráfica de líneas simple en canvas (tipo Administrador de tareas).

export class LineChart {
  /**
   * @param {HTMLCanvasElement} canvas
   * @param {{max?: number, points?: number, series: {color: string, fill?: boolean}[], grid?: boolean}} opts
   */
  constructor(canvas, opts) {
    this.canvas = canvas;
    this.ctx = canvas.getContext("2d");
    this.max = opts.max ?? null; // null = escala automática
    this.points = opts.points ?? 60;
    this.series = opts.series;
    this.grid = opts.grid ?? true;
    this.data = this.series.map(() => new Array(this.points).fill(0));
  }

  push(...values) {
    values.forEach((v, i) => {
      this.data[i].push(Number(v) || 0);
      if (this.data[i].length > this.points) this.data[i].shift();
    });
    this.draw();
  }

  draw() {
    const { canvas, ctx } = this;
    const dpr = window.devicePixelRatio || 1;
    const w = canvas.clientWidth;
    const h = canvas.clientHeight;
    if (!w || !h) return;
    if (canvas.width !== Math.round(w * dpr) || canvas.height !== Math.round(h * dpr)) {
      canvas.width = Math.round(w * dpr);
      canvas.height = Math.round(h * dpr);
    }
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);

    const peak = Math.max(1, ...this.data.flat());
    const max = this.max ?? niceMax(peak);
    this.currentMax = max;

    if (this.grid) {
      ctx.strokeStyle = "rgba(255,255,255,0.06)";
      ctx.lineWidth = 1;
      for (let i = 1; i < 4; i++) {
        const y = Math.round((h * i) / 4) + 0.5;
        ctx.beginPath();
        ctx.moveTo(0, y);
        ctx.lineTo(w, y);
        ctx.stroke();
      }
    }

    const step = w / (this.points - 1);
    this.series.forEach((s, si) => {
      const d = this.data[si];
      ctx.beginPath();
      d.forEach((v, i) => {
        const x = i * step;
        const y = h - (Math.min(v, max) / max) * (h - 2) - 1;
        i ? ctx.lineTo(x, y) : ctx.moveTo(x, y);
      });
      ctx.strokeStyle = s.color;
      ctx.lineWidth = 1.5;
      ctx.lineJoin = "round";
      ctx.stroke();
      if (s.fill !== false) {
        ctx.lineTo(w, h);
        ctx.lineTo(0, h);
        ctx.closePath();
        const g = ctx.createLinearGradient(0, 0, 0, h);
        g.addColorStop(0, s.color + "55");
        g.addColorStop(1, s.color + "05");
        ctx.fillStyle = g;
        ctx.fill();
      }
    });
  }
}

function niceMax(v) {
  const p = Math.pow(10, Math.floor(Math.log10(v)));
  for (const m of [1, 2, 5, 10]) if (v <= m * p) return m * p;
  return 10 * p;
}
