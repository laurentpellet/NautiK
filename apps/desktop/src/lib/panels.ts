/**
 * Disposition des panneaux de l'onglet Carte : chaque panneau peut être
 * déplacé et masqué ; le choix est mémorisé. Les alarmes et les alertes de
 * position ou de liaison ne sont jamais des panneaux : elles restent
 * toujours visibles.
 */

export type PanelId = 'side' | 'anchor' | 'live' | 'bar';

export interface PanelState {
  visible: boolean;
  /** Position déplacée (px, coin haut gauche, repère de la fenêtre) ; `null` : place par défaut. */
  x: number | null;
  y: number | null;
}

export type PanelLayout = Record<PanelId, PanelState>;

export const PANEL_IDS: PanelId[] = ['side', 'anchor', 'live', 'bar'];

export const PANEL_LABELS: Record<PanelId, string> = {
  side: 'Panneau',
  anchor: 'Mouillage',
  live: 'Navigation',
  bar: 'Barre',
};

const DEFAULT_STATE: PanelState = { visible: true, x: null, y: null };

export function defaultLayout(): PanelLayout {
  return Object.fromEntries(PANEL_IDS.map((id) => [id, { ...DEFAULT_STATE }])) as PanelLayout;
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
        visible: typeof e.visible === 'boolean' ? e.visible : true,
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

/** Panneaux masqués, pour le rappel affiché. */
export function hiddenPanels(layout: PanelLayout): string[] {
  return PANEL_IDS.filter((id) => !layout[id].visible).map((id) => PANEL_LABELS[id]);
}

/** Au moins un panneau a été déplacé ou masqué. */
export function isCustomized(layout: PanelLayout): boolean {
  return PANEL_IDS.some((id) => layout[id].x !== null || !layout[id].visible);
}
