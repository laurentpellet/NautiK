/**
 * Cartes ENC affichées : index des cellules, chargement des cellules de la
 * vue, données de la couche et avertissements.
 *
 * Orchestration seulement : décodage, mises à jour et catégories
 * d'affichage sont faits par le moteur. Aucune donnée n'est masquée en
 * silence : cellule pas à jour, annulée, illisible ou non affichée faute
 * de place est signalée.
 */
import { formatIssueDate } from './services/enc';
import type { Cm93View, EncApi, EncMapCell, EncMapIndex } from './services/enc';
import { EncCellCache, type Cm93CellInfo } from './encCells';
import type { Bounds } from './services/weather';
import {
  boundsIntersect,
  cellsForView,
  emptyEncCollection,
  encCollection,
  MAX_DISPLAYED_CELLS,
  type EncCollection,
} from './map/enc';
import type { VectorCharts } from './layers';

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** Nombre maximal de cellules gardées en mémoire côté interface. */
export const MAX_LOADED_CELLS = 48;

/** Avertissements sur les ENC de la vue. */
export function encWarnings(
  index: EncMapIndex | null,
  view: Bounds | null,
  displayed: EncMapCell[],
  failed: Map<string, string>,
): string[] {
  if (!index) return [];
  const warnings: string[] = [];
  if (index.error) warnings.push(`Dossier ENC inaccessible : ${index.error}`);
  if (index.truncated) warnings.push('Trop de cellules ENC : index incomplet');
  const inView = (c: EncMapCell) =>
    view !== null && c.bounds !== null && boundsIntersect(c.bounds, view);
  const all = index.cells.filter((c) => !c.error && !c.cancelled && inView(c));
  const cancelled = index.cells.filter((c) => c.cancelled && inView(c));
  const names = (cells: { name: string }[]) => cells.map((c) => c.name).join(', ');
  const outdated = displayed.filter((c) => c.updateError);
  if (outdated.length > 0) warnings.push(`ENC pas à jour : ${names(outdated)}`);
  if (cancelled.length > 0) warnings.push(`ENC annulée, non affichée : ${names(cancelled)}`);
  const broken = all.filter((c) => failed.has(c.path));
  if (broken.length > 0) warnings.push(`ENC illisible : ${names(broken)}`);
  const unreadable = index.cells.filter((c) => c.error);
  if (unreadable.length > 0) {
    warnings.push(`${unreadable.length} cellule(s) ENC illisible(s) dans le dossier`);
  }
  const left = all.length - Math.min(all.length, MAX_DISPLAYED_CELLS);
  if (left > 0) warnings.push(`${left} cellule(s) ENC non affichée(s) : zoomez`);
  return warnings;
}

/** Préfixe des cellules CM93 dans le cache (chemins distincts des ENC). */
export const CM93_KEY_PREFIX = 'cm93:';

/** État CM93 de la vue, pour les avertissements. */
export interface Cm93ViewState {
  /** Vue CM93 obtenue, ou `null` (pas de base, erreur). */
  view: Cm93View | null;
  /** Vue CM93 impossible à obtenir. */
  error: string | null;
  /** Cellules CM93 dessinées (chemins non préfixés). */
  displayed: EncMapCell[];
  /** Date et décalage des cellules dessinées. */
  info: Cm93CellInfo[];
  /** Cellules CM93 illisibles (noms). */
  failed: string[];
}

/**
 * Avertissements sur la base CM93 : carte non officielle et ancienne,
 * décalage WGS 84 déclaré non corrigé, cellules non affichées ou
 * illisibles. Rien n'est passé sous silence.
 */
export function cm93Warnings(index: EncMapIndex | null, state: Cm93ViewState): string[] {
  if (!index) return [];
  const warnings: string[] = [];
  if (index.cm93Error) warnings.push(`Base CM93 illisible : ${index.cm93Error}`);
  if (state.error) warnings.push(`Base CM93 indisponible : ${state.error}`);
  if (state.displayed.length > 0) {
    const dates = state.info.flatMap((i) => (i.dataDate ? [i.dataDate] : [])).sort();
    const age = dates.length > 0 ? ` (données de ${formatIssueDate(dates[0] ?? '')})` : '';
    warnings.push(`Cartes CM93 non officielles${age} : ne pas naviguer avec elles seules`);
    const offset = Math.max(0, ...state.info.map((i) => i.declaredOffsetM));
    if (offset >= 1) {
      warnings.push(`CM93 : décalage WGS 84 déclaré jusqu'à ${Math.round(offset)} m, non corrigé`);
    }
  }
  if (state.failed.length > 0) warnings.push(`CM93 illisible : ${state.failed.join(', ')}`);
  if (state.view?.truncated) warnings.push('Trop de cellules CM93 : index incomplet');
  const omitted = state.view?.omitted ?? 0;
  if (omitted > 0) warnings.push(`${omitted} cellule(s) CM93 non affichée(s) : zoomez`);
  return warnings;
}

