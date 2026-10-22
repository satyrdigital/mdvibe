# Changelog

All notable changes to MDVibe are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- macOS app (macOS 11 or later, Apple Silicon and Intel), distributed as a disk image:
  - one app with a window per document; documents open from Finder, the Dock and
    "Open With"; an already open document comes to the front; the app keeps running
    after its last window closes;
  - native menu bar in the interface language, ⌘ shortcuts shown throughout;
  - Mac look: the toolbar is the title bar, controls and the sidebar float on glass over
    the document, system font, the user's accent color, Dark Mode;
  - Print and Save as PDF with the page setup from the reading settings;
  - documents appear in the system's recent items (Dock, Apple menu);
  - prepared for the Mac App Store sandbox: a one-time bar allows the document's folder
    to show its images, Recent keeps working.
- Third-party notices cover the macOS app.

### Changed

- Tauri 2.12.5.

## [0.1.0] - unreleased

First public preview.

### Added

- Markdown viewer for Windows 10/11; every document opens in its own window and process.
- GitHub Flavored Markdown: tables, task lists, strikethrough, autolinks, footnotes,
  GitHub alerts, GitHub-compatible heading anchors.
- Code blocks with language label, syntax highlighting, Copy button and optional line numbers.
- Sidebar with document outline and recent documents (pin, remove, clear, missing-file state).
- Find in document with match count and keyboard navigation.
- Automatic reload of changed files that keeps the reading position (or a reload prompt).
- Reading layout presets and controls; continuous and page view; A4/A5/Letter/Legal/custom
  page sizes, orientation and margins.
- Print and Save as PDF using a dedicated print layout.
- Light, dark and system themes with a toolbar switch (Ctrl+Shift+T); Enhanced and Neutral Markdown styles.
- English, Ukrainian and Russian interface; the Windows display language is used by default
  (English when it is not available), switchable in the menu and in Settings.
- Source view (read-only Markdown), zoom, full screen, always on top.
- Safe link handling: web links open in the browser, local Markdown links in a new window,
  other local files are never executed.
- Remote images blocked by default with per-document opt-in.
- Settings, recent documents and window geometry stored locally; corrupt files are recovered.
- Installer that registers MDVibe in "Open with" without changing the default app;
  portable build.
- Local diagnostic log with rotation; no telemetry.
- Licensed under the MIT License with the Commons Clause (source-available).
