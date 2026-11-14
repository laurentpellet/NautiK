<script lang="ts">
  // Substitut de ChartView pour les tests (MapLibre requiert WebGL).
  import type { ChartCatalog } from '../lib/services/charts';
  import type { MapCamera } from '../lib/split';
  import type { Bounds, WindVector } from '../lib/services/weather';

  let {
    catalog,
    hiddenCharts = [],
    wind = null,
    onviewchange,
    camera = null,
    oncamera,
  }: {
    catalog: ChartCatalog;
    hiddenCharts?: string[];
    wind?: WindVector[] | null;
    onviewchange?: (bounds: Bounds) => void;
    camera?: MapCamera | null;
    oncamera?: (camera: MapCamera, user: boolean) => void;
  } = $props();

  // Emprise et vue fixes, comme après le chargement de la vraie carte.
  $effect(() => {
    onviewchange?.({ west: -150, south: -18, east: -146, north: -14 });
    oncamera?.({ center: [-148, -16], zoom: 7, bearing: 0 }, false);
  });

  export function recenter(): void {}
</script>

<div data-testid="chart-view-stub">{catalog.charts.length} cartes</div>
<div data-testid="hidden-charts-stub">{hiddenCharts.join(',')}</div>
<div data-testid="wind-stub">{wind === null ? 'sans vent' : `${wind.length} vents`}</div>
<div data-testid="camera-stub">
  {camera ? `${camera.center[0]},${camera.center[1]} z${camera.zoom}` : 'libre'}
</div>
<button
  type="button"
  onclick={() => oncamera?.({ center: [-147.7, -15], zoom: 12, bearing: 0 }, true)}
>
  Simuler un déplacement
</button>
