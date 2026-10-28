// Checks a published release the way a visitor gets it — through the
// releases/latest/download/… links the project page and README use:
//   * the release is published, not a pre-release, and is the latest one;
//   * every expected file is attached and matches SHA256SUMS*.txt;
//   * the Windows exe and (on macOS) the app inside the disk image carry the version;
//   * with --site (instead of the above): every download link on the live
//     project page leads to this release, and the README "release" badge shows it.
// Exits with 1 and a list of problems; the Published workflow runs it after
// the owner presses Publish.
//
// Usage:  node dev/scripts/verify-release.mjs v0.2.0 [--site]
// Env:    GITHUB_TOKEN (optional, avoids API rate limits)
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const REPO = 'satyrdigital/mdvibe';
const SITE = 'https://satyrdigital.github.io/mdvibe/';
const tag = process.argv[2];
const withSite = process.argv.includes('--site');
if (!/^v\d+\.\d+\.\d+/.test(tag || '')) {
  console.error('usage: node dev/scripts/verify-release.mjs v1.2.3 [--site]');
  process.exit(2);
}
const version = tag.slice(1);
const problems = [];
const fail = (msg) => problems.push(msg);
const ok = (msg) => console.log(`✓ ${msg}`);

const EXPECTED = [
  'MDVibe-Setup.exe',
  `MDVibe-Setup-${version}.exe`,
  `MDVibe-${version}-win-x64.zip`,
  'SHA256SUMS.txt',
  'MDVibe-mac.dmg',
  `MDVibe-${version}-mac-universal.dmg`,
  'SHA256SUMS-mac.txt',
];

const headers = { 'user-agent': 'mdvibe-release-check', accept: 'application/vnd.github+json' };
if (process.env.GITHUB_TOKEN) headers.authorization = `Bearer ${process.env.GITHUB_TOKEN}`;

async function api(url) {
  const res = await fetch(`https://api.github.com/repos/${REPO}${url}`, { headers });
  if (!res.ok) throw new Error(`GitHub API ${url}: ${res.status}`);
  return res.json();
}

/** Downloads through the public link; returns the bytes and the final URL. */
async function download(url) {
  const res = await fetch(url, { redirect: 'follow', headers: { 'user-agent': headers['user-agent'] } });
  if (!res.ok) throw new Error(`${url}: HTTP ${res.status}`);
  return { bytes: Buffer.from(await res.arrayBuffer()), finalUrl: res.url };
}

async function retry(what, attempts, fn) {
  for (let i = 1; ; i++) {
    try {
      return await fn();
    } catch (e) {
      if (i >= attempts) throw e;
      console.log(`… ${what}: ${e.message}; retrying (${i}/${attempts - 1})`);
      await new Promise((r) => setTimeout(r, 30_000));
    }
  }
}

const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');

/** ProductVersion from the version resource of a Windows exe. */
function exeVersion(bytes) {
  const text = bytes.toString('utf16le');
  const m = /ProductVersion\0+([0-9][0-9A-Za-z.+-]*)/.exec(text);
  return m ? m[1] : null;
}

