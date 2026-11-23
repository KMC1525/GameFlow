/* E3D — mini-engine 3D por software no Canvas 2D (sem WebGL, sem bibliotecas).
 * Mundo: x = direita, y = cima, z = frente. Câmera: E3D.cam = { x, y, z, yaw, pitch, fov }.
 * Uso por quadro:  E3D.begin(ctx);  E3D.sky(...);  E3D.ground(...);  E3D.box(...); E3D.sprite(...);  E3D.flush(); */
(() => {
  'use strict';
  const E3D = (window.E3D = { cam: { x: 0, y: 2, z: 0, yaw: 0, pitch: 0, fov: 70 }, near: 0.15, fogDist: 60, fog: [20, 24, 36], light: [0.55, 0.8, 0.65] });
  let ctx, W, H, F, cy, sy_, cp, sp, list = [], pre = [];
  const cache = {};
  const rgb = (c) => {
    if (Array.isArray(c)) return c;
    if (cache[c]) return cache[c];
    let h = String(c).replace('#', ''); if (h.length === 3) h = h.replace(/./g, (m) => m + m);
    return (cache[c] = [parseInt(h.slice(0, 2), 16) || 0, parseInt(h.slice(2, 4), 16) || 0, parseInt(h.slice(4, 6), 16) || 0]);
  };
  E3D.rgb = rgb;
  const mix = (a, b, t) => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
  const css = (c, k = 1) => `rgb(${Math.round(c[0] * k)},${Math.round(c[1] * k)},${Math.round(c[2] * k)})`;

  E3D.begin = (c) => {
    ctx = c; W = c.canvas.width; H = c.canvas.height;
    const cam = E3D.cam; F = H / 2 / Math.tan((cam.fov * Math.PI) / 360);
    cy = Math.cos(cam.yaw); sy_ = Math.sin(cam.yaw); cp = Math.cos(cam.pitch); sp = Math.sin(cam.pitch);
    list = []; pre = [];
  };
  /** mundo -> espaço da câmera [x, y, z] (z = profundidade) */
  const toCam = (x, y, z) => {
    const c = E3D.cam, px = x - c.x, py = y - c.y, pz = z - c.z;
    const x1 = px * cy - pz * sy_, z1 = px * sy_ + pz * cy;
    return [x1, py * cp - z1 * sp, py * sp + z1 * cp];
  };
  const scr = (p) => [W / 2 + (p[0] / p[2]) * F, H / 2 - (p[1] / p[2]) * F];
  E3D.project = (x, y, z) => { const p = toCam(x, y, z); if (p[2] < E3D.near) return null; const s = scr(p); return [s[0], s[1], p[2]]; };
  E3D.scale = (depth) => F / depth;

  const clipNear = (pts) => {
    const out = [], n = E3D.near;
    for (let i = 0; i < pts.length; i++) {
      const a = pts[i], b = pts[(i + 1) % pts.length], ain = a[2] >= n, bin = b[2] >= n;
      if (ain) out.push(a);
      if (ain !== bin) { const t = (n - a[2]) / (b[2] - a[2]); out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, n]); }
    }
    return out;
  };
  const fogK = (d) => Math.min(1, Math.max(0, (d - E3D.fogDist * 0.35) / (E3D.fogDist * 0.65)));

  function poly(pts3, color, shade, stroke) {
    const pts = clipNear(pts3); if (pts.length < 3) return;
    const base = mix(rgb(color).map((v) => v * shade), E3D.fog, fogK(pts.reduce((s, p) => s + p[2], 0) / pts.length));
    ctx.fillStyle = css(base);
    ctx.beginPath();
    pts.forEach((p, i) => { const s = scr(p); i ? ctx.lineTo(s[0], s[1]) : ctx.moveTo(s[0], s[1]); });
    ctx.closePath(); ctx.fill();
    if (stroke) { ctx.strokeStyle = css(mix(base, [0, 0, 0], 0.35)); ctx.lineWidth = 1; ctx.stroke(); }
  }

  E3D.sky = (top, bottom) => {
    const horizon = H / 2 + (Math.tan(E3D.cam.pitch) * F);
    const g = ctx.createLinearGradient(0, 0, 0, Math.max(40, horizon));
    g.addColorStop(0, css(rgb(top))); g.addColorStop(1, css(rgb(bottom)));
    ctx.fillStyle = g; ctx.fillRect(0, 0, W, H);
    E3D.fog = rgb(bottom);
  };

  /** Chão plano infinito (quadrado grande recortado) + grade opcional. */
  E3D.ground = (y, color, grid, size = 200, step = 4) => {
    const c = E3D.cam, s = size;
    const q = [[-s, -s], [s, -s], [s, s], [-s, s]].map(([dx, dz]) => toCam(Math.round(c.x / step) * step + dx, y, Math.round(c.z / step) * step + dz));
    const pts = clipNear(q); if (pts.length >= 3) {
      ctx.fillStyle = css(mix(rgb(color), E3D.fog, 0.0));
      ctx.beginPath(); pts.forEach((p, i) => { const sc = scr(p); i ? ctx.lineTo(sc[0], sc[1]) : ctx.moveTo(sc[0], sc[1]); }); ctx.closePath(); ctx.fill();
    }
    if (grid) {
      const gx = Math.round(c.x / step) * step, gz = Math.round(c.z / step) * step, n = Math.ceil(E3D.fogDist / step);
      ctx.lineWidth = 1;
      for (let i = -n; i <= n; i++) {
        for (const [a, b] of [[[gx + i * step, gz - n * step], [gx + i * step, gz + n * step]], [[gx - n * step, gz + i * step], [gx + n * step, gz + i * step]]]) {
          const p1 = toCam(a[0], y, a[1]), p2 = toCam(b[0], y, b[1]);
          let A = p1, B = p2; if (A[2] < E3D.near && B[2] < E3D.near) continue;
          if (A[2] < E3D.near) { const t = (E3D.near - A[2]) / (B[2] - A[2]); A = [A[0] + (B[0] - A[0]) * t, A[1] + (B[1] - A[1]) * t, E3D.near]; }
          if (B[2] < E3D.near) { const t = (E3D.near - B[2]) / (A[2] - B[2]); B = [B[0] + (A[0] - B[0]) * t, B[1] + (A[1] - B[1]) * t, E3D.near]; }
          const k = fogK((A[2] + B[2]) / 2); if (k > 0.97) continue;
          const sa = scr(A), sb = scr(B);
          ctx.strokeStyle = css(mix(rgb(grid), E3D.fog, k)); ctx.beginPath(); ctx.moveTo(sa[0], sa[1]); ctx.lineTo(sb[0], sb[1]); ctx.stroke();
        }
      }
    }
  };

  /** Sombra circular no chão (desenhada antes dos objetos). */
  E3D.shadow = (x, y, z, r) => { pre.push([x, y + 0.01, z, r]); };

  /** Caixa: (x,z) = centro, y = base, w (x), h (y), d (z). */
  E3D.box = (x, y, z, w, h, d, color, stroke = true) => {
    const hx = w / 2, hz = d / 2, c = E3D.cam;
    const v = (a, b, cc) => toCam(x + a, y + b, z + cc);
    const P = [v(-hx, 0, -hz), v(hx, 0, -hz), v(hx, 0, hz), v(-hx, 0, hz), v(-hx, h, -hz), v(hx, h, -hz), v(hx, h, hz), v(-hx, h, hz)];
    const L = E3D.light;
    const faces = [
      [[4, 5, 6, 7], [0, 1, 0], 1.0, [x, y + h, z]],          // topo
      [[0, 1, 5, 4], [0, 0, -1], L[0], [x, y + h / 2, z - hz]], // trás (z-)
      [[2, 3, 7, 6], [0, 0, 1], L[1], [x, y + h / 2, z + hz]],  // frente (z+)
      [[3, 0, 4, 7], [-1, 0, 0], L[2], [x - hx, y + h / 2, z]], // esquerda
      [[1, 2, 6, 5], [1, 0, 0], L[0], [x + hx, y + h / 2, z]],  // direita
    ];
    for (const [idx, n, shade, ctr] of faces) {
      if (n[0] * (c.x - ctr[0]) + n[1] * (c.y - ctr[1]) + n[2] * (c.z - ctr[2]) <= 0) continue; // face de costas
      const pts = idx.map((i) => P[i]);
      list.push({ d: toCam(ctr[0], ctr[1], ctr[2])[2], f: () => poly(pts, color, shade, stroke) });
    }
  };

  /** Quadrilátero horizontal (plataformas finas, tapetes). */
  E3D.plane = (x, y, z, w, d, color) => {
    const hx = w / 2, hz = d / 2;
    const pts = [toCam(x - hx, y, z - hz), toCam(x + hx, y, z - hz), toCam(x + hx, y, z + hz), toCam(x - hx, y, z + hz)];
    list.push({ d: toCam(x, y, z)[2], f: () => poly(pts, color, 1, true) });
  };

  /** Sprite sempre voltado para a câmera. (x,y,z) = base ao centro. Sem imagem, usa `fallback`: cor ou função(ctx,x,y,w,h). */
  E3D.sprite = (img, x, y, z, w, h, fallback) => {
    const p = toCam(x, y + h / 2, z); if (p[2] < E3D.near) return;
    list.push({ d: p[2], f: () => {
      const s = F / p[2], sx = W / 2 + p[0] * s, sy = H / 2 - p[1] * s, pw = w * s, ph = h * s;
      if (sx + pw < 0 || sx - pw > W) return;
      const k = fogK(p[2]);
      ctx.save();
      if (img) { ctx.globalAlpha = 1 - k * 0.85; ctx.drawImage(img, sx - pw / 2, sy - ph / 2, pw, ph); }
      else if (typeof fallback === 'function') fallback(ctx, sx, sy, pw, ph);
      else { ctx.fillStyle = css(mix(rgb(fallback || '#ff0'), E3D.fog, k)); ctx.beginPath(); ctx.ellipse(sx, sy, pw / 2, ph / 2, 0, 0, 7); ctx.fill(); }
      ctx.restore();
    } });
  };

  E3D.flush = () => {
    for (const [x, y, z, r] of pre) {
      const pts = []; for (let i = 0; i < 14; i++) { const a = (i / 14) * Math.PI * 2; pts.push(toCam(x + Math.cos(a) * r, y, z + Math.sin(a) * r)); }
      const cl = clipNear(pts); if (cl.length < 3) continue;
      ctx.fillStyle = 'rgba(0,0,0,.28)'; ctx.beginPath(); cl.forEach((p, i) => { const s = scr(p); i ? ctx.lineTo(s[0], s[1]) : ctx.moveTo(s[0], s[1]); }); ctx.closePath(); ctx.fill();
    }
    list.sort((a, b) => b.d - a.d);
    for (const it of list) it.f();
    list = []; pre = [];
  };
})();
