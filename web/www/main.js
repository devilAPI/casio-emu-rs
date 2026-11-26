import init, { Emulator, key_table_json, model_ids } from './pkg/casio_emu_web.js';

const canvas = document.getElementById('calc');
const ctx = canvas.getContext('2d', { alpha: false });
const status = document.getElementById('status');
const romInput = document.getElementById('romfile');
const resetButton = document.getElementById('reset');
const modelSelect = document.getElementById('model');

// Design space of the official 289x582 skin.  Everything below is drawn in
// these coordinates and then scaled once to the device-pixel backing store,
// so the UI stays crisp at any window size or display density.
const DESIGN = { w: 289, h: 582 };
const LCD = { x0: 39, y0: 94, x1: 254, y1: 179 };
const KEYPAD = { x0: 10, y0: 190, x1: 280, y1: 575 };
const GRID_W = 43;
const GRID_H = 28;

// Palette shared with style.css.
const cssVars = getComputedStyle(document.documentElement);
const cssColor = (name, fallback) => cssVars.getPropertyValue(name).trim() || fallback;
const COLORS = {
  lcdBg: cssColor('--lcd-bg', '#cfd6c2'),
  lcdFg: cssColor('--lcd-fg', '#3c4536'),
  device: cssColor('--device', '#d9d0c2'),
  deviceEdge: cssColor('--device-edge', '#b7ac9d'),
  key: cssColor('--key', '#4a4250'),
  bezel: cssColor('--bezel', '#3a323c'),
  accent: cssColor('--accent', '#c9a584'),
  text: cssColor('--text', '#eae2d8'),
  panel: cssColor('--panel', '#2f2832'),
};

let cssW = DESIGN.w;
let cssH = DESIGN.h;

function measureCanvas() {
  const rect = canvas.getBoundingClientRect();
  if (rect.width > 0) cssW = rect.width;
  if (rect.height > 0) cssH = rect.height;
}

function syncCanvas() {
  // Match the backing store to the displayed size in device pixels, then draw
  // in design coordinates.  Capping the DPR avoids enormous canvases on 4K
  // phones without making the UI look soft.
  const dpr = Math.min(window.devicePixelRatio || 1, 3);
  const bw = Math.max(1, Math.round(cssW * dpr));
  const bh = Math.max(1, Math.round(cssH * dpr));
  if (canvas.width !== bw || canvas.height !== bh) {
    canvas.width = bw;
    canvas.height = bh;
  }
  ctx.setTransform(bw / DESIGN.w, 0, 0, bh / DESIGN.h, 0, 0);
  ctx.imageSmoothingEnabled = false;
}

measureCanvas();
if (typeof ResizeObserver !== 'undefined') {
  new ResizeObserver(measureCanvas).observe(canvas);
}
window.addEventListener('resize', measureCanvas);

const skin = new Image();
skin.src = 'skin.png';

let keyTable = [];

let emu = null;
let skinReady = false;
let lcdW = 96;
let lcdH = 32;

function updateLcdSize() {
  if (!emu) return;
  const width = emu.lcdWidth || emu.lcd_width;
  const height = emu.lcdHeight || emu.lcd_height;
  if (typeof width === 'function') lcdW = width.call(emu);
  if (typeof height === 'function') lcdH = height.call(emu);
}

skin.addEventListener('load', () => { skinReady = true; });
skin.addEventListener('error', () => {
  status.textContent = 'skin.png missing — run web/build.sh after extracting assets/skin.png.';
});

// Key mapping for the browser keyboard.  Values are [KI, KO] matrix codes.
const keymap = {
  '0': [0x10, 0x40], '1': [0x01, 0x01], '2': [0x01, 0x02], '3': [0x01, 0x04],
  '4': [0x02, 0x01], '5': [0x02, 0x02], '6': [0x02, 0x04],
  '7': [0x04, 0x01], '8': [0x04, 0x02], '9': [0x04, 0x04],
  '.': [0x08, 0x40], '+': [0x01, 0x08], '-': [0x01, 0x10],
  '*': [0x02, 0x08], 'x': [0x02, 0x08], '/': [0x02, 0x10],
  '=': [0x01, 0x40], 'Enter': [0x01, 0x40],
  'Backspace': [0x04, 0x08], 'Delete': [0x04, 0x10], 'Escape': [0x04, 0x10],
  'ArrowLeft': [0x40, 0x04], 'ArrowRight': [0x80, 0x08],
  'ArrowUp': [0x80, 0x04], 'ArrowDown': [0x40, 0x08],
  's': [0x10, 0x08], 'c': [0x10, 0x10], 't': [0x10, 0x20],
  'l': [0x20, 0x10], 'n': [0x20, 0x20], 'r': [0x20, 0x02],
  'p': [0x20, 0x04], 'a': [0x40, 0x01], 'b': [0x20, 0x01],
  'h': [0x10, 0x04], 'y': [0x04, 0x40], 'z': [0x40, 0x10], 'm': [0x08, 0x20],
  'F1': [0x80, 0x10], 'F2': [0x80, 0x01], 'F3': [0x80, 0x02], 'F4': [0x00, 0x00],
  'F5': [0x20, 0x02],
};

