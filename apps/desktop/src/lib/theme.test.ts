import { describe, expect, it } from 'vitest';
import { loadTheme, saveTheme } from './theme';

describe('thème', () => {
  it('restaure le thème mémorisé', () => {
    expect(loadTheme({ getItem: () => 'night' }, false)).toBe('night');
    expect(loadTheme({ getItem: () => 'day' }, true)).toBe('day');
  });

  it('suit la préférence système à défaut', () => {
    expect(loadTheme({ getItem: () => null }, true)).toBe('night');
    expect(loadTheme({ getItem: () => 'invalide' }, false)).toBe('day');
    expect(loadTheme(undefined, false)).toBe('day');
  });

  it('tolère un stockage indisponible', () => {
    const throwing = {
      getItem: () => {
        throw new Error('refusé');
      },
      setItem: () => {
        throw new Error('refusé');
      },
    };
    expect(loadTheme(throwing, true)).toBe('night');
    expect(() => saveTheme('day', throwing)).not.toThrow();
  });

  it('mémorise le thème', () => {
    const saved: Record<string, string> = {};
    saveTheme('night', { setItem: (k, v) => (saved[k] = v) });
    expect(Object.values(saved)).toEqual(['night']);
  });
});
