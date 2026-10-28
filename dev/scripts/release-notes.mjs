// Release notes for a version: its CHANGELOG.md section plus the standard
// download and first-launch blocks. The release workflow uses this as the
// body of the draft, so the notes never need hand editing.
//
// Usage:  node dev/scripts/release-notes.mjs 0.2.0 [--out file]
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const repo = 'https://github.com/satyrdigital/mdvibe';

/** The `## [version] - date` section of CHANGELOG.md and the version before it. */
export function changelogSection(version) {
  const text = fs.readFileSync(path.join(root, 'CHANGELOG.md'), 'utf8');
  const heads = [...text.matchAll(/^## \[([^\]]+)\](?: - (\d{4}-\d{2}-\d{2}))?[ \t]*$/gm)];
  const i = heads.findIndex((h) => h[1] === version && h[2]);
  if (i < 0) return null;
  const end = heads[i + 1]?.index ?? text.length;
  const previous = heads.slice(i + 1).find((h) => /^\d/.test(h[1]))?.[1] ?? null;
  return { date: heads[i][2], body: text.slice(heads[i].index + heads[i][0].length, end).trim(), previous };
}

export function releaseNotes(version) {
  const section = changelogSection(version);
  if (!section) throw new Error(`CHANGELOG.md has no dated section for ${version}`);
  const compare = section.previous
    ? `**Full Changelog**: ${repo}/compare/v${section.previous}...v${version}`
    : `**Full Changelog**: ${repo}/commits/v${version}`;
  return `${section.body}

## Download

| File | What it is |
| --- | --- |
| \`MDVibe-Setup.exe\` | Windows installer (Windows 10 and 11) |
| \`MDVibe-${version}-win-x64.zip\` | Windows portable version |
| \`MDVibe-mac.dmg\` | Mac disk image (macOS 11 or later, Apple Silicon and Intel) |
| \`SHA256SUMS.txt\`, \`SHA256SUMS-mac.txt\` | Checksums |

## First launch

- **Windows:** builds are not code-signed, so SmartScreen may show *"Windows protected your PC"* — choose **More info → Run anyway**.
- **Mac:** MDVibe is not notarized by Apple yet. On the first launch press **Done**, then open **System Settings → Privacy & Security**, and next to *"MDVibe" was blocked to protect your Mac* press **Open Anyway**. Needed once. Details in the [README](${repo}#installation-on-macos).

${compare}
`;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  const version = (process.argv[2] || '').replace(/^v/, '');
  if (!version) {
    console.error('usage: node dev/scripts/release-notes.mjs <version> [--out file]');
    process.exit(2);
  }
  const notes = releaseNotes(version);
  const outIdx = process.argv.indexOf('--out');
  if (outIdx > 0) fs.writeFileSync(process.argv[outIdx + 1], notes);
  else process.stdout.write(notes);
}