async function checkRelease() {
  const release = await api(`/releases/tags/${tag}`);
  if (release.draft) fail(`${tag} is still a draft`);
  if (release.prerelease) fail(`${tag} is marked as a pre-release`);
  const latest = await api('/releases/latest');
  if (latest.tag_name !== tag) fail(`latest release is ${latest.tag_name}, not ${tag}`);
  else ok(`${tag} is published and is the latest release`);

  const names = release.assets.map((a) => a.name);
  const missing = EXPECTED.filter((n) => !names.includes(n));
  if (missing.length) fail(`missing files: ${missing.join(', ')}`);
  else ok(`all ${EXPECTED.length} files attached`);

  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'mdvibe-release-'));
  const files = {};
  for (const name of EXPECTED.filter((n) => names.includes(n))) {
    const { bytes, finalUrl } = await download(`https://github.com/${REPO}/releases/latest/download/${name}`);
    if (!finalUrl.includes(`/${tag}/`) && !finalUrl.includes('release-assets')) fail(`${name}: latest link leads to ${finalUrl}`);
    files[name] = bytes;
    fs.writeFileSync(path.join(dir, name), bytes);
  }
  ok('every file downloads through releases/latest/download/…');

  for (const sums of ['SHA256SUMS.txt', 'SHA256SUMS-mac.txt']) {
    if (!files[sums]) continue;
    for (const line of files[sums].toString('utf8').split('\n').filter(Boolean)) {
      const [hash, name] = line.trim().split(/\s+\*?/);
      if (!files[name]) fail(`${sums} lists ${name}, which is not attached`);
      else if (sha256(files[name]) !== hash) fail(`${name} does not match ${sums}`);
    }
  }
  const unlisted = EXPECTED.filter((n) => !n.startsWith('SHA256SUMS') && files[n]).filter(
    (n) => !['SHA256SUMS.txt', 'SHA256SUMS-mac.txt'].some((s) => files[s]?.toString('utf8').includes(` ${n}`)),
  );
  if (unlisted.length) fail(`not in any checksum file: ${unlisted.join(', ')}`);
  else ok('checksums match');

  const zip = `MDVibe-${version}-win-x64.zip`;
  if (files[zip]) {
    // Entries are stored as ./MDVibe.exe; the pattern matches either form.
    const exe = execFileSync('unzip', ['-p', path.join(dir, zip), '*MDVibe.exe'], { maxBuffer: 64 << 20 });
    const v = exeVersion(exe);
    if (v !== version) fail(`MDVibe.exe in ${zip} is version ${v}`);
    else ok(`Windows MDVibe.exe is ${v}`);
    const setup = exeVersion(files[`MDVibe-Setup-${version}.exe`] ?? Buffer.alloc(0));
    if (setup && setup !== version) fail(`MDVibe-Setup-${version}.exe is version ${setup}`);
  }

  const dmg = 'MDVibe-mac.dmg';
  if (files[dmg] && process.platform === 'darwin') {
    const mnt = path.join(dir, 'mnt');
    execFileSync('hdiutil', ['attach', '-nobrowse', '-readonly', '-mountpoint', mnt, path.join(dir, dmg)], { stdio: 'ignore' });
    try {
      const app = path.join(mnt, 'MDVibe.app');
      const plist = execFileSync('plutil', ['-extract', 'CFBundleShortVersionString', 'raw', path.join(app, 'Contents/Info.plist')], { encoding: 'utf8' }).trim();
      if (plist !== version) fail(`MDVibe.app in ${dmg} is version ${plist}`);
      else ok(`Mac MDVibe.app is ${plist}`);
      const archs = execFileSync('lipo', ['-archs', path.join(app, 'Contents/MacOS/MDVibe')], { encoding: 'utf8' }).trim().split(/\s+/);
      if (!archs.includes('arm64') || !archs.includes('x86_64')) fail(`Mac app is not universal: ${archs.join(' ')}`);
      else ok('Mac app runs on Apple Silicon and Intel');
      try {
        execFileSync('codesign', ['--verify', '--strict', app], { stdio: 'pipe' });
        ok('Mac app signature is valid');
      } catch (e) {
        fail(`Mac app signature: ${e.stderr?.toString().trim() || e.message}`);
      }
    } finally {
      execFileSync('hdiutil', ['detach', '-quiet', mnt]);
    }
  } else if (files[dmg]) {
    console.log('· disk image contents are checked on macOS only');
  }
}

async function checkSite() {
  // Pages and the badge service cache for a few minutes: retry before failing.
  await retry('project page', 6, async () => {
    const { bytes } = await download(`${SITE}?release-check=${Date.now()}`);
    const links = [...new Set([...bytes.toString('utf8').matchAll(/href="([^"]*\/releases\/[^"]*)"/g)].map((m) => m[1]))];
    const downloads = links.filter((l) => l.includes('/download/'));
    if (!downloads.length) throw new Error('no download links on the page');
    for (const link of downloads) {
      const res = await fetch(link, { method: 'HEAD', redirect: 'follow' });
      if (!res.ok) throw new Error(`${link}: HTTP ${res.status}`);
      if (!res.url.includes('release-assets') && !res.url.includes(`/${tag}/`)) throw new Error(`${link} leads to ${res.url}`);
    }
    ok(`project page: ${downloads.length} download links lead to ${tag}`);
  }).catch((e) => fail(`project page: ${e.message}`));

  await retry('README badge', 6, async () => {
    const res = await fetch(`https://img.shields.io/github/v/release/${REPO}.json?t=${Date.now()}`);
    const { value } = await res.json();
    if (value !== tag) throw new Error(`shows ${value}`);
    ok(`README release badge shows ${value}`);
  }).catch((e) => fail(`README badge: ${e.message}`));
}

try {
  if (withSite) await checkSite();
  else await checkRelease();
} catch (e) {
  fail(e.message);
}

if (problems.length) {
  for (const p of problems) console.error(`✗ ${p}`);
  process.exit(1);
}
console.log(`✓ release ${tag} checked`);
