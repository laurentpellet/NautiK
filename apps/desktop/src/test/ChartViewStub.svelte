<script lang="ts">
  // Substitut de ChartView pour les tests (MapLibre requiert WebGL).
  import type { ChartCatalog } from '../lib/services/charts';
  import type { MapCamera } from '../lib/split';
  import type { Bounds, WindVector } from '../lib/services/weather';

  let {
    catalog,
    hiddenCharts = [],
    wind = null,
    enc,
    onviewchange,
    camera = null,
    oncamera,
    follow = false,
    onunfollow,
  }: {
    catalog: ChartCatalog;
    hiddenCharts?: string[];
    wind?: WindVector[] | null;
    enc?: { features: unknown[] };
    onviewchange?: (bounds: Bounds) => void;
    camera?: MapCamera | null;
    oncamera?: (camera: MapCamera, user: boolean) => void;
    follow?: boolean;
    onunfollow?: () => void;
  } = $props();

  // Emprise et vue fixes, comme après le chargement de la vraie carte.
  $effect(() => {
    onviewchange?.({ west: -150, south: -18, east: -146, north: -14 });
    oncamera?.({ center: [-148, -16], zoom: 7, bearing: 0 }, false);
  });

  export function recenter(): void {}

  /** Dernier cadrage demandé (centre), pour les tests. */
  let fitted = $state<string>('aucun cadrage');
  export function fitBounds(b: { west: number; south: number; east: number; north: number }): void {
    fitted = `${((b.west + b.east) / 2).toFixed(2)},${((b.south + b.north) / 2).toFixed(2)}`;
  }
</script>

<div data-testid="fit-stub">{fitted}</div>

<div data-testid="chart-view-stub">{catalog.charts.length} cartes</div>
<div data-testid="hidden-charts-stub">{hiddenCharts.join(',')}</div>
<div data-testid="enc-stub">{enc?.features.length ?? 0} entités ENC</div>
<div data-testid="wind-stub">{wind === null ? 'sans vent' : `${wind.length} vents`}</div>
<div data-testid="camera-stub">
  {camera ? `${camera.center[0]},${camera.center[1]} z${camera.zoom}` : 'libre'}
</div>
<div data-testid="follow-stub">{follow ? 'suivi' : 'sans suivi'}</div>
<button type="button" onclick={() => onunfollow?.()}>Déplacer la carte à la main</button>
<button
  type="button"
  onclick={() => oncamera?.({ center: [-147.7, -15], zoom: 12, bearing: 0 }, true)}
>
  Simuler un déplacement
</button>
