import { describe, expect, it } from 'vitest';
import {
  KEEP_VISIBLE_PX,
  PANEL_IDS,
  PANEL_LABELS,
  clampPosition,
  defaultLayout,
  hiddenPanels,
  isCustomized,
  loadLayout,
  saveLayout,
} from './panels';

const store = (value: string | null) => ({ getItem: () => value });

describe('disposition des panneaux', () => {
  it('affiche tout à sa place par défaut ou si le stockage est invalide', () => {
    const d = defaultLayout();
    expect(Object.values(d).every((p) => p.visible && p.x === null && p.y === null)).toBe(true);
    expect(loadLayout(store(null))).toEqual(d);
    expect(loadLayout(store('{pas du json'))).toEqual(d);
    expect(loadLayout(store('[1]'))).toEqual(d);
    expect(
      loadLayout({
        getItem: () => {
          throw new Error('bloqué');
        },
      }),
    ).toEqual(d);
    expect(isCustomized(d)).toBe(false);
  });

  it('relit une disposition mémorisée et écarte les valeurs invalides', () => {
    const layout = loadLayout(
      store(
        JSON.stringify({
          side: { visible: false, x: 10.4, y: 20 },
          bar: { visible: 'non', x: 5, y: null },
          layers: { visible: false },
          anchor: { visible: true, x: Infinity, y: 3 },
          live: { visible: true, x: 1e9, y: 1 },
          inconnu: { visible: false },
        }),
      ),
    );
    expect(layout.side).toEqual({ visible: false, x: 10, y: 20 });
    expect(layout.anchor).toEqual({ visible: true, x: null, y: null });
    expect(layout.live).toEqual({ visible: true, x: null, y: null });
    expect(layout.bar).toEqual({ visible: true, x: null, y: null });
    // Ancien panneau Calques (devenu un menu de la barre) : ignoré.
    expect('layers' in layout).toBe(false);
    expect(hiddenPanels(layout)).toEqual(['Panneau']);
    expect(isCustomized(layout)).toBe(true);
  });

  it('ajoute le panneau Route et la barre de route, affichés par défaut', () => {
    const d = defaultLayout();
    expect(PANEL_IDS).toContain('route');
    expect(PANEL_IDS).toContain('routeBar');
    expect(d.route).toEqual({ visible: true, x: null, y: null });
    expect(d.routeBar).toEqual({ visible: true, x: null, y: null });
    expect(PANEL_LABELS.route).toBe('Route');
    expect(PANEL_LABELS.routeBar).toBe('Barre de route');
    // Disposition mémorisée avant leur ajout : ils sont affichés à leur place.
    const old = loadLayout(store(JSON.stringify({ side: { visible: false, x: null, y: null } })));
    expect(old.route).toEqual({ visible: true, x: null, y: null });
    expect(old.routeBar).toEqual({ visible: true, x: null, y: null });
    const hidden = loadLayout(
      store(JSON.stringify({ routeBar: { visible: false, x: 12, y: 34 } })),
    );
    expect(hidden.routeBar).toEqual({ visible: false, x: 12, y: 34 });
    expect(hiddenPanels(hidden)).toEqual(['Barre de route']);
  });

  it('mémorise la disposition et tolère un stockage indisponible', () => {
    let saved = '';
    const layout = defaultLayout();
    layout.bar = { visible: true, x: 1, y: 2 };
    saveLayout(layout, { setItem: (_k: string, v: string) => (saved = v) });
    expect(loadLayout(store(saved))).toEqual(layout);
    expect(() =>
      saveLayout(layout, {
        setItem: () => {
          throw new Error('plein');
        },
      }),
    ).not.toThrow();
  });

  it('garde toujours la poignée du panneau dans la zone de la carte', () => {
    const area = { left: 0, top: 50, width: 1000, height: 700 };
    const panel = { width: 300 };
    expect(clampPosition(100, 200, panel, area)).toEqual({ x: 100, y: 200 });
    expect(clampPosition(5000, 5000, panel, area)).toEqual({
      x: 1000 - KEEP_VISIBLE_PX,
      y: 750 - KEEP_VISIBLE_PX / 2,
    });
    expect(clampPosition(-5000, -5000, panel, area)).toEqual({
      x: -300 + KEEP_VISIBLE_PX,
      y: 50,
    });
    // Zone minuscule : position cohérente quand même.
    const tiny = clampPosition(10, 10, panel, { left: 0, top: 0, width: 10, height: 10 });
    expect(Number.isFinite(tiny.x) && Number.isFinite(tiny.y)).toBe(true);
  });
});
