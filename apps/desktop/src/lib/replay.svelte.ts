/**
 * Relecture d'une trace : profil chargé depuis le moteur (statistiques,
 * vitesse de chaque point), curseur et lecture accélérée.
 *
 * Aucune grandeur n'est calculée ici : le curseur choisit seulement le
 * point enregistré affiché.
 */
import type { ProfilePoint, TrackProfile } from './services/trackProfile';

/** Accélérations proposées (temps de trace par seconde réelle). */
export const REPLAY_RATES = [10, 60, 300, 1800] as const;
export type ReplayRate = (typeof REPLAY_RATES)[number];
/** Période de rafraîchissement de la lecture (ms). */
export const REPLAY_TICK_MS = 100;

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** Dernier point enregistré à l'instant `at` ou avant (recherche dichotomique). */
export function indexAt(points: Pick<ProfilePoint, 'at'>[], at: number): number {
  let lo = 0;
  let hi = points.length - 1;
  if (hi < 0) return -1;
  while (lo < hi) {
    const mid = Math.ceil((lo + hi) / 2);
    if ((points[mid]?.at ?? Infinity) <= at) lo = mid;
    else hi = mid - 1;
  }
  return lo;
}

export class ReplayController {
  trackId = $state<string | null>(null);
  trackName = $state('');
  profile = $state.raw<TrackProfile | null>(null);
  index = $state(0);
  playing = $state(false);
  rate = $state<ReplayRate>(60);
  error = $state<string | null>(null);
  busy = $state(false);

  readonly current: ProfilePoint | null = $derived(this.profile?.points[this.index] ?? null);
  readonly active: boolean = $derived(this.trackId !== null);

  private timer: ReturnType<typeof setInterval> | null = null;
  private clock = 0;
  private loadSeq = 0;

  constructor(private readonly fetchProfile: (id: string) => Promise<TrackProfile>) {}

  async load(id: string, name: string): Promise<void> {
    this.stop();
    const seq = ++this.loadSeq;
    this.trackId = id;
    this.trackName = name;
    this.profile = null;
    this.index = 0;
    this.error = null;
    this.busy = true;
    try {
      const profile = await this.fetchProfile(id);
      if (seq !== this.loadSeq) return;
      this.profile = profile;
    } catch (e) {
      if (seq === this.loadSeq) this.error = message(e);
    } finally {
      if (seq === this.loadSeq) this.busy = false;
    }
  }

  close(): void {
    this.stop();
    this.loadSeq += 1;
    this.trackId = null;
    this.profile = null;
    this.index = 0;
    this.error = null;
    this.busy = false;
  }

  setIndex(index: number): void {
    const n = this.profile?.points.length ?? 0;
    if (n === 0 || !Number.isFinite(index)) return;
    this.index = Math.min(Math.max(Math.round(index), 0), n - 1);
    this.clock = this.current?.at ?? 0;
  }

  setRate(rate: number): void {
    if ((REPLAY_RATES as readonly number[]).includes(rate)) this.rate = rate as ReplayRate;
  }

  toggle(): void {
    if (this.playing) this.stop();
    else this.play();
  }

  play(): void {
    const points = this.profile?.points ?? [];
    if (points.length < 2) return;
    // Arrivé au bout : on recommence.
    if (this.index >= points.length - 1) this.index = 0;
    this.clock = points[this.index]?.at ?? 0;
    this.playing = true;
    this.timer = setInterval(() => this.advance(REPLAY_TICK_MS), REPLAY_TICK_MS);
  }

  stop(): void {
    if (this.timer !== null) clearInterval(this.timer);
    this.timer = null;
    this.playing = false;
  }

  /** Avance la lecture de `elapsedMs` de temps réel. */
  advance(elapsedMs: number): void {
    const points = this.profile?.points ?? [];
    if (!this.playing || points.length === 0) return;
    this.clock += elapsedMs * this.rate;
    this.index = indexAt(points, this.clock);
    if (this.index >= points.length - 1) this.stop();
  }
}
