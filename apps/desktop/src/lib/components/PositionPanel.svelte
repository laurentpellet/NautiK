<script lang="ts">
  import { formatAge, formatLatitude, formatLongitude } from '../format';
  import type { PositionStatus } from '../services/instruments';

  interface Props {
    /** Position et fraîcheur calculée par le moteur. */
    position: PositionStatus | null;
    /** Le moteur ne diffuse plus : les valeurs affichées ne sont plus garanties. */
    silent: boolean;
    error: string | null;
  }

  let { position, silent, error }: Props = $props();

  const stale = $derived(silent || (position?.stale ?? false));
</script>

<section class="position" aria-label="Position">
  {#if silent}
    <p class="badge stale" role="alert">Moteur muet : données non actualisées</p>
  {/if}
  {#if position}
    {#if position.fix.source === 'simulator'}
      <p class="badge simulated" role="status">POSITION SIMULÉE</p>
    {:else if position.fix.source === 'replay'}
      <p class="badge simulated" role="status">REJEU — POSITION NON ACTUELLE</p>
    {/if}
    {#if position.stale}
      <p class="badge stale" role="alert">
        Position obsolète ({formatAge(position.ageMs)})
      </p>
    {/if}
    <p class="coords" class:dimmed={stale} data-testid="position">
      <span>{formatLatitude(position.fix.latitudeDeg)}</span>
      <span>{formatLongitude(position.fix.longitudeDeg)}</span>
    </p>
    <p class="meta">
      Source : {position.fix.sourceId} — il y a {formatAge(position.ageMs)}
      {#if position.freshSources > 1}
        <span data-testid="position-sources">({position.freshSources} sources disponibles)</span>
      {/if}
    </p>
  {:else}
    <p class="badge stale" role="alert">Aucune position</p>
  {/if}
  {#if error}
    <p class="error" role="alert">Données rejetées : {error}</p>
  {/if}
</section>

<style>
  .position p {
    margin: 0.25rem 0;
  }

  .coords {
    display: flex;
    flex-direction: column;
    font:
      600 1.15rem/1.3 ui-monospace,
      'Cascadia Mono',
      Consolas,
      monospace;
  }

  .coords.dimmed {
    opacity: 0.5;
  }

  .meta {
    color: var(--muted);
    font-size: 0.8rem;
  }

  .badge {
    display: inline-block;
    margin-right: 0.25rem;
    padding: 0.1rem 0.5rem;
    border-radius: 0.25rem;
    font-size: 0.75rem;
    font-weight: 700;
    letter-spacing: 0.04em;
  }

  .simulated {
    background: var(--sim-bg);
    color: var(--sim-fg);
  }

  .stale {
    background: var(--alert-bg);
    color: var(--alert-fg);
  }

  .error {
    color: var(--error-fg);
    font-size: 0.8rem;
  }
</style>
