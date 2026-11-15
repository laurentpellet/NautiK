/**
 * État d'interface des routes, traces et de la navigation active.
 *
 * Orchestration seulement : chaque action délègue au moteur via
 * `RoutingApi`, puis recharge l'état. Aucun calcul de navigation : la
 * longueur, la durée, les heures de passage et les emprises viennent du
 * moteur.
 */
import type {
  GeoBounds,
  ImportReport,
  LatLon,
  NavigationStatus,
  Route,
  RoutePlan,
  RoutingApi,
  TrackSummary,
  WaypointEdit,
} from './services/routing';

/** Trace affichée sur la carte. */
export interface DisplayedTrack {
  id: string;
  points: LatLon[];
  recording: boolean;
}

/** Grandeurs d'une trace chargée (moteur). */
export interface TrackInfo {
  lengthM: number;
  durationS: number | null;
  bounds: GeoBounds | null;
}

/**
 * Où le prochain clic sur la carte ajoute un point : nouveau départ (avant
 * le premier), nouvelle arrivée (après le dernier, nommée « Arrivée »), en
 * fin de route ou à un rang donné.
 */
export type InsertMode = 'start' | 'arrival' | 'end' | number;

/** Route en cours d'édition sur la carte. */
export interface RouteEditor {
  /** Route modifiée ; `null` : nouvelle route. */
  routeId: string | null;
  name: string;
  points: WaypointEdit[];
  insert: InsertMode;
}

/** Demande de cadrage de la carte (le numéro distingue deux demandes identiques). */
export interface FocusRequest {
  bounds: GeoBounds;
  seq: number;
}

/** Vitesse prévue par défaut : 6 nœuds. */
export const DEFAULT_PLAN_SPEED_MPS = (6 * 1852) / 3600;
const PLAN_STORAGE_KEY = 'nautik.routePlan';
/** Bornes du moteur (nav-routing) : 0,1 à 30 m/s. */
const MIN_SPEED_MPS = 0.1;
const MAX_SPEED_MPS = 30;

/** Vitesse prévue mémorisée ; absente ou invalide : 6 nœuds. */
export function loadPlanSpeed(
  storage: Pick<Storage, 'getItem'> | undefined = globalThis.localStorage,
): number {
  try {
    const raw: unknown = JSON.parse(storage?.getItem(PLAN_STORAGE_KEY) ?? 'null');
    if (typeof raw !== 'object' || raw === null) return DEFAULT_PLAN_SPEED_MPS;
    const speed = (raw as Record<string, unknown>).speedMps;
    return typeof speed === 'number' && speed >= MIN_SPEED_MPS && speed <= MAX_SPEED_MPS
      ? speed
      : DEFAULT_PLAN_SPEED_MPS;
  } catch {
    return DEFAULT_PLAN_SPEED_MPS;
  }
}

function savePlanSpeed(
  speedMps: number,
  storage: Pick<Storage, 'setItem'> | undefined = globalThis.localStorage,
): void {
  try {
    storage?.setItem(PLAN_STORAGE_KEY, JSON.stringify({ speedMps }));
  } catch {
    // Préférence non mémorisée : sans conséquence.
  }
}

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

export class NavigationController {
  routes = $state<Route[]>([]);
  tracks = $state<TrackSummary[]>([]);
  status = $state<NavigationStatus | null>(null);
  activeRouteId = $state<string | null>(null);
  recordingId = $state<string | null>(null);
  /** Route en cours d'édition (création ou modification). */
  editor = $state<RouteEditor | null>(null);
  /** Route sélectionnée dans la liste (plan affiché). */
  selectedRouteId = $state<string | null>(null);
  /** Points des traces affichées, par identifiant. */
  shownTracks = $state<Record<string, LatLon[]>>({});
  /** Longueur, durée et emprise des traces chargées. */
  trackInfo = $state<Record<string, TrackInfo>>({});
  /** Dernière demande de cadrage de la carte. */
  focus = $state<FocusRequest | null>(null);
  /** Vitesse prévue (m/s) et heure de départ (ms UTC) du plan. */
  speedMps = $state(loadPlanSpeed());
  departure = $state<number | null>(null);
  /** Plan du parcours édité ou sélectionné (moteur). */
  plan = $state<RoutePlan | null>(null);
  planError = $state<string | null>(null);
  error = $state<string | null>(null);
  notice = $state<string | null>(null);
  busy = $state(false);

  private planSeq = 0;
  private focusSeq = 0;

  readonly displayedTracks: DisplayedTrack[] = $derived(
    Object.entries(this.shownTracks).map(([id, points]) => ({
      id,
      points,
      recording: id === this.recordingId,
    })),
  );

