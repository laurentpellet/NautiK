/**
 * Disposition des panneaux de l'onglet Carte : chaque panneau peut être
 * déplacé et masqué ; le choix est mémorisé. Les alarmes et les alertes de
 * position ou de liaison ne sont jamais des panneaux : elles restent
 * toujours visibles.
 */

export type PanelId = 'side' | 'anchor' | 'live' | 'ais' | 'bar' | 'route' | 'routeBar';

export interface PanelState {
  visible: boolean;
  /** Position déplacée (px, coin haut gauche, repère de la fenêtre) ; `null` : place par défaut. */
  x: number | null;
  y: number | null;
}

export type PanelLayout = Record<PanelId, PanelState>;

export const PANEL_IDS: PanelId[] = ['side', 'anchor', 'live', 'ais', 'bar', 'route', 'routeBar'];

export const PANEL_LABELS: Record<PanelId, string> = {
  side: 'Panneau',
  anchor: 'Mouillage',
  live: 'Navigation',
  ais: 'Cibles AIS',
  bar: 'Barre',
  route: 'Routes et traces',
  routeBar: 'Barre de route',
};

const DEFAULT_STATE: PanelState = { visible: true, x: null, y: null };

/** Panneaux masqués par défaut : ouverts depuis la barre d'outils. */
const HIDDEN_BY_DEFAULT: readonly PanelId[] = ['ais'];

const visibleByDefault = (id: PanelId) => !HIDDEN_BY_DEFAULT.includes(id);

export function defaultLayout(): PanelLayout {
  return Object.fromEntries(
    PANEL_IDS.map((id) => [id, { ...DEFAULT_STATE, visible: visibleByDefault(id) }]),
  ) as PanelLayout;
}

const STORAGE_KEY = 'nautik.panels';

/** Coordonnée acceptable : nombre fini, bornes larges (écrans multiples). */
function coordinate(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) && Math.abs(value) <= 100_000
    ? Math.round(value)
    : null;
}

/** Disposition mémorisée ; valeurs absentes ou invalides : par défaut. */
export function loadLayout(
  storage: Pick<Storage, 'getItem'> | undefined = globalThis.localStorage,
): PanelLayout {
  const layout = defaultLayout();
  try {
    const raw = storage?.getItem(STORAGE_KEY);
    if (!raw) return layout;
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== 'object' || parsed === null) return layout;
    const record = parsed as Record<string, unknown>;
    for (const id of PANEL_IDS) {
      const entry = record[id];
      if (typeof entry !== 'object' || entry === null) continue;
      const e = entry as Record<string, unknown>;
      const x = coordinate(e.x);
      const y = coordinate(e.y);
      layout[id] = {
        visible: typeof e.visible === 'boolean' ? e.visible : visibleByDefault(id),
        // Une position n'a de sens que complète.
        x: x !== null && y !== null ? x : null,
        y: x !== null && y !== null ? y : null,
      };
    }
    return layout;
  } catch {
    return defaultLayout();
  }
}

export function saveLayout(
  layout: PanelLayout,
  storage: Pick<Storage, 'setItem'> | undefined = globalThis.localStorage,
): void {
  try {
    storage?.setItem(STORAGE_KEY, JSON.stringify(layout));
  } catch {
    // Disposition non mémorisée : sans conséquence.
  }
}

export interface Rect {
  left: number;
  top: number;
  width: number;
  height: number;
}

/** Partie d'un panneau qui doit toujours rester visible (poignée). */
export const KEEP_VISIBLE_PX = 48;

/**
 * Position bornée : la poignée du panneau reste dans la zone de la carte,
 * pour qu'un panneau ne puisse jamais être perdu hors de l'écran.
 */
export function clampPosition(
  x: number,
  y: number,
  panel: { width: number },
  area: Rect,
): { x: number; y: number } {
  const minX = area.left - panel.width + KEEP_VISIBLE_PX;
  const maxX = area.left + area.width - KEEP_VISIBLE_PX;
  const minY = area.top;
  const maxY = area.top + area.height - KEEP_VISIBLE_PX / 2;
  return {
    x: Math.round(Math.min(Math.max(x, minX), Math.max(minX, maxX))),
    y: Math.round(Math.min(Math.max(y, minY), Math.max(minY, maxY))),
  };
}

/** Panneaux masqués (hors ceux masqués par défaut), pour le rappel affiché. */
export function hiddenPanels(layout: PanelLayout): string[] {
  return PANEL_IDS.filter((id) => !layout[id].visible && visibleByDefault(id)).map(
    (id) => PANEL_LABELS[id],
  );
}

/** Au moins un panneau a été déplacé, ou affiché / masqué autrement que par défaut. */
export function isCustomized(layout: PanelLayout): boolean {
  return PANEL_IDS.some(
    (id) => layout[id].x !== null || layout[id].visible !== visibleByDefault(id),
  );
}

