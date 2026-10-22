// Builds the macOS artwork (macOS only: uses iconutil and tiffutil).
// App icon from the brand mark, on Apple's icon grid: 1024 px canvas,
// 824 px continuous-corner squircle, drop shadow below it.
//   dev/assets/brand/svg/mdvibe-icon-macos.svg        (generated source, in git)
//   dev/app/src-tauri/icons/icon.icns                 (16–1024 px)
// DMG window background, 1x + 2x in one TIFF:
//   dev/app/src-tauri/macos/dmg-background.svg        (source, in git)
//   dev/app/src-tauri/macos/dmg-background.tiff
// Needs `sharp` (dev only, not shipped): `npm install --no-save sharp` in dev/app.
// Usage: node dev/scripts/mac-assets.mjs
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const svgOut = path.join(root, 'assets', 'brand', 'svg', 'mdvibe-icon-macos.svg');
const icnsOut = path.join(root, 'app', 'src-tauri', 'icons', 'icon.icns');
const dmgSvg = path.join(root, 'app', 'src-tauri', 'macos', 'dmg-background.svg');
const dmgOut = path.join(root, 'app', 'src-tauri', 'macos', 'dmg-background.tiff');

/** Continuous-corner squircle (superellipse, n = 5) as an SVG path. */
function squircle(x, y, size, n = 5, steps = 256) {
  const r = size / 2;
  const cx = x + r;
  const cy = y + r;
  const pts = [];
  for (let i = 0; i < steps; i++) {
    const t = (i / steps) * 2 * Math.PI;
    const c = Math.cos(t);
    const s = Math.sin(t);
    pts.push([cx + r * Math.sign(c) * Math.abs(c) ** (2 / n), cy + r * Math.sign(s) * Math.abs(s) ** (2 / n)]);
  }
  return `M${pts.map(([a, b]) => `${a.toFixed(2)},${b.toFixed(2)}`).join('L')}Z`;
}

const body = squircle(100, 100, 824);
// Brand mark (64-unit grid) placed on the 824 body, slightly inset so it breathes.
const k = (824 / 64) * 0.94;
const ox = 100 + (824 - 64 * k) / 2;
const oy = 100 + (824 - 64 * k) / 2;
const X = (v) => (ox + v * k).toFixed(1);
const Y = (v) => (oy + v * k).toFixed(1);
const bar = (x, y, w) =>
  `<rect x="${X(x)}" y="${Y(y)}" width="${(w * k).toFixed(1)}" height="${(4 * k).toFixed(1)}" rx="${(2 * k).toFixed(1)}" fill="url(#ink)"/>`;

const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" width="1024" height="1024">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#5874EC"/>
      <stop offset="1" stop-color="#2F4BC8"/>
    </linearGradient>
    <linearGradient id="sheen" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#FFFFFF" stop-opacity="0.22"/>
      <stop offset="0.45" stop-color="#FFFFFF" stop-opacity="0"/>
    </linearGradient>
    <linearGradient id="paper" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#FFFFFF"/>
      <stop offset="1" stop-color="#EEF1FB"/>
    </linearGradient>
    <linearGradient id="fold" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#FFC96B"/>
      <stop offset="1" stop-color="#FFA634"/>
    </linearGradient>
    <linearGradient id="ink" x1="0" y1="0" x2="1" y2="0">
      <stop offset="0" stop-color="#3B5BDB"/>
      <stop offset="1" stop-color="#4C6BE6"/>
    </linearGradient>
    <filter id="drop" x="-20%" y="-20%" width="140%" height="150%">
      <feGaussianBlur in="SourceAlpha" stdDeviation="14"/>
      <feOffset dy="12"/>
      <feComponentTransfer><feFuncA type="linear" slope="0.32"/></feComponentTransfer>
      <feMerge><feMergeNode/><feMergeNode in="SourceGraphic"/></feMerge>
    </filter>
    <filter id="lift" x="-30%" y="-30%" width="160%" height="160%">
      <feGaussianBlur in="SourceAlpha" stdDeviation="16"/>
      <feOffset dy="14"/>
      <feComponentTransfer><feFuncA type="linear" slope="0.28"/></feComponentTransfer>
      <feMerge><feMergeNode/><feMergeNode in="SourceGraphic"/></feMerge>
    </filter>
    <clipPath id="clip"><path d="${body}"/></clipPath>
  </defs>
  <g filter="url(#drop)">
    <path d="${body}" fill="url(#bg)"/>
  </g>
  <g clip-path="url(#clip)">
    <path d="${body}" fill="url(#sheen)"/>
    <g filter="url(#lift)">
      <path d="M${X(18)} ${Y(10)}H${X(36)}L${X(48)} ${Y(22)}V${Y(54)}H${X(18)}Z" fill="url(#paper)"/>
    </g>
    <path d="M${X(36)} ${Y(10)}L${X(48)} ${Y(22)}H${X(36)}Z" fill="url(#fold)"/>
    ${bar(23, 29, 20)}
    ${bar(23, 37, 20)}
    ${bar(23, 45, 12)}
  </g>
  <path d="${body}" fill="none" stroke="#FFFFFF" stroke-opacity="0.18" stroke-width="3"/>
</svg>
`;

fs.writeFileSync(svgOut, svg);
console.log(`wrote ${path.relative(root, svgOut)}`);

let sharp;
try {
  sharp = createRequire(path.join(root, 'app', 'package.json'))('sharp');
} catch {
  console.error('sharp is not installed: run `npm install --no-save sharp` in dev/app, then run this again.');
  process.exit(1);
}

const set = fs.mkdtempSync(path.join(os.tmpdir(), 'mdvibe-icon-')) + '/icon.iconset';
fs.mkdirSync(set);
const master = await sharp(Buffer.from(svg)).png().toBuffer();
for (const size of [16, 32, 128, 256, 512]) {
  for (const scale of [1, 2]) {
    const px = size * scale;
    const name = `icon_${size}x${size}${scale === 2 ? '@2x' : ''}.png`;
    await sharp(master).resize(px, px, { kernel: 'lanczos3' }).png().toFile(path.join(set, name));
  }
}
execFileSync('iconutil', ['-c', 'icns', set, '-o', icnsOut]);
console.log(`wrote ${path.relative(root, icnsOut)}`);

const bg = fs.readFileSync(dmgSvg);
const bg1 = path.join(path.dirname(set), 'bg.png');
const bg2 = path.join(path.dirname(set), 'bg@2x.png');
await sharp(bg, { density: 72 }).resize(660, 420).png().toFile(bg1);
await sharp(bg, { density: 144 }).resize(1320, 840).png().toFile(bg2);
execFileSync('tiffutil', ['-cathidpicheck', bg1, bg2, '-out', dmgOut]);
console.log(`wrote ${path.relative(root, dmgOut)}`);
fs.rmSync(path.dirname(set), { recursive: true, force: true });
