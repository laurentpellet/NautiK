<script lang="ts">
  // Courbe historique (une ou plusieurs séries) : échelle, heures UTC,
  // moyenne et extrêmes de la première série. Les trous restent vides.
  import { inWindow, trendPath, trendStats, type Sample } from '../dashboard';
  import { chartRange, chartUnit, chartValue, utcHourMinute, type ChartKind } from '../history';

  interface Series {
    label: string;
    /** Couleur CSS (variable de thème). */
    color: string;
    /** Mesures SI, les plus anciennes en tête. */
    samples: Sample[];
  }

  interface Props {
    title: string;
    kind: ChartKind;
    series: Series[];
    now: number;
    windowMs: number;
  }

  let { title, kind, series, now, windowMs }: Props = $props();

  const W = 600;
  const H = 110;

  const unit = $derived(chartUnit(kind));
  const converted = $derived(
    series.map((s) => ({
      ...s,
      points: inWindow(s.samples, now, windowMs).map((p) => ({ t: p.t, v: chartValue(kind, p.v) })),
    })),
  );
  const first = $derived(converted[0] ? trendStats(converted[0].points) : null);
  const values = $derived(converted.flatMap((s) => s.points.map((p) => p.v)));
  const allMax = $derived(values.reduce((m, v) => Math.max(m, v), 0));
  const allMin = $derived(values.reduce((m, v) => Math.min(m, v), Infinity));
  const range = $derived(chartRange(kind, allMax, allMin));
  const yMin = $derived(range[0]);
  const yMax = $derived(range[1]);
  const empty = $derived(values.length === 0);
  const minimum = $derived(
    converted[0]?.points.length
      ? converted[0].points.reduce((m, p) => Math.min(m, p.v), Infinity)
      : null,
  );
  const frame = $derived({
    now,
    windowMs,
    yMin,
    yMax,
    width: W,
    height: H,
    gapMs: 10_000,
    maxJump: kind === 'angle' ? 180 : Infinity,
    invert: kind === 'depth',
  });
  const yMid = $derived((yMin + yMax) / 2);
  const yLabels = $derived(kind === 'depth' ? [yMin, yMid, yMax] : [yMax, yMid, yMin]);
  const fmt = (v: number) =>
    kind === 'angle' || kind === 'rpm' ? `${Math.round(v)}` : v.toFixed(1);
</script>

<article class="chart" data-testid="history-{title}">
  <header>
    <h3>{title}</h3>
    <div class="legend">
      {#if series.length > 1}
        {#each series as s (s.label)}
          <span><i style:background={s.color}></i>{s.label}</span>
        {/each}
      {/if}
      {#if first && kind !== 'angle'}
        <span class="stats">
          {#if kind === 'depth' && minimum !== null}
            min {fmt(minimum)} · max {fmt(first.max)} {unit}
          {:else}
            moy {fmt(first.avg)} · max {fmt(first.max)} {unit}
          {/if}
        </span>
      {/if}
    </div>
  </header>
  {#if empty}
    <p class="empty">Aucune mesure sur la période.</p>
  {:else}
    <div class="plot">
      <div class="y" aria-hidden="true">
        {#each yLabels as l, i (i)}<span>{fmt(l)}</span>{/each}
      </div>
      <svg viewBox="0 0 {W} {H}" preserveAspectRatio="none" aria-label="{title} ({unit})">
        {#each [0.25, 0.5, 0.75] as g (g)}
          <line x1="0" x2={W} y1={H * g} y2={H * g} class="grid" />
        {/each}
        {#each converted as s (s.label)}
          <path d={trendPath(s.points, frame)} style:stroke={s.color} class="line" />
        {/each}
      </svg>
    </div>
    <div class="x" aria-hidden="true">
      <span>{utcHourMinute(now - windowMs)}</span>
      <span>{utcHourMinute(now - windowMs / 2)}</span>
      <span>{utcHourMinute(now)} UTC</span>
    </div>
  {/if}
</article>

<style>
  .chart {
    padding: 0.6rem 0.8rem;
    border: 1px solid var(--border);
    border-radius: 0.6rem;
    background: var(--card-bg);
  }

  header {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  h3 {
    margin: 0;
    font-size: 0.9rem;
  }

  .legend {
    display: flex;
    gap: 0.7rem;
    color: var(--muted);
    font-size: 0.75rem;
  }

  .legend i {
    display: inline-block;
    width: 0.6rem;
    height: 0.6rem;
    margin-right: 0.25rem;
    border-radius: 50%;
  }

  .stats {
    color: var(--fg);
    font-weight: 600;
  }

  .plot {
    display: grid;
    grid-template-columns: 2.4rem 1fr;
    gap: 0.3rem;
    margin-top: 0.3rem;
  }

  .y {
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    align-items: flex-end;
    color: var(--muted);
    font-size: 0.7rem;
  }

  svg {
    display: block;
    width: 100%;
    height: 6rem;
    border-left: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
  }

  .grid {
    stroke: var(--border);
    stroke-width: 1;
    vector-effect: non-scaling-stroke;
  }

  .line {
    fill: none;
    stroke-width: 1.8;
    vector-effect: non-scaling-stroke;
  }

  .x {
    display: flex;
    justify-content: space-between;
    margin-left: 2.7rem;
    color: var(--muted);
    font-size: 0.7rem;
  }

  .empty {
    margin: 0.5rem 0 0;
    color: var(--muted);
    font-size: 0.85rem;
  }
</style>
