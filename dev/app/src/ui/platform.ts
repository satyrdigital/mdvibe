/**
 * Platform conventions. Shortcuts are written once in Windows notation
 * ("Ctrl+Shift+L"); on macOS they are shown with the Mac symbols ("⇧⌘L"),
 * and keys that have a Mac equivalent use it (F5 → ⌘R, F11 → ⌃⌘F).
 */
let mac = false;

export function setPlatform(platform: string): void {
  mac = platform === 'macos';
}

export function isMac(): boolean {
  return mac;
}

/** Windows shortcuts that are different keys on a Mac. */
const MAC_KEYS: Record<string, string> = {
  F5: 'Ctrl+R',
  F11: 'Ctrl+Meta+F',
  F3: 'Ctrl+G',
  'Shift+F3': 'Ctrl+Shift+G',
  'Ctrl+Shift+B': 'Ctrl+Meta+S',
};

/** Formats one shortcut ("Ctrl+Shift+L", or several joined by " / "). */
export function keys(shortcut: string): string {
  if (!mac) return shortcut;
  return shortcut
    .split(' / ')
    .map((one) => {
      const k = MAC_KEYS[one] ?? one;
      // "Ctrl++" ends with the key "+".
      const parts = k.endsWith('++') ? [...k.slice(0, -2).split('+'), '+'] : k.split('+');
      const key = parts.pop() ?? '';
      const mods = new Set(parts);
      // Mac order: Control, Option, Shift, Command. "Meta" here is the real Control key.
      return `${mods.has('Meta') ? '⌃' : ''}${mods.has('Alt') ? '⌥' : ''}${mods.has('Shift') ? '⇧' : ''}${mods.has('Ctrl') ? '⌘' : ''}${key === 'Esc' ? 'esc' : key}`;
    })
    .join(' / ');
}

/** Rewrites shortcuts inside a translated label ("Reading layout (Ctrl+Shift+L)"). */
export function keysIn(text: string): string {
  if (!mac) return text;
  return text.replace(/ \(Alt\)$/, '').replace(/\b(?:(?:Ctrl|Shift|Alt)\+)+(?:\S)/g, (m) => keys(m));
}