export class EncMapController {
  index = $state<EncMapIndex | null>(null);
  /** Entités des ENC officielles (S-57) de la vue. */
  s57Data = $state.raw(emptyEncCollection());
  /** Entités de la base CM93 de la vue. */
  cm93Data = $state.raw(emptyEncCollection());
  /** Cellules dessinées, de la moins à la plus détaillée. */
  displayed = $state.raw<EncMapCell[]>([]);
  warnings = $state<string[]>([]);
  error = $state<string | null>(null);

  private readonly cache = new EncCellCache();
  private generation = 0;

  constructor(private readonly api: EncApi) {}

  /** Relit l'index (dossier changé, cellules mises à jour). */
  async refresh(): Promise<void> {
    try {
      this.index = await this.api.mapIndex();
      this.error = null;
    } catch (e) {
      this.index = null;
      this.error = message(e);
    }
    this.cache.clear();
  }

  /**
   * Couche d'un volet : CM93 sous les ENC officielles, selon les cartes
   * choisies pour ce volet.
   */
  layer(charts: VectorCharts): EncCollection {
    if (charts.enc && !charts.cm93) return this.s57Data;
    if (charts.cm93 && !charts.enc) return this.cm93Data;
    if (!charts.enc) return emptyEncCollection();
    return {
      type: 'FeatureCollection',
      features: [...this.cm93Data.features, ...this.s57Data.features],
    };
  }

  /**
   * Charge et prépare les cellules de la vue. `charts` : cartes voulues
   * dans au moins un volet ; les autres ne sont ni chargées ni signalées.
   */
  async show(view: Bounds | null, charts: VectorCharts = { enc: true, cm93: true }): Promise<void> {
    const generation = ++this.generation;
    if (!this.index) await this.refresh();
    if (generation !== this.generation) return;
    const index = this.index;
    const wanted = index && charts.enc ? cellsForView(index.cells, view) : [];
    for (const cell of wanted) {
      if (this.cache.known(cell.path)) continue;
      try {
        const features = await this.api.cellFeatures(cell.path);
        this.cache.features.set(cell.path, features);
      } catch (e) {
        this.cache.failed.set(cell.path, message(e));
      }
      if (generation !== this.generation) return;
    }
    const cm93 = await this.loadCm93(charts.cm93 ? index : null, view, generation);
    if (cm93 === null) return;
    this.cache.evict([...wanted, ...cm93.keyed], MAX_LOADED_CELLS);
    this.displayed = wanted.filter((c) => this.cache.features.has(c.path));
    this.s57Data = encCollection(this.displayed, this.cache.features);
    this.cm93Data = encCollection(cm93.keyed, this.cache.features);
    this.warnings = this.error
      ? [`Cartes ENC indisponibles : ${this.error}`]
      : [
          ...(charts.enc ? encWarnings(index, view, this.displayed, this.cache.failed) : []),
          ...(charts.cm93 ? cm93Warnings(index, cm93.state) : []),
        ];
  }

  /**
   * Charge les cellules CM93 de la vue. `keyed` : cellules dessinées, au
   * chemin préfixé du cache ; `null` si une vue plus récente a pris le relais.
   */
  private async loadCm93(
    index: EncMapIndex | null,
    view: Bounds | null,
    generation: number,
  ): Promise<{ keyed: EncMapCell[]; state: Cm93ViewState } | null> {
    const state: Cm93ViewState = { view: null, error: null, displayed: [], info: [], failed: [] };
    if (!index?.cm93 || !view) return { keyed: [], state };
    try {
      state.view = await this.api.cm93View(view);
    } catch (e) {
      state.error = message(e);
    }
    if (generation !== this.generation) return null;
    const keyed: EncMapCell[] = [];
    for (const cell of state.view?.cells ?? []) {
      const key = CM93_KEY_PREFIX + cell.path;
      if (!this.cache.known(key)) {
        try {
          const data = await this.api.cm93Features(cell.path);
          this.cache.features.set(key, data.features);
          this.cache.cm93Info.set(key, {
            dataDate: data.dataDate,
            declaredOffsetM: data.declaredOffsetM,
          });
        } catch (e) {
          this.cache.failed.set(key, message(e));
        }
        if (generation !== this.generation) return null;
      }
      const info = this.cache.cm93Info.get(key);
      if (this.cache.features.has(key) && info) {
        keyed.push({ ...cell, path: key });
        state.displayed.push(cell);
        state.info.push(info);
      } else if (this.cache.failed.has(key)) {
        state.failed.push(cell.name);
      }
    }
    return { keyed, state };
  }

  /** Masque la couche (les cellules chargées restent en mémoire). */
  hide(): void {
    this.generation++;
    this.displayed = [];
    this.s57Data = emptyEncCollection();
    this.cm93Data = emptyEncCollection();
    this.warnings = [];
  }
}
