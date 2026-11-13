import { describe, expect, it, vi } from 'vitest';
import { applyFullscreen, fullscreenShortcut, type FullscreenWindow } from './fullscreen';

describe('plein écran', () => {
  it('bascule la fenêtre et retourne son état effectif', async () => {
    let state = false;
    const win: FullscreenWindow = {
      setFullscreen: vi.fn((on: boolean) => {
        state = on;
        return Promise.resolve();
      }),
      isFullscreen: () => Promise.resolve(state),
    };
    expect(await applyFullscreen(win, true)).toBe(true);
    expect(win.setFullscreen).toHaveBeenCalledWith(true);
    expect(await applyFullscreen(win, false)).toBe(false);
  });

  it("retourne l'état demandé sans fenêtre ou si elle ne répond pas", async () => {
    expect(await applyFullscreen(null, true)).toBe(true);
    const win: FullscreenWindow = {
      setFullscreen: () => Promise.resolve(),
      isFullscreen: () => Promise.reject(new Error('refusé')),
    };
    expect(await applyFullscreen(win, true)).toBe(true);
  });

  it("propage l'échec de la bascule", async () => {
    const win: FullscreenWindow = {
      setFullscreen: () => Promise.reject(new Error('permission')),
      isFullscreen: () => Promise.resolve(false),
    };
    await expect(applyFullscreen(win, true)).rejects.toThrow('permission');
  });

  it('F11 bascule, Échap quitte seulement le plein écran', () => {
    expect(fullscreenShortcut('F11', false)).toBe(true);
    expect(fullscreenShortcut('F11', true)).toBe(false);
    expect(fullscreenShortcut('Escape', true)).toBe(false);
    expect(fullscreenShortcut('Escape', false)).toBeNull();
    expect(fullscreenShortcut('a', true)).toBeNull();
  });
});