function keyCodeForEvent(e) {
  return keymap[e.key] || keymap[e.key.toLowerCase()] || keymap[e.code] || null;
}

function pressKey(code) {
  if (!emu) return;
  if (code[0] === 0x00 && code[1] === 0x00) {
    emu.reset();
  } else {
    emu.press(code[0], code[1]);
  }
}

window.addEventListener('keydown', (e) => {
  if (!emu) return;
  const code = keyCodeForEvent(e);
  if (!code) return;
  e.preventDefault();
  if (!e.repeat) pressKey(code);
}, { passive: false });

window.addEventListener('keyup', (e) => {
  if (!emu) return;
  const code = keyCodeForEvent(e);
  if (!code) return;
  e.preventDefault();
  emu.release();
}, { passive: false });

let romBytes = null;

async function loadRom(bytes) {
  romBytes = bytes;
  try {
    const id = modelSelect.value;
    emu = id ? Emulator.new_with_model(bytes, id) : new Emulator(bytes);
    updateLcdSize();
    modelSelect.value = emu.modelId ? emu.modelId() : id;
    status.textContent = `${emu.modelName()} — ROM loaded (${bytes.length.toLocaleString()} bytes). Running.`;
  } catch (err) {
    console.error(err);
    status.textContent = `Could not start ROM: ${err}`;
  }
}

async function switchModel(id) {
  if (!romBytes) {
    modelSelect.value = '';
    return;
  }
  try {
    emu = id ? Emulator.new_with_model(romBytes, id) : new Emulator(romBytes);
    updateLcdSize();
    status.textContent = `${emu.modelName()} — model switched. Running.`;
  } catch (err) {
    console.error(err);
    status.textContent = `Could not switch model: ${err}`;
  }
}

modelSelect.addEventListener('change', (e) => switchModel(e.target.value));

romInput.addEventListener('change', async (e) => {
  const file = e.target.files && e.target.files[0];
  if (!file) return;
  await loadRom(new Uint8Array(await file.arrayBuffer()));
});

function hitKey(x, y) {
  if (x < KEYPAD.x0 || x >= KEYPAD.x1 || y < KEYPAD.y0 || y >= KEYPAD.y1) return null;
  const gx = (x - KEYPAD.x0) * GRID_W / (KEYPAD.x1 - KEYPAD.x0);
  const gy = (y - KEYPAD.y0) * GRID_H / (KEYPAD.y1 - KEYPAD.y0);
  return keyTable.find((k) => gx >= k.x0 && gx <= k.x1 && gy >= k.y0 && gy <= k.y1) || null;
}

let activePointer = null;
let releaseTimer = null;

function pointerPoint(e) {
  const rect = canvas.getBoundingClientRect();
  // Map directly into design coordinates regardless of CSS size or DPR.
  return {
    x: (e.clientX - rect.left) * DESIGN.w / rect.width,
    y: (e.clientY - rect.top) * DESIGN.h / rect.height,
  };
}

function releaseKey() {
  if (releaseTimer !== null) {
    clearTimeout(releaseTimer);
    releaseTimer = null;
  }
  if (emu) emu.release();
}

canvas.addEventListener('pointerdown', (e) => {
  e.preventDefault();
  if (!emu) return;
  if (activePointer !== null && activePointer !== e.pointerId) return;
  activePointer = e.pointerId;
  if (canvas.setPointerCapture) {
    try { canvas.setPointerCapture(e.pointerId); } catch (_) { /* ignore */ }
  }
  const { x, y } = pointerPoint(e);
  const key = hitKey(x, y);
  if (!key) return;
  if (key.label === 'ON') {
    emu.reset();
  } else if (key.ki !== 0 || key.ko !== 0) {
    emu.press(key.ki, key.ko);
    // Release after the firmware has had a chance to scan the latch.  The
    // emulation loop is time-budgeted, so this delay is wall-clock accurate
    // even on slower machines.
    if (releaseTimer !== null) clearTimeout(releaseTimer);
    releaseTimer = setTimeout(releaseKey, 120);
  }
}, { passive: false });

function endPointer(e) {
  if (activePointer !== null && e.pointerId !== activePointer) return;
  activePointer = null;
  // Do not release immediately: the scheduled hold lets the firmware matrix
  // scan see short taps.  A long hold has already been released by the timer.
  if (canvas.hasPointerCapture && canvas.hasPointerCapture(e.pointerId)) {
    canvas.releasePointerCapture(e.pointerId);
  }
}

function cancelPointer(e) {
  if (activePointer !== null && e.pointerId !== activePointer) return;
  activePointer = null;
  releaseKey();
  if (canvas.hasPointerCapture && canvas.hasPointerCapture(e.pointerId)) {
    canvas.releasePointerCapture(e.pointerId);
  }
}

canvas.addEventListener('pointerup', endPointer);
canvas.addEventListener('pointercancel', cancelPointer);
window.addEventListener('pointerup', endPointer);
canvas.addEventListener('contextmenu', (e) => e.preventDefault());

