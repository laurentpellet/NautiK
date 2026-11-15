<script lang="ts">
  // Résultat de l'outil de mesure : distance totale, dernier segment
  // (distance et relèvement) et durée à la vitesse prévue. Valeurs du
  // moteur ; ce composant ne fait que les présenter.
  import { formatBearing, formatDistance, formatDuration } from '../format';
  import type { MapToolsController } from '../mapTools.svelte';
  import { SPEED_UNITS, speedIn, units } from '../units.svelte';

  interface Props {
    tools: MapToolsController;
    /** Convertit la mesure en route enregistrée (à partir de deux points). */
    onconvert?: () => void;
  }

  let { tools, onconvert }: Props = $props();

  const plan = $derived(tools.plan);
  const last = $derived(plan?.legs.at(-1) ?? null);
  /** Le plan correspond-il aux points cliqués ? (sinon recalcul en cours) */
  const current = $derived(plan !== null && plan.legs.length === tools.points.length - 1);
</script>

<section class="measure" aria-label="Mesure" data-testid="measure-panel">
  <header>
    <h3>Mesure</h3>
    <button
      type="button"
      class="close"
      aria-label="Fermer la mesure"
      title="Fermer la mesure (Échap)"
      onclick={() => tools.close()}>✕</button
    >
  </header>
  {#if tools.points.length < 2}
    <p class="hint">
      {tools.points.length === 0
        ? 'Cliquez sur la carte pour placer le premier point.'
        : 'Cliquez sur la carte pour placer le point suivant.'}
    </p>
  {:else if plan && last}
    <dl class:pending={!current}>
      <dt>Distance</dt>
      <dd data-testid="measure-total">{formatDistance(plan.totalDistanceM)}</dd>
      {#if plan.legs.length > 1}
        <dt>Dernier segment</dt>
        <dd>{formatDistance(last.distanceM)}</dd>
      {/if}
      <dt>Relèvement</dt>
      <dd data-testid="measure-bearing">{formatBearing(last.bearingRad)}</dd>
      <dt>À {speedIn(plan.speedMps).toFixed(1)} {SPEED_UNITS[units.speed].label}</dt>
      <dd>{formatDuration(plan.totalDurationS * 1000)}</dd>
    </dl>
  {:else if !tools.error}
    <p class="hint">Calcul…</p>
  {/if}
  {#if tools.error}
    <p class="error" role="alert">{tools.error}</p>
  {/if}
  {#if tools.points.length > 0}
    <div class="actions">
      <button type="button" onclick={() => tools.undoPoint()}>Annuler le point</button>
      <button type="button" onclick={() => tools.clearMeasure()}>Effacer</button>
      {#if onconvert && tools.points.length >= 2}
        <button
          type="button"
          title="Enregistrer ces points comme une route (non activée)"
          onclick={onconvert}>Convertir en route</button
        >
      {/if}
    </div>
  {/if}
</section>

<style>
  .measure {
    min-width: 12rem;
    max-width: 16rem;
    padding: 0.5rem 0.6rem;
    border: 1px solid var(--border);
    border-left: 3px solid #00838f;
    border-radius: 0.4rem;
    background: var(--panel-bg);
    color: var(--fg);
    font-size: 0.85rem;
    box-shadow: 0 2px 8px rgb(0 0 0 / 25%);
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  h3 {
    margin: 0;
    color: var(--muted);
    font-size: 0.75rem;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }

  dl {
    display: grid;
    grid-template-columns: auto auto;
    gap: 0.15rem 0.6rem;
    margin: 0.35rem 0;
  }

  dl.pending {
    opacity: 0.55;
  }

  dt {
    color: var(--muted);
  }

  dd {
    margin: 0;
    font-weight: 600;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  p {
    margin: 0.35rem 0;
  }

  .hint {
    color: var(--muted);
  }

  .error {
    color: var(--error-fg);
    font-weight: 600;
  }

  .actions {
    display: flex;
    gap: 0.3rem;
  }

  button {
    padding: 0.2rem 0.5rem;
    border: 1px solid var(--border);
    border-radius: 0.3rem;
    background: var(--button-bg);
    color: var(--fg);
    font: inherit;
    font-size: 0.8rem;
    cursor: pointer;
  }

  .close {
    padding: 0 0.35rem;
    border-color: transparent;
    background: transparent;
  }
</style>
