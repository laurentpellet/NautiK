<script lang="ts">
  // Barre de la route active : waypoint visé et éléments de navigation
  // (distance, relèvement, écart, temps et heures d'arrivée), comme la
  // barre d'outils de la carte. Valeurs du moteur ; une valeur obsolète
  // est estompée et signalée, une valeur absente reste « — ».
  import Icon from './Icon.svelte';
  import { routeBarTiles } from '../instrumentTiles';
  import type { NavigationStatus } from '../services/routing';

  interface Props {
    status: NavigationStatus;
    now: number;
    /** La position qui sert au calcul est obsolète (ou moteur muet). */
    stale: boolean;
    /** Panneau Route affiché. */
    routePanel: boolean;
    ontoggleroute: () => void;
    ondeactivate: () => void;
  }

  let { status, now, stale, routePanel, ontoggleroute, ondeactivate }: Props = $props();

  const tiles = $derived(routeBarTiles(status, now, stale));
</script>

<div class="bar" data-testid="route-bar">
  <div class="group" role="toolbar" aria-label="Route">
    <button
      type="button"
      aria-label="Panneau Route"
      aria-pressed={routePanel}
      title={routePanel ? 'Masquer le panneau Route' : 'Afficher le panneau Route'}
      onclick={ontoggleroute}
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

  <span class="sep" aria-hidden="true"></span>

  <div class="target" title={status.routeName}>
    <span class="name">
      {status.routeName}
      {#if status.simulated}<span class="badge">SIMULATION</span>{/if}
    </span>
    {#if status.finished}
      <span class="value" role="status">Arrivée</span>
    {:else}
      <span class="value" class:stale
        >→ {status.targetName}
        <small>{status.targetIndex + 1}/{status.waypointCount}</small></span
      >
    {/if}
  </div>

  {#if !status.finished}
    <span class="sep" aria-hidden="true"></span>
    <div class="metrics" aria-label="Éléments de route">
      {#each tiles as t (t.id)}
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
    align-items: center;
    gap: 0.5rem;
    padding: 0.25rem 0.35rem;
    flex-wrap: wrap;
    border-left: 3px solid #d81b60;
  }

  .group {
    display: flex;
    gap: 0.15rem;
  }

  .sep {
    align-self: stretch;
    width: 1px;
    margin: 0.2rem 0;
    background: var(--border);
  }

  .target {
    display: flex;
    flex-direction: column;
    max-width: 12rem;
  }

  .target .value {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .metrics {
    display: flex;
    gap: 0.9rem;
    flex: 1;
    justify-content: center;
    flex-wrap: wrap;
    padding: 0 0.3rem;
  }

  .metric {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    min-width: 3.2rem;
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

  .value small {
    margin-left: 0.15rem;
    color: var(--muted);
    font-size: 0.7rem;
    font-weight: 500;
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
    margin-left: 0.3rem;
    padding: 0 0.3rem;
    border-radius: 0.2rem;
    background: var(--sim-bg);
    color: var(--sim-fg);
    letter-spacing: 0;
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

  button[aria-pressed='true'] {
    border-color: var(--border);
    background: var(--button-bg);
  }

  button[aria-pressed='false'] {
    color: var(--muted);
    opacity: 0.6;
  }

  .stop {
    color: var(--error-fg);
  }
</style>
