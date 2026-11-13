/**
 * Plein écran de la fenêtre (Tauri) : la fenêtre couvre tout l'écran,
 * sans barre de titre ni barre des tâches. Seules les permissions
 * `set_fullscreen` et `is_fullscreen` de la fenêtre sont accordées.
 */
import { getCurrentWindow } from '@tauri-apps/api/window';

export interface FullscreenWindow {
  setFullscreen(on: boolean): Promise<void>;
  isFullscreen(): Promise<boolean>;
}

/** Fenêtre courante ; `null` hors de Tauri (tests, navigateur). */
export function currentWindow(): FullscreenWindow | null {
  try {
    return getCurrentWindow();
  } catch {
    return null;
  }
}

/**
 * Bascule le plein écran et retourne l'état effectif de la fenêtre
 * (l'état demandé si la fenêtre ne peut pas être interrogée).
 */
export async function applyFullscreen(win: FullscreenWindow | null, on: boolean): Promise<boolean> {
  if (!win) return on;
  await win.setFullscreen(on);
  try {
    return await win.isFullscreen();
  } catch {
    return on;
  }
}

/** Raccourcis : F11 bascule, Échap quitte le plein écran. */
export function fullscreenShortcut(key: string, fullscreen: boolean): boolean | null {
  if (key === 'F11') return !fullscreen;
  if (key === 'Escape' && fullscreen) return false;
  return null;
}
