/**
 * Outils de la carte : mesure (distances et relèvements calculés par le
 * moteur), cercle de distance et ligne de relèvement (VRM/EBL) autour du
 * bateau, et « bateau ici » (place le bateau simulé, refusé par le moteur
 * hors du mode simulateur).
 *
 * Orchestration seulement : aucun calcul de distance en TypeScript.
 */
import type { LatLon, RoutePlan, RoutingApi } from './services/routing';
import type { SimulatorApi } from './services/simulator';

/** Outil courant : un clic sur la carte lui est destiné. */
export type MapTool = 'none' | 'measure' | 'placeBoat' | 'rangeBearing';

/** Repère VRM/EBL : rayon (m) et relèvement vrai (rad) depuis le bateau. */
export interface RangeBearingMark {
  radiusM: number | null;
  bearingRad: number | null;
}

/** Rayon maximal du VRM (m) : 100 milles. */
export const MAX_VRM_M = 100 * 1852;

/** Nombre maximal de points d'une mesure. */
export const MAX_MEASURE_POINTS = 100;

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

export class MapToolsController {
  tool = $state<MapTool>('none');
  /** Points cliqués de la mesure. */
  points = $state.raw<LatLon[]>([]);
  /** Distances et relèvements de la mesure (moteur) ; `null` sous deux points. */
  plan = $state.raw<RoutePlan | null>(null);
  error = $state<string | null>(null);
  notice = $state<string | null>(null);
  busy = $state(false);
  /** Repère VRM/EBL (outil « relèvement et distance »). */
  rangeBearing = $state.raw<RangeBearingMark>({ radiusM: null, bearingRad: null });
  private seq = 0;

  /** Un clic sur la carte est destiné à un outil (pas à la carte). */
  readonly picking: boolean = $derived(this.tool !== 'none');
  readonly measuring: boolean = $derived(this.tool === 'measure');

  constructor(
    private readonly routing: RoutingApi,
    private readonly simulator: SimulatorApi,
    /** Vitesse prévue (m/s) pour la durée de la mesure. */
    private readonly speedMps: () => number,
    /** Position fraîche du bateau (centre du VRM/EBL). */
    private readonly boat: () => LatLon | null = () => null,
  ) {}

  /** Active ou quitte l'outil de mesure. */
  toggleMeasure(): void {
    if (this.tool === 'measure') this.close();
    else this.select('measure');
  }

  /** Active ou quitte l'outil « bateau ici ». */
  togglePlaceBoat(): void {
    if (this.tool === 'placeBoat') this.close();
    else this.select('placeBoat');
  }

  /** Active ou quitte l'outil VRM/EBL (repère effacé en le quittant). */
  toggleRangeBearing(): void {
    if (this.tool === 'rangeBearing') this.close();
    else this.select('rangeBearing');
  }

  private select(tool: MapTool): void {
    this.close();
    this.tool = tool;
  }

  /** Quitte l'outil courant ; la mesure est effacée. */
  close(): void {
    this.tool = 'none';
    this.error = null;
    this.clearMeasure();
    this.rangeBearing = { radiusM: null, bearingRad: null };
  }

  /** Clic sur la carte pendant qu'un outil est actif. */
  async click(position: LatLon): Promise<void> {
    if (this.tool === 'measure') await this.addPoint(position);
    else if (this.tool === 'placeBoat') await this.placeBoat(position);
    else if (this.tool === 'rangeBearing') await this.pointRangeBearing(position);
  }

  /** Commence une nouvelle mesure à `position` (menu contextuel). */
  async measureFrom(position: LatLon): Promise<void> {
    this.select('measure');
    await this.addPoint(position);
  }

  async addPoint(position: LatLon): Promise<void> {
    if (this.points.length >= MAX_MEASURE_POINTS) {
      this.error = `Mesure limitée à ${MAX_MEASURE_POINTS} points.`;
      return;
    }
    this.points = [...this.points, position];
    await this.refresh();
  }

  async undoPoint(): Promise<void> {
    this.points = this.points.slice(0, -1);
    this.error = null;
    await this.refresh();
  }

  clearMeasure(): void {
    this.seq += 1;
    this.points = [];
    this.plan = null;
  }

  /** Recalcule la mesure (moteur) ; une réponse périmée est ignorée. */
  async refresh(): Promise<void> {
    const seq = ++this.seq;
    const points = this.points;
    if (points.length < 2) {
      this.plan = null;
      return;
    }
    try {
      const plan = await this.routing.planRoute(points, this.speedMps(), null);
      if (seq !== this.seq) return;
      this.plan = plan;
      this.error = null;
    } catch (e) {
      if (seq !== this.seq) return;
      this.plan = null;
      this.error = `Mesure indisponible : ${message(e)}`;
    }
  }

  /**
   * Fait passer le VRM et l'EBL par `position` : distance et relèvement
   * depuis le bateau, calculés par le moteur.
   */
  async pointRangeBearing(position: LatLon): Promise<void> {
    const boat = this.boat();
    if (!boat) {
      this.error = 'Position du bateau inconnue ou obsolète : VRM/EBL indisponible.';
      return;
    }
    const seq = ++this.seq;
    try {
      const plan = await this.routing.planRoute([boat, position], this.speedMps(), null);
      if (seq !== this.seq) return;
      const leg = plan.legs[0];
      if (!leg) return;
      this.rangeBearing = { radiusM: leg.distanceM, bearingRad: leg.bearingRad };
      this.error = null;
    } catch (e) {
      if (seq !== this.seq) return;
      this.error = `Relèvement indisponible : ${message(e)}`;
    }
  }

  /** Règle le VRM et l'EBL à la main (valeurs validées). */
  setRangeBearing(mark: RangeBearingMark): void {
    const radius = mark.radiusM;
    const bearing = mark.bearingRad;
    if (radius !== null && (!Number.isFinite(radius) || radius <= 0 || radius > MAX_VRM_M)) {
      this.error = `Rayon invalide : de 0 à ${MAX_VRM_M / 1852} milles.`;
      return;
    }
    if (bearing !== null && !Number.isFinite(bearing)) {
      this.error = 'Relèvement invalide.';
      return;
    }
    this.error = null;
    this.rangeBearing = {
      radiusM: radius,
      bearingRad:
        bearing === null ? null : ((bearing % (2 * Math.PI)) + 2 * Math.PI) % (2 * Math.PI),
    };
  }

  /** Place le bateau simulé ; l'outil se referme après un succès. */
  async placeBoat(position: LatLon): Promise<boolean> {
    this.busy = true;
    this.error = null;
    try {
      await this.simulator.placeBoat(position);
      if (this.tool === 'placeBoat') this.tool = 'none';
      this.notice = 'Bateau simulé déplacé.';
      return true;
    } catch (e) {
      this.error = message(e);
      return false;
    } finally {
      this.busy = false;
    }
  }
}
