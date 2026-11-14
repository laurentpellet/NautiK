/**
 * Cellules ENC chargées côté interface : entités par cellule et cellules
 * en échec. Cache simple, non réactif (l'état affiché est porté par
 * `EncMapController`).
 */
import type { Cm93CellData, EncFeature, EncMapCell } from './services/enc';

/** Ce qu'il faut signaler d'une cellule CM93 chargée. */
export type Cm93CellInfo = Omit<Cm93CellData, 'features'>;

export class EncCellCache {
  /** Entités par chemin de cellule. */
  readonly features = new Map<string, EncFeature[]>();
  /** Cellules illisibles : chemin → raison. */
  readonly failed = new Map<string, string>();
  /** Cellules CM93 chargées : chemin (préfixé) → date et décalage déclaré. */
  readonly cm93Info = new Map<string, Cm93CellInfo>();

  /** Cellule déjà chargée ou déjà en échec. */
  known(path: string): boolean {
    return this.features.has(path) || this.failed.has(path);
  }

  clear(): void {
    this.features.clear();
    this.failed.clear();
    this.cm93Info.clear();
  }

  /** Garde au plus `max` cellules, dont toujours celles de `keep`. */
  evict(keep: EncMapCell[], max: number): void {
    if (this.features.size <= max) return;
    const kept = new Set(keep.map((c) => c.path));
    for (const path of [...this.features.keys()]) {
      if (this.features.size <= max) break;
      if (!kept.has(path)) {
        this.features.delete(path);
        this.cm93Info.delete(path);
      }
    }
  }
}
