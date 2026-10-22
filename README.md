<p align="center">
  <img src="dev/assets/brand/svg/mdvibe-icon.svg" width="80" height="80" alt="MDVibe">
</p>

<h1 align="center">MDVibe</h1>

<p align="center"><strong>Markdown, made calm.</strong></p>

<p align="center">
  A lightweight, fast Markdown viewer for Windows and macOS.<br>
  Double-click a <code>.md</code> file and read it — clean, safe, private. Every document opens in its own window.
</p>

<p align="center">
  <a href="https://github.com/satyrdigital/mdvibe/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/satyrdigital/mdvibe?label=release&color=3B5BDB"></a>
  <img alt="Windows 10 | 11" src="https://img.shields.io/badge/Windows-10%20%7C%2011-3B5BDB">
  <img alt="macOS 11 or later" src="https://img.shields.io/badge/macOS-11%2B-3B5BDB">
  <a href="https://github.com/satyrdigital/mdvibe/actions/workflows/ci.yml"><img alt="Build" src="https://github.com/satyrdigital/mdvibe/actions/workflows/ci.yml/badge.svg"></a>
</p>

<p align="center">
  <img src="docs/site/assets/screens/hero-light.webp" width="820" alt="MDVibe showing a Markdown document">
</p>

## Features

- **GitHub Flavored Markdown** — tables, task lists, footnotes, alerts, autolinks
- **Readable code** — syntax highlighting, language labels, one-click **Copy**, optional line numbers
- **One window per document** — opening another file never replaces the one you are reading
- **Live reload** — keeps your place while an editor or an AI agent rewrites the file
- **Outline and Recent** in a sidebar that hides completely
- **Find** in the document with match count
- **Reading layouts** — Comfortable, Compact, Wide, Book; font, size, width, margins
- **Page view, Print and Save as PDF** — A4, Letter, Legal or custom, portrait or landscape
- **Light, dark or system theme** — one click in the toolbar or Ctrl+Shift+T
- **English, Ukrainian and Russian interface** — follows the system language, switchable any time
- **At home on the Mac** — menu bar, ⌘ shortcuts, Dark Mode, your accent color, one app with a window per document
- **Private and safe** — works offline, no telemetry, scripts never run, remote images only when you allow them

<p align="center">
  <img src="docs/site/assets/screens/reading-layout.webp" width="260" alt="Reading layout panel">
  &nbsp;
  <img src="docs/site/assets/screens/settings.webp" width="520" alt="Settings window">
</p>

## Download

