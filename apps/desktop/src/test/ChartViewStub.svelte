<script lang="ts">
  // Substitut de ChartView pour les tests (MapLibre requiert WebGL).
  import type { ChartCatalog } from '../lib/services/charts';

  let { catalog }: { catalog: ChartCatalog } = $props();

  export function recenter(): void {}
</script>

<div data-testid="chart-view-stub">{catalog.charts.length} cartes</div>
