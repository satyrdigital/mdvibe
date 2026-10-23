/**
 * Typed access to the native shell. In a plain browser (vite dev server,
 * UI work) a mock backend with a sample document is used instead.
 */

export interface LoadedDocument {
  path: string;
  name: string;
  dir: string;
  text: string;
  encoding: string;
  bytes: number;
  modifiedMs: number;
  lineEnding: 'LF' | 'CRLF';
}

export interface DocumentError {
  kind: 'notFound' | 'accessDenied' | 'locked' | 'isDirectory' | 'tooLarge' | 'notText' | 'other';
  path: string;
  detail: string;
}

export interface RecentItem {
  path: string;
  name: string;
  dir: string;
  openedAt: number;
  pinned: boolean;
  exists: boolean;
}

export type LinkTarget =
  | { kind: 'external'; url: string }
  | { kind: 'markdown'; path: string; fragment: string | null }
  | { kind: 'localFile'; path: string }
  | { kind: 'missing'; path: string }
  | { kind: 'blocked'; reason: string };

export interface StartupInfo {
  productName: string;
  version: string;
  platform: string;
  debug: boolean;
  launchFile: string | null;
  settings: unknown;
  imageBase: string;
  webviewVersion: string | null;
  logDir: string;
  /** macOS: the user picked an accent color, controls follow it. */
  systemAccent: boolean;
  /** Mac App Store build: no donation link and no own update check (store rules). */
  storeBuild: boolean;
}

export interface PdfOptions {
  pageWidthMm: number;
  pageHeightMm: number;
  marginMm: number;
  landscape: boolean;
  backgrounds: boolean;
  headerFooter: boolean;
  title: string;
}

/** Native menu state (macOS): labels by item id, checkmarks and enabled flags. */
export interface MenuState {
  labels?: Record<string, string>;
  checks?: Record<string, boolean>;
  enabled?: Record<string, boolean>;
}

export interface Backend {
  readonly native: boolean;
  startupInfo(): Promise<StartupInfo>;
  loadSettings(): Promise<unknown>;
  openNotices(): Promise<void>;
  openDocument(path: string): Promise<LoadedDocument>;
  reloadDocument(): Promise<LoadedDocument>;
  pickMarkdownFile(): Promise<string | null>;
  openInNewWindow(path: string | null): Promise<void>;
  activateLink(href: string): Promise<LinkTarget>;
  openExternal(url: string): Promise<void>;
  revealInFolder(path: string): Promise<void>;
  saveSettings(settings: unknown): Promise<void>;
  recentList(): Promise<RecentItem[]>;
  recentRemove(path: string): Promise<void>;
  recentSetPinned(path: string, pinned: boolean): Promise<void>;
  recentClear(): Promise<void>;
  exportPdf(options: PdfOptions): Promise<string | null>;
  /** Native print dialog (macOS); resolves when it closes. */
  printDocument(options: PdfOptions): Promise<void>;
  menuSync(state: MenuState): void;
  /** App Store sandbox (macOS): must the user allow the document's folder to show its images? */
  folderAccess(): Promise<{ needed: boolean }>;
  /** Asks for the document's folder; true when its images can be shown now. */
  grantFolderAccess(): Promise<boolean>;
  log(level: 'info' | 'warn' | 'error', message: string): void;
  openLogFolder(): Promise<void>;
  ready(metrics: { renderMs?: number; docBytes?: number }): Promise<void>;
  on(event: 'doc-changed', cb: (p: { exists: boolean }) => void): Promise<void>;
  on(event: 'settings-changed' | 'recent-changed', cb: () => void): Promise<void>;
  on(event: 'open-file' | 'menu', cb: (p: string) => void): Promise<void>;
  on(event: 'fullscreen', cb: (on: boolean) => void): Promise<void>;
  onFileDrop(cb: (paths: string[]) => void, hover: (over: boolean) => void): Promise<void>;
  setFullscreen(on: boolean): Promise<void>;
  isFullscreen(): Promise<boolean>;
  setAlwaysOnTop(on: boolean): Promise<void>;
  setTheme(theme: 'light' | 'dark' | null): Promise<void>;
  closeWindow(): Promise<void>;
}

function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

async function createTauriBackend(): Promise<Backend> {
  const { invoke } = await import('@tauri-apps/api/core');
  const { listen } = await import('@tauri-apps/api/event');
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  const { getCurrentWebview } = await import('@tauri-apps/api/webview');
  const win = getCurrentWindow();
  return {
    native: true,
    startupInfo: () => invoke('startup_info'),
    loadSettings: () => invoke('settings_load'),
    openNotices: () => invoke('open_notices'),
    openDocument: (path) => invoke('open_document', { path }),
    reloadDocument: () => invoke('reload_document'),
    pickMarkdownFile: () => invoke('pick_markdown_file'),
    openInNewWindow: (path) => invoke('open_in_new_window', { path }),
    activateLink: (href) => invoke('activate_link', { href }),
    openExternal: (url) => invoke('open_external', { url }),
    revealInFolder: (path) => invoke('reveal_in_folder', { path }),
    saveSettings: (settings) => invoke('settings_save', { settings }),
    recentList: () => invoke('recent_list'),
    recentRemove: (path) => invoke('recent_remove', { path }),
    recentSetPinned: (path, pinned) => invoke('recent_set_pinned', { path, pinned }),
    recentClear: () => invoke('recent_clear'),
    exportPdf: (options) => invoke('export_pdf', { options }),
    printDocument: (options) => invoke('print_document', { options }),
    menuSync: (menu) => void invoke('menu_sync', { menu }).catch(() => {}),
    folderAccess: () => invoke('folder_access'),
    grantFolderAccess: () => invoke('grant_folder_access'),
    log: (level, message) => void invoke('log_message', { level, message }).catch(() => {}),
    openLogFolder: () => invoke('open_log_folder'),
    ready: (metrics) => invoke('app_ready', { metrics }),
    // Only events for this window (or for all windows): on macOS every
    // document window lives in the same process.
    on: async (event: string, cb: (p: never) => void) => {
      await listen(event, (e) => cb(e.payload as never), { target: { kind: 'WebviewWindow', label: win.label } });
    },
    onFileDrop: async (cb, hover) => {
      await getCurrentWebview().onDragDropEvent((e) => {
        const p = e.payload;
        if (p.type === 'enter' || p.type === 'over') hover(true);
        else if (p.type === 'leave') hover(false);
        else if (p.type === 'drop') {
          hover(false);
          cb(p.paths);
        }
      });
    },
    setFullscreen: (on) => win.setFullscreen(on),
    isFullscreen: () => win.isFullscreen(),
    setAlwaysOnTop: (on) => win.setAlwaysOnTop(on),
    setTheme: (theme) => win.setTheme(theme),
    closeWindow: () => win.close(),
  } as Backend;
}

export async function createBackend(): Promise<Backend> {
  if (isTauri()) return createTauriBackend();
  const { createMockBackend } = await import('./mock');
  return createMockBackend();
}