const ROUTE_BAR_COLLAPSED_KEY = 'nautik.routeBar.collapsed';

/** Barre de route réduite (préférence d'interface mémorisée). */
export function loadRouteBarCollapsed(
  storage: Pick<Storage, 'getItem'> | undefined = globalThis.localStorage,
): boolean {
  try {
    return storage?.getItem(ROUTE_BAR_COLLAPSED_KEY) === 'true';
  } catch {
    return false;
  }
}

export function saveRouteBarCollapsed(
  collapsed: boolean,
  storage: Pick<Storage, 'setItem'> | undefined = globalThis.localStorage,
): void {
  try {
    storage?.setItem(ROUTE_BAR_COLLAPSED_KEY, String(collapsed));
  } catch {
    // Préférence non mémorisée : sans conséquence.
  }
}

const STATUS_PANEL_OPEN_KEY = 'nautik.statusPanel.open';

/** Panneau d'état des données ouvert (préférence d'interface mémorisée). */
export function loadStatusPanelOpen(
  storage: Pick<Storage, 'getItem'> | undefined = globalThis.localStorage,
): boolean {
  try {
    return storage?.getItem(STATUS_PANEL_OPEN_KEY) === 'true';
  } catch {
    return false;
  }
}

export function saveStatusPanelOpen(
  open: boolean,
  storage: Pick<Storage, 'setItem'> | undefined = globalThis.localStorage,
): void {
  try {
    storage?.setItem(STATUS_PANEL_OPEN_KEY, String(open));
  } catch {
    // Préférence non mémorisée : sans conséquence.
  }
}

/**
 * Disposition de lancement : le panneau Navigation (bandeau du bas) est
 * toujours affiché au démarrage, même masqué lors de la session
 * précédente.
 */
export function withNavigationShown(layout: PanelLayout): PanelLayout {
  return { ...layout, live: { ...layout.live, visible: true } };
}

/** Position de la barre Navigation : à gauche, en bas ou à droite de la carte. */
export type NavDock = 'left' | 'bottom' | 'right';
/** Côté de la carte où se place la barre de route. */
export type RouteBarSide = 'left' | 'right';

export const NAV_DOCKS: readonly NavDock[] = ['left', 'bottom', 'right'];
export const ROUTE_BAR_SIDES: readonly RouteBarSide[] = ['left', 'right'];

const NAV_DOCK_KEY = 'nautik.navDock';
const ROUTE_BAR_SIDE_KEY = 'nautik.routeBar.side';

function loadChoice<T extends string>(
  key: string,
  allowed: readonly T[],
  fallback: T,
  storage: Pick<Storage, 'getItem'> | undefined,
): T {
  try {
    const value = storage?.getItem(key);
    return allowed.find((a) => a === value) ?? fallback;
  } catch {
    return fallback;
  }
}

function saveChoice(key: string, value: string, storage: Pick<Storage, 'setItem'> | undefined) {
  try {
    storage?.setItem(key, value);
  } catch {
    // Préférence non mémorisée : sans conséquence.
  }
}

/** Position mémorisée de la barre Navigation (en bas par défaut). */
export function loadNavDock(
  storage: Pick<Storage, 'getItem'> | undefined = globalThis.localStorage,
): NavDock {
  return loadChoice(NAV_DOCK_KEY, NAV_DOCKS, 'bottom', storage);
}

export function saveNavDock(
  dock: NavDock,
  storage: Pick<Storage, 'setItem'> | undefined = globalThis.localStorage,
): void {
  saveChoice(NAV_DOCK_KEY, dock, storage);
}

/** Côté mémorisé de la barre de route (à gauche par défaut). */
export function loadRouteBarSide(
  storage: Pick<Storage, 'getItem'> | undefined = globalThis.localStorage,
): RouteBarSide {
  return loadChoice(ROUTE_BAR_SIDE_KEY, ROUTE_BAR_SIDES, 'left', storage);
}

export function saveRouteBarSide(
  side: RouteBarSide,
  storage: Pick<Storage, 'setItem'> | undefined = globalThis.localStorage,
): void {
  saveChoice(ROUTE_BAR_SIDE_KEY, side, storage);
}

const NAV_COLLAPSED_KEY = 'nautik.navBar.collapsed';

/** Barre Navigation réduite (préférence mémorisée, dépliée par défaut). */
export function loadNavCollapsed(
  storage: Pick<Storage, 'getItem'> | undefined = globalThis.localStorage,
): boolean {
  return loadChoice(NAV_COLLAPSED_KEY, ['true', 'false'] as const, 'false', storage) === 'true';
}

export function saveNavCollapsed(
  collapsed: boolean,
  storage: Pick<Storage, 'setItem'> | undefined = globalThis.localStorage,
): void {
  saveChoice(NAV_COLLAPSED_KEY, String(collapsed), storage);
}
