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
  loadRouteBarCollapsed,
  loadNavCollapsed,
  loadNavDock,
  loadRouteBarSide,
  loadStatusPanelOpen,
  saveRouteBarCollapsed,
  saveNavCollapsed,
  saveNavDock,
  saveRouteBarSide,
  saveStatusPanelOpen,
  withNavigationShown,
  saveLayout,
} from './panels';

const store = (value: string | null) => ({ getItem: () => value });

describe('disposition des panneaux', () => {
  it('affiche tout à sa place par défaut ou si le stockage est invalide', () => {
    const d = defaultLayout();
    expect(Object.values(d).every((p) => p.x === null && p.y === null)).toBe(true);
    // Tout est affiché, sauf les cibles AIS (ouvertes depuis la barre).
    expect(PANEL_IDS.filter((id) => !d[id].visible)).toEqual(['ais']);
    expect(hiddenPanels(d)).toEqual([]);
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
          bar: { visible: false, x: 10.4, y: 20 },
          ais: { visible: 'non', x: 5, y: null },
          layers: { visible: false },
          anchor: { visible: true, x: Infinity, y: 3 },
          live: { visible: true, x: 1e9, y: 1 },
          inconnu: { visible: false },
        }),
      ),
    );
    expect(layout.bar).toEqual({ visible: false, x: 10, y: 20 });
    expect(layout.ais).toEqual({ visible: false, x: null, y: null });
    expect(layout.anchor).toEqual({ visible: true, x: null, y: null });
    expect(layout.live).toEqual({ visible: true, x: null, y: null });
    // Anciens panneaux Calques (devenu un menu) et latéral (supprimé) : ignorés.
    expect('layers' in layout).toBe(false);
    expect('side' in layout).toBe(false);
    expect(hiddenPanels(layout)).toEqual(['Barre']);
    expect(isCustomized(layout)).toBe(true);
  });

  it('ajoute le panneau Route et la barre de route, affichés par défaut', () => {
    const d = defaultLayout();
    expect(PANEL_IDS).toContain('route');
    expect(PANEL_IDS).toContain('routeBar');
    expect(d.route).toEqual({ visible: true, x: null, y: null });
    expect(d.routeBar).toEqual({ visible: true, x: null, y: null });
    expect(PANEL_LABELS.route).toBe('Édition sur la carte');
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

describe('barre de route réduite', () => {
  it('mémorise le choix, sans planter si le stockage échoue', () => {
    const data = new Map<string, string>();
    const storage = {
      getItem: (k: string) => data.get(k) ?? null,
      setItem: (k: string, v: string) => void data.set(k, v),
    };
    expect(loadRouteBarCollapsed(storage)).toBe(false);
    saveRouteBarCollapsed(true, storage);
    expect(loadRouteBarCollapsed(storage)).toBe(true);
    const broken = {
      getItem: () => {
        throw new Error('bloqué');
      },
      setItem: () => {
        throw new Error('bloqué');
      },
    };
    expect(loadRouteBarCollapsed(broken)).toBe(false);
    expect(() => saveRouteBarCollapsed(true, broken)).not.toThrow();
  });
});

describe('panneau d’état des données', () => {
  it('est fermé par défaut, mémorise le choix et résiste au stockage bloqué', () => {
    const data = new Map<string, string>();
    const storage = {
      getItem: (k: string) => data.get(k) ?? null,
      setItem: (k: string, v: string) => void data.set(k, v),
    };
    expect(loadStatusPanelOpen(storage)).toBe(false);
    saveStatusPanelOpen(true, storage);
    expect(loadStatusPanelOpen(storage)).toBe(true);
    saveStatusPanelOpen(false, storage);
    expect(loadStatusPanelOpen(storage)).toBe(false);
    const broken = {
      getItem: () => {
        throw new Error('bloqué');
      },
      setItem: () => {
        throw new Error('bloqué');
      },
    };
    expect(loadStatusPanelOpen(broken)).toBe(false);
    expect(() => saveStatusPanelOpen(true, broken)).not.toThrow();
  });
});

describe('disposition de lancement', () => {
  it('affiche toujours Navigation, sans toucher aux autres panneaux', () => {
    const saved = defaultLayout();
    saved.live = { ...saved.live, visible: false };
    saved.anchor = { ...saved.anchor, visible: false };
    const launch = withNavigationShown(saved);
    expect(launch.live.visible).toBe(true);
    expect(launch.anchor.visible).toBe(false);
    expect(saved.live.visible, 'disposition d’origine intacte').toBe(false);
  });
});

describe('positions des barres Navigation et route', () => {
  const memory = () => {
    const data = new Map<string, string>();
    return {
      data,
      getItem: (k: string) => data.get(k) ?? null,
      setItem: (k: string, v: string) => void data.set(k, v),
    };
  };

  it('Navigation en bas et route à gauche par défaut, choix mémorisés', () => {
    const storage = memory();
    expect(loadNavDock(storage)).toBe('bottom');
    expect(loadRouteBarSide(storage)).toBe('left');
    saveNavDock('left', storage);
    saveRouteBarSide('right', storage);
    expect(loadNavDock(storage)).toBe('left');
    expect(loadRouteBarSide(storage)).toBe('right');
  });

  it('ignore une valeur inconnue ou un stockage bloqué', () => {
    const storage = memory();
    storage.data.set('nautik.navDock', 'top');
    storage.data.set('nautik.routeBar.side', 'bottom');
    expect(loadNavDock(storage)).toBe('bottom');
    expect(loadRouteBarSide(storage)).toBe('left');
    const broken = {
      getItem: () => {
        throw new Error('bloqué');
      },
      setItem: () => {
        throw new Error('bloqué');
      },
    };
    expect(loadNavDock(broken)).toBe('bottom');
    expect(() => saveRouteBarSide('right', broken)).not.toThrow();
  });
});

describe('barre Navigation réduite', () => {
  it('dépliée par défaut, choix mémorisé, valeur inconnue ignorée', () => {
    const data = new Map<string, string>();
    const storage = {
      getItem: (k: string) => data.get(k) ?? null,
      setItem: (k: string, v: string) => void data.set(k, v),
    };
    expect(loadNavCollapsed(storage)).toBe(false);
    saveNavCollapsed(true, storage);
    expect(loadNavCollapsed(storage)).toBe(true);
    data.set('nautik.navBar.collapsed', 'oui');
    expect(loadNavCollapsed(storage)).toBe(false);
  });
});
