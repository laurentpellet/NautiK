<script lang="ts">
  // Barre latérale de la route active, collée à gauche de la carte :
  // waypoint visé et éléments de navigation (distance, relèvement, écart,
  // temps et heures d'arrivée). Réductible à une colonne étroite qui garde
  // DTW et BTW. Valeurs du moteur ; une valeur obsolète est estompée et
  // signalée, une valeur absente reste « — ».
  import Icon from './Icon.svelte';
  import { routeBarTiles } from '../instrumentTiles';
  import type { NavigationStatus } from '../services/routing';

  interface Props {
    status: NavigationStatus;
    now: number;
    /** La position qui sert au calcul est obsolète (ou moteur muet). */
    stale: boolean;
    /** Ouvre le gestionnaire des routes, traces et marques. */
    onroutes: () => void;
    /** Barre réduite à une colonne étroite. */
    collapsed?: boolean;
    ondeactivate: () => void;
    oncollapse?: (collapsed: boolean) => void;
    /** Côté de la carte (sens des flèches de réduction). */
    side?: 'left' | 'right';
  }

  let {
    status,
    now,
    stale,
    collapsed = false,
    onroutes,
    ondeactivate,
    oncollapse,
    side = 'left',
  }: Props = $props();

  // Réduire pousse la barre vers son bord de la carte.
  const arrows = $derived(
    side === 'left' ? { fold: '«', unfold: '»' } : { fold: '»', unfold: '«' },
  );

  const tiles = $derived(routeBarTiles(status, now, stale));
  /** Réduite : seuls la distance et le relèvement du waypoint restent. */
  const shown = $derived(collapsed ? tiles.filter((t) => t.id === 'dtw' || t.id === 'btw') : tiles);
</script>

<div class="bar" class:collapsed data-testid="route-bar">
  <div class="group" role="toolbar" aria-label="Route">
    {#if oncollapse}
      <button
        type="button"
        class="fold"
        aria-label={collapsed ? 'Déplier la barre de route' : 'Réduire la barre de route'}
        aria-expanded={!collapsed}
        title={collapsed ? 'Déplier la barre de route' : 'Réduire la barre de route'}
        onclick={() => oncollapse(!collapsed)}>{collapsed ? arrows.unfold : arrows.fold}</button
      >
    {/if}
    <button
      type="button"
      aria-label="Routes, traces et marques"
      title="Routes, traces et marques (Paramètres)"
      onclick={onroutes}
    >
      <Icon name="route" />
    </button>
    <button
      type="button"
      class="stop"
      aria-label="Arrêter la navigation"
      title="Arrêter la navigation"
      onclick={ondeactivate}
    >
      <Icon name="stop" />
    </button>
  </div>

  <div class="target" title={`${status.routeName} → ${status.targetName}`}>
    {#if !collapsed}
      <span class="name route-name">
        {status.routeName}
      </span>
    {/if}
    {#if status.simulated}<span class="badge">{collapsed ? 'SIM' : 'SIMULATION'}</span>{/if}
    {#if status.finished}
      <span class="value" role="status">Arrivée</span>
    {:else if collapsed}
      <span class="value small" class:stale>{status.targetIndex + 1}/{status.waypointCount}</span>
    {:else}
      <span class="value" class:stale
        >→ {status.targetName}
        <small>{status.targetIndex + 1}/{status.waypointCount}</small></span
      >
    {/if}
  </div>

  {#if !status.finished}
    <div class="metrics" aria-label="Éléments de route">
      {#each shown as t (t.id)}
        <div
          class="metric"
          class:stale={t.stale}
          class:missing={t.missing}
          data-metric={t.id}
          title={t.detail ?? undefined}
        >
          <span class="name">{t.label}</span>
          <span class="value"
            >{t.value}{#if t.unit}<small>{t.unit}</small>{/if}{#if t.stale}<span
                class="flag"
                aria-label="donnée obsolète">⚠</span
              >{/if}</span
          >
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .bar {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    height: 100%;
    padding: 0.5rem 0.35rem 0.5rem 0.6rem;
    overflow-y: auto;
    border-left: 3px solid #d81b60;
  }

  .bar.collapsed {
    align-items: center;
    padding: 0.5rem 0.2rem;
  }

  .group {
    display: flex;
    gap: 0.15rem;
  }

  .collapsed .group {
    flex-direction: column;
  }

  .target {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    padding-bottom: 0.5rem;
    border-bottom: 1px solid var(--border);
    min-width: 0;
  }

  .collapsed .target {
    align-items: center;
  }

  .route-name,
  .target .value {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .metrics {
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
  }

  .metric {
    display: flex;
    flex-direction: column;
  }

  .collapsed .metric {
    align-items: center;
  }

  .name {
    color: var(--muted);
    font-size: 0.6rem;
    font-weight: 700;
    letter-spacing: 0.1em;
    white-space: nowrap;
  }

  .value {
    font-size: 1.05rem;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .collapsed .value {
    font-size: 0.8rem;
  }

  .value small {
    margin-left: 0.15rem;
    color: var(--muted);
    font-size: 0.7rem;
    font-weight: 500;
  }

  .collapsed .value small {
    display: block;
    margin: 0;
    text-align: center;
  }

  .stale .value,
  .value.stale {
    opacity: 0.5;
  }

  .missing .value {
    color: var(--muted);
  }

  .flag {
    margin-left: 0.2rem;
    color: var(--warn-fg);
    opacity: 1;
  }

  .badge {
    align-self: flex-start;
    padding: 0 0.3rem;
    border-radius: 0.2rem;
    background: var(--sim-bg);
    color: var(--sim-fg);
    font-size: 0.65rem;
    font-weight: 700;
  }

  .collapsed .badge {
    align-self: center;
  }

  button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 2.1rem;
    height: 2.1rem;
    padding: 0;
    border: 1px solid transparent;
    border-radius: 0.35rem;
    background: transparent;
    color: var(--fg);
    cursor: pointer;
  }

  button:hover,
  button:focus-visible {
    border-color: var(--border);
    background: var(--button-bg);
  }

  .fold {
    font-size: 1.1rem;
    font-weight: 700;
  }

  .stop {
    color: var(--error-fg);
  }
</style>