function drawLcd(lcd) {
  ctx.fillStyle = COLORS.lcdBg;
  ctx.fillRect(LCD.x0, LCD.y0, LCD.x1 - LCD.x0, LCD.y1 - LCD.y0);
  ctx.fillStyle = COLORS.lcdFg;
  const w = LCD.x1 - LCD.x0;
  const h = LCD.y1 - LCD.y0;
  for (let y = 0; y < lcdH; y++) {
    let x = 0;
    while (x < lcdW) {
      if (!lcd[y * lcdW + x]) { x++; continue; }
      let run = x;
      while (run < lcdW && lcd[y * lcdW + run]) run++;
      const dx0 = LCD.x0 + Math.floor(x * w / lcdW);
      const dx1 = LCD.x0 + Math.floor(run * w / lcdW);
      const dy0 = LCD.y0 + Math.floor(y * h / lcdH);
      const dy1 = LCD.y0 + Math.floor((y + 1) * h / lcdH);
      ctx.fillRect(dx0, dy0, dx1 - dx0, dy1 - dy0);
      x = run;
    }
  }
}

function drawFallback() {
  ctx.fillStyle = COLORS.panel;
  ctx.fillRect(0, 0, DESIGN.w, DESIGN.h);

  // Warm ivory body with a soft edge.
  ctx.beginPath();
  ctx.roundRect(8, 8, DESIGN.w - 16, DESIGN.h - 16, 20);
  ctx.fillStyle = COLORS.device;
  ctx.fill();
  ctx.lineWidth = 1;
  ctx.strokeStyle = COLORS.deviceEdge;
  ctx.stroke();

  // Recessed bezel around the LCD.
  ctx.beginPath();
  ctx.roundRect(LCD.x0 - 6, LCD.y0 - 6, LCD.x1 - LCD.x0 + 12, LCD.y1 - LCD.y0 + 12, 6);
  ctx.fillStyle = COLORS.bezel;
  ctx.fill();

  const kw = (KEYPAD.x1 - KEYPAD.x0) / GRID_W;
  const kh = (KEYPAD.y1 - KEYPAD.y0) / GRID_H;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  for (const k of keyTable) {
    const x = KEYPAD.x0 + k.x0 * kw;
    const y = KEYPAD.y0 + k.y0 * kh;
    const w = (k.x1 - k.x0) * kw - 2;
    const h = (k.y1 - k.y0) * kh - 2;
    const on = k.label === 'ON';
    ctx.beginPath();
    ctx.roundRect(x + 1, y + 1, w, h, 4);
    ctx.fillStyle = on ? COLORS.accent : COLORS.key;
    ctx.fill();
    ctx.fillStyle = on ? '#2a2329' : COLORS.text;
    ctx.font = `${Math.max(5, Math.min(9, w / Math.max(1, k.label.length)))}px system-ui, sans-serif`;
    ctx.fillText(k.label, x + 1 + w / 2, y + 1 + h / 2);
  }
}

function draw() {
  syncCanvas();
  if (skinReady) {
    ctx.drawImage(skin, 0, 0, DESIGN.w, DESIGN.h);
  } else {
    drawFallback();
  }
  if (emu) drawLcd(emu.render());
}

function frame() {
  if (emu) {
    // Time-budget the emulation so pointer/keyboard events are handled every
    // frame even on slower machines.  Wasm calls are batched to reduce
    // boundary overhead.
    // ~12ms of a 16ms frame: drawing is cheap, and a short slice left busy
    // firmware (menu redraws) running well below real-hardware speed.
    const deadline = performance.now() + 12;
    let batches = 0;
    do {
      emu.step(4096);
      batches++;
    } while (performance.now() < deadline && batches < 64);
  }
  draw();
  requestAnimationFrame(frame);
}

async function boot() {
  await init();
  keyTable = JSON.parse(key_table_json());
  try {
    for (const m of JSON.parse(model_ids())) {
      const option = document.createElement('option');
      option.value = m.id;
      option.textContent = m.name;
      modelSelect.appendChild(option);
    }
  } catch (e) {
    console.warn('could not populate model list', e);
  }
  // Optional convenience for local demos/tests:
  //   ?rom=fx991es.rom.bin&boot=6000000
  const params = new URLSearchParams(location.search);
  const autoRom = params.get('rom');
  if (autoRom) {
    try {
      const res = await fetch(autoRom);
      if (res.ok) {
        await loadRom(new Uint8Array(await res.arrayBuffer()));
        const bootSteps = Number.parseInt(params.get('boot') || '0', 10);
        if (emu && bootSteps > 0) emu.step(bootSteps);
      }
    } catch (e) {
      console.warn('auto ROM load failed', e);
    }
  }

  resetButton.addEventListener('click', () => { if (emu) emu.reset(); });

  draw();
  requestAnimationFrame(frame);
}

boot().catch((err) => {
  console.error(err);
  status.textContent = `Startup failed: ${err}`;
});
