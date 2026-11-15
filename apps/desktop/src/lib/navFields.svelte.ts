/**
 * Valeurs de la barre Navigation : lesquelles sont affichées et dans quel
 * ordre (Paramètres → Instruments).
 *
 * Préférence d'interface mémorisée localement, comme les unités. Une
 * valeur inconnue est ignorée, une valeur manquante (nouvelle version)
 * est ajoutée à la fin, affichée ; au moins une valeur reste affichée.
 */

/** Valeurs disponibles (identifiants des tuiles d'instruments). */
export const NAV_FIELDS = [
  { id: 'sog', label: 'SOG (vitesse fond)' },
  { id: 'heading', label: 'Cap' },
  { id: 'cog', label: 'COG (route fond)' },
  { id: 'aws', label: 'Vent apparent' },
  { id: 'depth', label: 'Fond' },
  { id: 'stw', label: 'Vitesse surface' },
  { id: 'awa', label: 'Angle du vent' },
  { id: 'rudder', label: 'Angle de barre' },
] as const;

export type NavFieldId = (typeof NAV_FIELDS)[number]['id'];

export interface NavField {
  id: NavFieldId;
  shown: boolean;
}

const STORAGE_KEY = 'nautik.navBar.fields';

const isFieldId = (value: unknown): value is NavFieldId => NAV_FIELDS.some((f) => f.id === value);

/** Ordre et choix par défaut : toutes les valeurs, dans l'ordre d'origine. */
export function defaultNavFields(): NavField[] {
  return NAV_FIELDS.map((f) => ({ id: f.id, shown: true }));
}

/** Liste validée ; illisible ou vide : liste par défaut. */
export function parseNavFields(value: unknown): NavField[] {
  if (!Array.isArray(value)) return defaultNavFields();
  const list: NavField[] = [];
  for (const entry of value as unknown[]) {
    if (typeof entry !== 'object' || entry === null) continue;
    const { id, shown } = entry as Record<string, unknown>;
    if (!isFieldId(id) || list.some((f) => f.id === id)) continue;
    list.push({ id, shown: shown !== false });
  }
  for (const f of NAV_FIELDS) {
    if (!list.some((e) => e.id === f.id)) list.push({ id: f.id, shown: true });
  }
  return list.some((f) => f.shown) ? list : defaultNavFields();
}

export function loadNavFields(
  storage: Pick<Storage, 'getItem'> | undefined = globalThis.localStorage,
): NavField[] {
  try {
    const raw = storage?.getItem(STORAGE_KEY);
    return raw ? parseNavFields(JSON.parse(raw)) : defaultNavFields();
  } catch {
    return defaultNavFields();
  }
}

/** Choix courant (réactif). */
export const navBar: { fields: NavField[] } = $state({ fields: loadNavFields() });

function commit(fields: NavField[], storage: Pick<Storage, 'setItem'> | undefined): void {
  navBar.fields = fields;
  try {
    storage?.setItem(STORAGE_KEY, JSON.stringify(fields));
  } catch {
    // Préférence non mémorisée : sans conséquence.
  }
}

/** Affiche ou masque une valeur ; refuse de masquer la dernière affichée. */
export function setNavFieldShown(
  id: NavFieldId,
  shown: boolean,
  storage: Pick<Storage, 'setItem'> | undefined = globalThis.localStorage,
): boolean {
  const next = navBar.fields.map((f) => (f.id === id ? { ...f, shown } : f));
  if (!next.some((f) => f.shown)) return false;
  commit(next, storage);
  return true;
}

/** Déplace une valeur d'un rang (`-1` : plus tôt, `1` : plus tard). */
export function moveNavField(
  id: NavFieldId,
  step: -1 | 1,
  storage: Pick<Storage, 'setItem'> | undefined = globalThis.localStorage,
): boolean {
  const from = navBar.fields.findIndex((f) => f.id === id);
  const to = from + step;
  const moved = navBar.fields[from];
  const other = navBar.fields[to];
  if (from < 0 || !moved || !other) return false;
  const next = [...navBar.fields];
  next[from] = other;
  next[to] = moved;
  commit(next, storage);
  return true;
}

/** Revient au choix par défaut. */
export function resetNavFields(
  storage: Pick<Storage, 'setItem'> | undefined = globalThis.localStorage,
): void {
  commit(defaultNavFields(), storage);
}

/** Identifiants affichés, dans l'ordre choisi. */
export function shownNavFields(fields: NavField[]): NavFieldId[] {
  return fields.filter((f) => f.shown).map((f) => f.id);
}

/** Libellé d'une valeur. */
export function navFieldLabel(id: NavFieldId): string {
  return NAV_FIELDS.find((f) => f.id === id)?.label ?? id;
}
