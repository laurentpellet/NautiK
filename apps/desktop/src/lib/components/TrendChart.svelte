<script lang="ts">
  import { inWindow, trendPath, trendScale, trendStats, type Sample } from '../dashboard';
  import { SPEED_UNITS, speedIn, units } from '../units.svelte';

  interface Props {
    title: string;
    /** Mesures SI (m/s), les plus anciennes en tête. */
    samples: Sample[];
    now: number;
    windowMs: number;
  }

  let { title, samples, now, windowMs }: Props = $props();

  const W = 600;
  const H = 120;

  const visible = $derived(inWindow(samples, now, windowMs));
  const stats = $derived(trendStats(visible));
  const unit = $derived(SPEED_UNITS[units.speed].label);
  const converted = $derived(visible.map((s) => ({ t: s.t, v: speedIn(s.v) })));
  const yMax = $derived(trendScale(stats ? speedIn(stats.max) : 0, 5));
  const path = $derived(trendPath(converted, { now, windowMs, yMax, width: W, height: H }));
  const avgY = $derived(stats ? H - (speedIn(stats.avg) / yMax) * H : null);
  const fmt = (mps: number) => speedIn(mps).toFixed(1);
</script>

<div class="trend" data-testid="trend">
  <div class="head">
    <span class="title">{title}</span>
    {#if stats}
      <span class="stats">
        <span class="last">{fmt(stats.last)} {unit}</span>
        <span class="max">max {fmt(stats.max)}</span>
      </span>
    {/if}
  </div>
  {#if stats}
    <svg viewBox="0 0 {W} {H}" preserveAspectRatio="none" aria-hidden="true">
      {#each [0.25, 0.5, 0.75] as g (g)}
        <line x1="0" x2={W} y1={H * g} y2={H * g} class="grid" />
      {/each}
      {#if avgY !== null}
        <line x1="0" x2={W} y1={avgY} y2={avgY} class="avg" />
      {/if}
      <path d={path} class="line" />
    </svg>
    <div class="axis">
      <span>moy. {fmt(stats.avg)} {unit}</span>
      <span>échelle 0–{yMax} {unit}</span>
      <span>maintenant</span>
    </div>
  {:else}
    <p class="empty">Aucune mesure sur la période.</p>
  {/if}
</div>

<style>
  .trend {
    padding: 0.6rem 0.8rem;
    border-radius: 0.5rem;
    background: var(--inset-bg);
  }

  .head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }

  .title {
    color: var(--muted);
    font-size: 0.8rem;
    font-weight: 700;
    letter-spacing: 0.08em;
  }

  .stats {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
  }

  .last {
    color: var(--tone-wind);
    font-size: 1.4rem;
    font-weight: 700;
  }

  .max {
    color: var(--tone-bad);
    font-size: 0.85rem;
    font-weight: 600;
  }

  svg {
    display: block;
    width: 100%;
    height: 7rem;
  }

  .grid {
    stroke: var(--border);
    stroke-width: 1;
  }

  .avg {
    stroke: var(--muted);
    stroke-width: 1;
    stroke-dasharray: 6 4;
  }

  .line {
    fill: none;
    stroke: var(--tone-wind);
    stroke-width: 2;
    vector-effect: non-scaling-stroke;
  }

  .axis {
    display: flex;
    justify-content: space-between;
    color: var(--muted);
    font-size: 0.75rem;
  }

  .empty {
    margin: 0.5rem 0 0;
    color: var(--muted);
    font-size: 0.85rem;
  }
</style>
