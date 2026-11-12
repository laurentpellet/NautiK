import type { Theme } from './map/style';

const STORAGE_KEY = 'nautik.theme';

/** Thème initial : choix mémorisé, sinon préférence du système. */
export function loadTheme(
  storage: Pick<Storage, 'getItem'> | undefined = globalThis.localStorage,
  prefersDark: boolean = globalThis.matchMedia?.('(prefers-color-scheme: dark)').matches ?? false,
): Theme {
  try {
    const stored = storage?.getItem(STORAGE_KEY);
    if (stored === 'day' || stored === 'night') return stored;
  } catch {
    // Stockage indisponible : préférence système.
  }
  return prefersDark ? 'night' : 'day';
}

export function saveTheme(
  theme: Theme,
  storage: Pick<Storage, 'setItem'> | undefined = globalThis.localStorage,
): void {
  try {
    storage?.setItem(STORAGE_KEY, theme);
  } catch {
    // Préférence non mémorisée : sans conséquence.
  }
}
