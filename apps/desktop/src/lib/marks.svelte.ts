/**
 * État d'interface des marques personnelles : liste, édition, import et
 * export KML. Orchestration seulement : la validation est faite par le
 * moteur, qui renvoie un message lisible en cas de refus.
 */
import type { Mark, MarkDraft, MarkIcon, MarksApi } from './services/marks';
import type { LatLon } from './services/validate';

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** Nom proposé pour une nouvelle marque : « Marque n » libre suivant. */
export function nextMarkName(marks: Pick<Mark, 'name'>[]): string {
  const used = marks.map((m) => m.name);
  let n = marks.length + 1;
  while (used.includes(`Marque ${n}`)) n += 1;
  return `Marque ${n}`;
}

export class MarksController {
  marks = $state.raw<Mark[]>([]);
  /** Marque en cours d'édition (création si `id` est `null`). */
  draft = $state<MarkDraft | null>(null);
  error = $state<string | null>(null);
  notice = $state<string | null>(null);
  busy = $state(false);

  constructor(private readonly api: MarksApi) {}

  private async run<T>(action: () => Promise<T>): Promise<T | undefined> {
    this.busy = true;
    this.error = null;
    try {
      return await action();
    } catch (e) {
      this.error = message(e);
      return undefined;
    } finally {
      this.busy = false;
    }
  }

  async refresh(): Promise<void> {
    try {
      this.marks = await this.api.list();
    } catch (e) {
      this.error = message(e);
    }
  }

  /** Nouvelle marque à une position (clic droit, centre de la carte…). */
  startNew(position: LatLon, icon: MarkIcon = 'flag'): void {
    this.notice = null;
    this.error = null;
    this.draft = { id: null, name: nextMarkName(this.marks), position, icon, note: '' };
  }

  edit(id: string): void {
    const mark = this.marks.find((m) => m.id === id);
    if (!mark) return;
    this.error = null;
    this.draft = {
      id: mark.id,
      name: mark.name,
      position: mark.position,
      icon: mark.icon,
      note: mark.note ?? '',
    };
  }

  cancel(): void {
    this.draft = null;
    this.error = null;
  }

  /** Enregistre la marque éditée ; reste en édition si le moteur refuse. */
  async save(): Promise<Mark | undefined> {
    const draft = this.draft;
    if (!draft) return undefined;
    const saved = await this.run(() => this.api.save($state.snapshot(draft) as MarkDraft));
    if (saved) {
      this.draft = null;
      this.notice = `Marque « ${saved.name} » enregistrée`;
      await this.refresh();
    }
    return saved;
  }

  async remove(id: string): Promise<void> {
    await this.run(() => this.api.delete(id));
    if (this.draft?.id === id) this.draft = null;
    await this.refresh();
  }

  async importKml(): Promise<void> {
    this.notice = null;
    const report = await this.run(() => this.api.importKml());
    if (!report) return;
    const warnings = report.warnings.length
      ? ` ; ${report.warnings.length} élément(s) écarté(s) : ${report.warnings.slice(0, 3).join(' ; ')}`
      : '';
    this.notice = `Import : ${report.imported} marque(s)${warnings}`;
    await this.refresh();
  }

  async exportKml(): Promise<void> {
    this.notice = null;
    if (await this.run(() => this.api.exportKml())) this.notice = 'Marques exportées';
  }
}

/** Demi-côté (degrés) du cadrage sur un point : environ 1 km. */
export const POINT_FRAME_DEG = 0.01;

/** Emprise de cadrage autour d'un point (marque, résultat de recherche). */
export function boundsAround(
  p: LatLon,
  half = POINT_FRAME_DEG,
): { west: number; south: number; east: number; north: number } {
  return {
    west: p.longitudeDeg - half,
    east: p.longitudeDeg + half,
    south: Math.max(-90, p.latitudeDeg - half),
    north: Math.min(90, p.latitudeDeg + half),
  };
}
