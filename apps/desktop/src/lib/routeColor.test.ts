import { describe, expect, it } from 'vitest';
import { DEFAULT_ACTIVE_ROUTE_COLOR } from './map/overlays';
import {
  loadActiveRouteColor,
  parseColor,
  routeStyle,
  setActiveRouteColor,
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
