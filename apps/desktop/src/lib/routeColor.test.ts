import { describe, expect, it } from 'vitest';
import { DEFAULT_ACTIVE_ROUTE_COLOR, DEFAULT_TRACK_COLOR } from './map/overlays';
import {
  loadActiveRouteColor,
  loadTrackColor,
  parseColor,
  routeStyle,
  setActiveRouteColor,
  setTrackColor,
} from './routeColor.svelte';

const store = (value: string | null) => ({ getItem: () => value });

describe('couleur de la route active', () => {
  it('est jaune par défaut', () => {
    expect(DEFAULT_ACTIVE_ROUTE_COLOR).toBe('#ffd600');
    expect(loadActiveRouteColor(store(null))).toBe(DEFAULT_ACTIVE_ROUTE_COLOR);
  });

  it('relit une couleur mémorisée valide, ignore le reste', () => {
    expect(loadActiveRouteColor(store('#00E5FF'))).toBe('#00e5ff');
    for (const bad of ['red', '#fff', '#12345g', 'url(x)', '#1234567', '']) {
      expect(loadActiveRouteColor(store(bad)), bad).toBe(DEFAULT_ACTIVE_ROUTE_COLOR);
    }
    const throwing = {
      getItem: () => {
        throw new Error('stockage indisponible');
      },
    };
    expect(loadActiveRouteColor(throwing)).toBe(DEFAULT_ACTIVE_ROUTE_COLOR);
  });

  it('mémorise un changement valide et refuse une couleur invalide', () => {
    const saved: string[] = [];
    const storage = { setItem: (_: string, v: string) => saved.push(v) };
    expect(setActiveRouteColor('#D81B60', storage)).toBe(true);
    expect(routeStyle.activeColor).toBe('#d81b60');
    expect(setActiveRouteColor('rouge', storage)).toBe(false);
    expect(routeStyle.activeColor).toBe('#d81b60');
    expect(saved).toEqual(['#d81b60']);
    expect(parseColor(42)).toBeNull();
    setActiveRouteColor(DEFAULT_ACTIVE_ROUTE_COLOR, storage);
  });
});

describe('couleur des traces', () => {
  it('est rose par défaut', () => {
    expect(DEFAULT_TRACK_COLOR).toBe('#e91e63');
    expect(loadTrackColor(store(null))).toBe(DEFAULT_TRACK_COLOR);
    expect(routeStyle.trackColor).toBe(DEFAULT_TRACK_COLOR);
  });

  it('relit une couleur mémorisée valide, ignore le reste', () => {
    expect(loadTrackColor(store('#1565C0'))).toBe('#1565c0');
    for (const bad of ['rose', '#e91', '#e91e6z', '']) {
      expect(loadTrackColor(store(bad)), bad).toBe(DEFAULT_TRACK_COLOR);
    }
    const throwing = {
      getItem: () => {
        throw new Error('stockage indisponible');
      },
    };
    expect(loadTrackColor(throwing)).toBe(DEFAULT_TRACK_COLOR);
  });

  it('mémorise un changement valide, séparément de la route active', () => {
    const saved: [string, string][] = [];
    const storage = { setItem: (k: string, v: string) => saved.push([k, v]) };
    expect(setTrackColor('#1565C0', storage)).toBe(true);
    expect(routeStyle.trackColor).toBe('#1565c0');
    expect(routeStyle.activeColor).toBe(DEFAULT_ACTIVE_ROUTE_COLOR);
    expect(setTrackColor('bleu', storage)).toBe(false);
    expect(routeStyle.trackColor).toBe('#1565c0');
    expect(saved).toEqual([['nautik.trackColor', '#1565c0']]);
    const full = {
      setItem: () => {
        throw new Error('plein');
      },
    };
    expect(setTrackColor('#00e5ff', full)).toBe(true);
    expect(routeStyle.trackColor).toBe('#00e5ff');
    setTrackColor(DEFAULT_TRACK_COLOR, storage);
  });
});
