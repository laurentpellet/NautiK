<script lang="ts">
  // Substitut de ChartView pour les tests (MapLibre requiert WebGL).
  import type { ChartCatalog } from '../lib/services/charts';
  import type { Bounds, WindVector } from '../lib/services/weather';

  let {
    catalog,
    wind = null,
    onviewchange,
  }: {
    catalog: ChartCatalog;
    wind?: WindVector[] | null;
    onviewchange?: (bounds: Bounds) => void;
  } = $props();

  // Emprise fixe, comme après le chargement de la vraie carte.
  $effect(() => {
    onviewchange?.({ west: -150, south: -18, east: -146, north: -14 });
  });

  export function recenter(): void {}
</script>

<div data-testid="chart-view-stub">{catalog.charts.length} cartes</div>
<div data-testid="wind-stub">{wind === null ? 'sans vent' : `${wind.length} vents`}</div>