**[Download for Windows](https://github.com/satyrdigital/mdvibe/releases/latest/download/MDVibe-Setup.exe)** ·
**[Download for Mac](https://github.com/satyrdigital/mdvibe/releases/latest/download/MDVibe-mac.dmg)** ·
project page: **[satyrdigital.github.io/mdvibe](https://satyrdigital.github.io/mdvibe/)**

All files of every version are on the **[Releases](https://github.com/satyrdigital/mdvibe/releases/latest)** page:

| File | What it is |
| --- | --- |
| `MDVibe-Setup.exe` | Installer of the latest version (recommended) |
| `MDVibe-Setup-x.y.z.exe` | The same installer, named with its version |
| `MDVibe-x.y.z-win-x64.zip` | Portable version, no installation |
| `SHA256SUMS.txt` | Checksums |
| `MDVibe-mac.dmg` | Mac disk image of the latest version (Apple Silicon and Intel) |
| `MDVibe-x.y.z-mac-universal.dmg` | The same disk image, named with its version |
| `SHA256SUMS-mac.txt` | Checksums of the Mac files |

## Installation on Windows

1. Run `MDVibe-Setup.exe`. It installs for the current user — no administrator rights needed.
2. Start MDVibe from the Start menu, or open any `.md` file with it.

Requirements: Windows 10 or 11 with the Microsoft Edge WebView2 Runtime (built into
Windows 11; the installer adds it if missing).

Builds are not code-signed yet, so Windows SmartScreen may show *"Windows protected your PC"*.
Choose **More info → Run anyway** if you downloaded the file from this repository.

## Installation on macOS

Requires macOS 11 Big Sur or later, on Apple Silicon or Intel.

1. Open `MDVibe-mac.dmg` and drag **MDVibe** to **Applications**.
2. Open MDVibe from Applications.

MDVibe is not notarized by Apple yet (that needs a paid Apple Developer account), so the
first launch shows *"MDVibe" Not Opened — Apple could not verify…*. If you downloaded the
disk image from this repository:

1. Press **Done** in that message.
2. Open **System Settings → Privacy & Security**, scroll down to **Security**.
3. Next to *"MDVibe" was blocked…* press **Open Anyway**, confirm with your password, then **Open**.

This is needed once. Alternatively, in Terminal:
`xattr -dr com.apple.quarantine /Applications/MDVibe.app`

To open `.md` files with MDVibe by double-click: select a `.md` file in Finder →
**File → Get Info** → **Open with: MDVibe** → **Change All…**. MDVibe never makes itself
the default app on its own.

## Open With and default app on Windows

The installer adds MDVibe to **Open with** for `.md` and `.markdown` files. It never changes
your current default app — you decide:

1. Right-click a `.md` file → **Open with** → **Choose another app**.
2. Pick **MDVibe**, tick **Always use this app to open .md files**, press **OK**.

Uninstalling removes these registrations and leaves your documents untouched.

## Supported Markdown

CommonMark plus the GitHub extensions people actually use:

- headings, emphasis, ~~strikethrough~~, links, autolinks, images
- ordered, unordered and nested lists, task lists `- [x]`
- tables with column alignment
- fenced code blocks with syntax highlighting (35+ languages)
- block quotes and GitHub alerts (`> [!NOTE]`, `> [!TIP]`, `> [!IMPORTANT]`, `> [!WARNING]`, `> [!CAUTION]`)
- footnotes, horizontal rules, escaping
- embedded HTML such as `<details>`, `<kbd>`, `<sub>`, `<p align="center">` — sanitized

Local links to other Markdown files open in a new window. Links to other local files are
never run.

## Keyboard shortcuts

| Action | Windows | Mac |
| --- | --- | --- |
| Open file | Ctrl+O | ⌘O |
| New window | Ctrl+N | ⌘N |
| Find | Ctrl+F · Enter / Shift+Enter | ⌘F · ⌘G / ⇧⌘G |
| Sidebar | Ctrl+Shift+B | ⌃⌘S |
| Reading layout | Ctrl+Shift+L | ⇧⌘L |
| Switch theme | Ctrl+Shift+T | ⇧⌘T |
| Markdown source | Ctrl+U | ⌘U |
| Zoom in / out / reset | Ctrl+= · Ctrl+- · Ctrl+0 | ⌘= · ⌘- · ⌘0 |
| Print / Save as PDF | Ctrl+P · Ctrl+Shift+S | ⌘P · ⇧⌘S |
| Reload | F5 | ⌘R |
| Full screen | F11 | ⌃⌘F |
| Settings | Ctrl+, | ⌘, |
| All shortcuts | F1 | Help menu |

## Privacy

- No telemetry, analytics, ads, accounts or cloud.
- Your documents never leave your computer.
- Remote images are blocked until you allow them, because loading them reveals your IP
  address to the image host.
- Settings and the recent-files list stay on your computer; history can be turned off or cleared.

Found a security problem? Please see [SECURITY.md](SECURITY.md).

## Support

MDVibe is free. If it saves you time, you can leave a tip:

<a href="https://ko-fi.com/shumak"><img alt="Tip on Ko-fi" src="https://img.shields.io/badge/Ko--fi-Tip-FF5E5B?logo=kofi&logoColor=white"></a>
<a href="https://buymeacoffee.com/shumak"><img alt="Buy Me a Coffee" src="https://img.shields.io/badge/Buy%20Me%20a%20Coffee-Tip-FFDD00?logo=buymeacoffee&logoColor=black"></a>

Payments are processed by Soul Motors LLC.

## License

MDVibe is **source-available** under the [MIT License with the Commons Clause](LICENSE):
free to use (including at work), study, modify and share; selling MDVibe itself is not
permitted. It is not an OSI-approved open-source license.

Third-party components: [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## Author

**Andrii Shumak** · [satyr.digital](https://satyr.digital) · satyrdigital@gmail.com

Published by Satyr Digital.

Copyright © 2026 Andrii Shumak
