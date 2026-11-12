/**
 * État d'interface des routes, traces et de la navigation active.
 *
 * Orchestration seulement : chaque action délègue au moteur via
 * `RoutingApi`, puis recharge l'état. Aucun calcul de navigation.
 */
import type {
  ImportReport,
  LatLon,
  NavigationStatus,
  Route,
  RoutingApi,
  TrackSummary,
} from './services/routing';

/** Trace affichée sur la carte. */
export interface DisplayedTrack {
  id: string;
  points: LatLon[];
  recording: boolean;
}

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

export class NavigationController {
  routes = $state<Route[]>([]);
  tracks = $state<TrackSummary[]>([]);
  status = $state<NavigationStatus | null>(null);
  activeRouteId = $state<string | null>(null);
  recordingId = $state<string | null>(null);
  drawing = $state(false);
  draft = $state<LatLon[]>([]);
  /** Points des traces affichées, par identifiant. */
  shownTracks = $state<Record<string, LatLon[]>>({});
  error = $state<string | null>(null);
  notice = $state<string | null>(null);
  busy = $state(false);

  readonly displayedTracks: DisplayedTrack[] = $derived(
    Object.entries(this.shownTracks).map(([id, points]) => ({
      id,
      points,
      recording: id === this.recordingId,
    })),
  );

  constructor(private readonly api: RoutingApi) {}

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

  /**
   * Recharge routes, traces et état de navigation depuis le moteur. Ne
   * remplace pas l'erreur d'une action précédente (elle doit rester
   * visible), sauf si le rechargement échoue lui-même.
   */
  async refresh(): Promise<void> {
    try {
      const [routes, tracks, state] = await Promise.all([
        this.api.listRoutes(),
        this.api.listTracks(),
        this.api.navigationState(),
      ]);
      this.routes = routes;
      this.tracks = tracks;
      this.activeRouteId = state.activeRouteId;
      this.recordingId = state.recordingTrackId;
      if (state.activeRouteId === null) this.status = null;
      if (state.recordingTrackId !== null && !(state.recordingTrackId in this.shownTracks)) {
        this.shownTracks = { ...this.shownTracks, [state.recordingTrackId]: [] };
      }
    } catch (e) {
      this.error = message(e);
    }
  }

  /** État de navigation reçu du moteur (`null` : navigation désactivée). */
  onStatus(status: NavigationStatus | null): void {
    this.status = status;
    if (status?.reachedWaypoint) {
      this.notice = status.finished
        ? `Arrivée : ${status.reachedWaypoint}`
        : `Waypoint atteint : ${status.reachedWaypoint}`;
    }
  }

  // ------------------------------------------------------ tracé de route

  startDrawing(): void {
    this.drawing = true;
    this.draft = [];
    this.error = null;
  }

  addPoint(position: LatLon): void {
    if (this.drawing) this.draft = [...this.draft, position];
  }

  undoPoint(): void {
    this.draft = this.draft.slice(0, -1);
  }

  cancelDrawing(): void {
    this.drawing = false;
    this.draft = [];
  }

  async finishDrawing(name: string): Promise<boolean> {
    const created = await this.run(() => this.api.createRoute(name, this.draft));
    if (!created) return false;
    this.cancelDrawing();
    this.notice = `Route « ${created.name} » enregistrée`;
    await this.refresh();
    return true;
  }

  // ------------------------------------------------------------- routes

  async activate(id: string): Promise<void> {
    await this.run(() => this.api.activateRoute(id));
    await this.refresh();
  }

  async deactivate(): Promise<void> {
    await this.run(() => this.api.deactivateRoute());
    this.status = null;
    await this.refresh();
  }

  async reverse(id: string): Promise<void> {
    await this.run(() => this.api.reverseRoute(id));
    await this.refresh();
  }

  async deleteRoute(id: string): Promise<void> {
    await this.run(() => this.api.deleteRoute(id));
    await this.refresh();
  }

  async exportRoute(id: string): Promise<void> {
    if (await this.run(() => this.api.exportRouteGpx(id))) this.notice = 'Route exportée';
  }

  async importGpx(): Promise<ImportReport | undefined> {
    const report = await this.run(() => this.api.importGpx());
    if (!report) return undefined;
    this.notice =
      `Import : ${report.routes} route(s), ${report.tracks} trace(s), ` +
      `${report.waypoints} waypoint(s)` +
      (report.warnings.length > 0 ? ` — ${report.warnings.length} élément(s) écarté(s)` : '');
    await this.refresh();
    return report;
  }

  // ------------------------------------------------------------- traces

  async startRecording(): Promise<void> {
    const summary = await this.run(() => this.api.startRecording());
    if (summary) this.shownTracks = { ...this.shownTracks, [summary.id]: [] };
    await this.refresh();
  }

  async stopRecording(): Promise<void> {
    const id = this.recordingId;
    await this.run(() => this.api.stopRecording());
    if (id) await this.loadTrack(id);
    await this.refresh();
  }

  private async loadTrack(id: string): Promise<void> {
    const track = await this.run(() => this.api.getTrack(id));
    if (track) this.shownTracks = { ...this.shownTracks, [id]: track.points };
  }

  async toggleTrack(id: string): Promise<void> {
    if (id in this.shownTracks) {
      this.hideTrack(id);
    } else {
      await this.loadTrack(id);
    }
  }

  private hideTrack(id: string): void {
    this.shownTracks = Object.fromEntries(
      Object.entries(this.shownTracks).filter(([key]) => key !== id),
    );
  }

  /** Recharge la trace en cours d'enregistrement (affichage en direct). */
  async refreshRecording(): Promise<void> {
    if (this.recordingId) await this.loadTrack(this.recordingId);
  }

  async deleteTrack(id: string): Promise<void> {
    await this.run(() => this.api.deleteTrack(id));
    // `run` remet l'erreur à zéro : une erreur ici vient de la suppression.
    if (this.error === null) this.hideTrack(id);
    await this.refresh();
  }

  async exportTrack(id: string): Promise<void> {
    if (await this.run(() => this.api.exportTrackGpx(id))) this.notice = 'Trace exportée';
  }
}