  /** Mode édition : les clics sur la carte ajoutent des points. */
  readonly drawing: boolean = $derived(this.editor !== null);
  /** Points de la route en édition. */
  readonly draft: WaypointEdit[] = $derived(this.editor?.points ?? []);
  readonly selectedRoute: Route | null = $derived(
    this.routes.find((r) => r.id === this.selectedRouteId) ?? null,
  );
  /** Positions du parcours planifié : édition en cours, sinon route sélectionnée. */
  readonly planPositions: LatLon[] = $derived(
    this.editor
      ? this.editor.points.map((p) => p.position)
      : (this.selectedRoute?.waypoints.map((w) => w.position) ?? []),
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
      if (this.selectedRouteId !== null && !routes.some((r) => r.id === this.selectedRouteId)) {
        this.selectedRouteId = null;
      }
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

  /** Cadre la carte sur une emprise. */
  zoomTo(bounds: GeoBounds | null): void {
    if (bounds) this.focus = { bounds, seq: ++this.focusSeq };
  }

  // ------------------------------------------------- édition de route

  /** Nouvelle route : les clics sur la carte ajoutent l'arrivée. */
  startDrawing(): void {
    this.editor = { routeId: null, name: '', points: [], insert: 'end' };
    this.error = null;
  }

  /** Modifie une route enregistrée (refusé par le moteur si elle est active). */
  editRoute(id: string): void {
    const route = this.routes.find((r) => r.id === id);
    if (!route) return;
    this.editor = {
      routeId: route.id,
      name: route.name,
      points: route.waypoints.map((w) => ({ id: w.id, name: w.name, position: w.position })),
      insert: 'end',
    };
    this.selectedRouteId = route.id;
    this.error = null;
    this.zoomTo(route.bounds);
  }

  /** Choisit où le prochain clic ajoute un point. */
  setInsert(mode: InsertMode): void {
    if (this.editor) this.editor.insert = mode;
  }

  /**
   * Ajoute un point à l'endroit choisi. Un départ, une arrivée ou un
   * point intermédiaire reçoit un nom par défaut modifiable ; après un
   * départ, une arrivée ou un point intermédiaire, les clics suivants
   * ajoutent à nouveau en fin de route.
   */
  addPoint(position: LatLon): void {
    const editor = this.editor;
    if (!editor) return;
    const points = [...editor.points];
    const mode = editor.insert;
    if (mode === 'start') {
      points.unshift({ id: null, name: 'Départ', position });
      editor.insert = 'end';
    } else if (mode === 'arrival') {
      points.push({ id: null, name: 'Arrivée', position });
      editor.insert = 'end';
    } else if (mode === 'end') {
      points.push({ id: null, name: points.length === 0 ? 'Départ' : '', position });
    } else {
      const index = Math.max(0, Math.min(mode, points.length));
      points.splice(index, 0, { id: null, name: '', position });
      editor.insert = 'end';
    }
    editor.points = points;
  }

  movePoint(index: number, position: LatLon): void {
    const editor = this.editor;
    if (!editor || !editor.points[index]) return;
    editor.points = editor.points.map((p, i) => (i === index ? { ...p, position } : p));
  }

  renamePoint(index: number, name: string): void {
    const editor = this.editor;
    if (!editor || !editor.points[index]) return;
    editor.points = editor.points.map((p, i) => (i === index ? { ...p, name } : p));
  }

  removePoint(index: number): void {
    const editor = this.editor;
    if (!editor || !editor.points[index]) return;
    editor.points = editor.points.filter((_, i) => i !== index);
  }

  undoPoint(): void {
    if (this.editor) this.editor.points = this.editor.points.slice(0, -1);
  }

  cancelDrawing(): void {
    this.editor = null;
  }

  /** Enregistre la route éditée ; l'édition reste ouverte en cas de refus. */
  async saveEdit(): Promise<boolean> {
    const editor = this.editor;
    if (!editor) return false;
    const saved = await this.run(() =>
      this.api.saveRoute(editor.routeId, editor.name, $state.snapshot(editor.points)),
    );
    if (!saved) return false;
    this.editor = null;
    this.selectedRouteId = saved.id;
    this.notice = `Route « ${saved.name} » enregistrée`;
    await this.refresh();
    return true;
  }

  // ------------------------------------------------------------- plan

  setSpeed(speedMps: number): void {
    if (!Number.isFinite(speedMps) || speedMps < MIN_SPEED_MPS || speedMps > MAX_SPEED_MPS) return;
    this.speedMps = speedMps;
    savePlanSpeed(speedMps);
  }

  setDeparture(departure: number | null): void {
    this.departure = departure !== null && Number.isFinite(departure) ? departure : null;
  }

  /**
   * Recalcule le plan (moteur) du parcours édité ou sélectionné. Une
   * réponse arrivée après une demande plus récente est ignorée.
   */
  async refreshPlan(): Promise<void> {
    const seq = ++this.planSeq;
    const positions = $state.snapshot(this.planPositions);
    if (positions.length < 2) {
      this.plan = null;
      this.planError = null;
      return;
    }
    try {
      const plan = await this.api.planRoute(positions, this.speedMps, this.departure);
      if (seq !== this.planSeq) return;
      this.plan = plan;
      this.planError = null;
    } catch (e) {
      if (seq !== this.planSeq) return;
      this.plan = null;
      this.planError = message(e);
    }
  }

  // ------------------------------------------------------------- routes

  /** Sélectionne une route (plan affiché) et cadre la carte dessus. */
  selectRoute(id: string | null): void {
    this.selectedRouteId = id;
    if (id !== null) this.zoomTo(this.routes.find((r) => r.id === id)?.bounds ?? null);
  }

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

  private async loadTrack(id: string): Promise<TrackInfo | undefined> {
    const track = await this.run(() => this.api.getTrack(id));
    if (!track) return undefined;
    const info = { lengthM: track.lengthM, durationS: track.durationS, bounds: track.bounds };
    this.shownTracks = { ...this.shownTracks, [id]: track.points };
    this.trackInfo = { ...this.trackInfo, [id]: info };
    return info;
  }

  /** Affiche (et cadre) ou masque une trace. */
  async toggleTrack(id: string): Promise<void> {
    if (id in this.shownTracks) {
      this.hideTrack(id);
    } else {
      this.zoomTo((await this.loadTrack(id))?.bounds ?? null);
    }
  }

  /** Affiche une trace et cadre la carte sur son emprise. */
  async zoomTrack(id: string): Promise<void> {
    const info = (await this.loadTrack(id)) ?? this.trackInfo[id];
    this.zoomTo(info?.bounds ?? null);
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

  async renameTrack(id: string, name: string): Promise<boolean> {
    await this.run(() => this.api.renameTrack(id, name));
    const ok = this.error === null;
    await this.refresh();
    return ok;
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
