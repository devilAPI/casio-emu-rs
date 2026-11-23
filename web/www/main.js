import init, { Emulator, key_table_json } from './pkg/fx991es_web.js';

const canvas = document.getElementById('calc');
const ctx = canvas.getContext('2d', { alpha: false });
const status = document.getElementById('status');
const romInput = document.getElementById('romfile');
const resetButton = document.getElementById('reset');

// Geometry of the official 289x582 skin.
const LCD = { x0: 39, y0: 94, x1: 254, y1: 179 };
const KEYPAD = { x0: 10, y0: 190, x1: 280, y1: 575 };
const GRID_W = 43;
const GRID_H = 28;

const skin = new Image();
skin.src = 'skin.png';

let keyTable = [];

let emu = null;
let skinReady = false;

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

async function loadRom(bytes) {
  try {
    emu = new Emulator(bytes);
    status.textContent = `ROM loaded (${bytes.length.toLocaleString()} bytes). Running.`;
  } catch (err) {
    console.error(err);
    status.textContent = `Could not start ROM: ${err}`;
  }
}

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

canvas.addEventListener('pointerdown', (e) => {
  if (!emu) return;
  const rect = canvas.getBoundingClientRect();
  const x = (e.clientX - rect.left) * canvas.width / rect.width;
  const y = (e.clientY - rect.top) * canvas.height / rect.height;
  const key = hitKey(x, y);
  if (!key) return;
  if (key.label === 'ON') {
    emu.reset();
  } else if (key.ki !== 0 || key.ko !== 0) {
    emu.press(key.ki, key.ko);
    // Release after the firmware has had a chance to scan the latch.
    setTimeout(() => { if (emu) emu.release(); }, 120);
  }
});

function drawLcd(lcd) {
  ctx.fillStyle = '#d6e3d6';
  ctx.fillRect(LCD.x0, LCD.y0, LCD.x1 - LCD.x0, LCD.y1 - LCD.y0);
  ctx.fillStyle = '#101810';
  const w = LCD.x1 - LCD.x0;
  const h = LCD.y1 - LCD.y0;
  for (let y = 0; y < 32; y++) {
    let x = 0;
    while (x < 96) {
      if (!lcd[y * 96 + x]) { x++; continue; }
      let run = x;
      while (run < 96 && lcd[y * 96 + run]) run++;
      const dx0 = LCD.x0 + Math.floor(x * w / 96);
      const dx1 = LCD.x0 + Math.floor(run * w / 96);
      const dy0 = LCD.y0 + Math.floor(y * h / 32);
      const dy1 = LCD.y0 + Math.floor((y + 1) * h / 32);
      ctx.fillRect(dx0, dy0, dx1 - dx0, dy1 - dy0);
      x = run;
    }
  }
}

function drawFallback() {
  ctx.fillStyle = '#2b2c33';
  ctx.fillRect(0, 0, canvas.width, canvas.height);

  ctx.fillStyle = '#d8d8dc';
  ctx.beginPath();
  ctx.roundRect(8, 8, canvas.width - 16, canvas.height - 16, 18);
  ctx.fill();

  ctx.fillStyle = '#3a3a40';
  ctx.fillRect(LCD.x0 - 5, LCD.y0 - 5, LCD.x1 - LCD.x0 + 10, LCD.y1 - LCD.y0 + 10);

  const kw = (KEYPAD.x1 - KEYPAD.x0) / GRID_W;
  const kh = (KEYPAD.y1 - KEYPAD.y0) / GRID_H;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  for (const k of keyTable) {
    const x = KEYPAD.x0 + k.x0 * kw;
    const y = KEYPAD.y0 + k.y0 * kh;
    const w = (k.x1 - k.x0) * kw - 2;
    const h = (k.y1 - k.y0) * kh - 2;
    ctx.fillStyle = k.label === 'ON' ? '#ff8844' : '#44444c';
    ctx.beginPath();
    ctx.roundRect(x + 1, y + 1, w, h, 3);
    ctx.fill();
    ctx.fillStyle = '#f0f0f0';
    ctx.font = `${Math.max(5, Math.min(8, w / Math.max(1, k.label.length)))}px system-ui, sans-serif`;
    ctx.fillText(k.label, x + 1 + w / 2, y + 1 + h / 2);
  }
}

function draw() {
  if (skinReady) {
    ctx.drawImage(skin, 0, 0, canvas.width, canvas.height);
  } else {
    drawFallback();
  }
  if (emu) drawLcd(emu.render());
}

function frame() {
  if (emu) {
    emu.step(120000);
  }
  draw();
  requestAnimationFrame(frame);
}

async function boot() {
  await init();
  keyTable = JSON.parse(key_table_json());
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
