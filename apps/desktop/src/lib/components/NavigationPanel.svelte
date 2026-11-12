<script lang="ts">
  import { formatBearing, formatCrossTrack, formatDistance, formatEta } from '../format';
  import type { NavigationStatus } from '../services/routing';

  interface Props {
    status: NavigationStatus;
    now: number;
    ondeactivate: () => void;
  }

  let { status, now, ondeactivate }: Props = $props();
</script>

<section class="navigation" aria-label="Navigation active">
  <header>
    <strong>{status.routeName}</strong>
    {#if status.simulated}
      <span class="badge simulated">SIMULATION</span>
    {/if}
  </header>

  {#if status.finished}
    <p class="arrived" role="status">Arrivée — dernier waypoint atteint</p>
  {:else}
    <p class="target">
      Vers <strong>{status.targetName}</strong>
      <span class="muted">({status.targetIndex + 1}/{status.waypointCount})</span>
    </p>
    <dl data-testid="navigation-data">
      <dt title="Distance au waypoint">DTW</dt>
      <dd>{formatDistance(status.distanceToTargetM)}</dd>
      <dt title="Relèvement vrai du waypoint">BTW</dt>
      <dd>{formatBearing(status.bearingToTargetRad)}</dd>
      <dt title="Écart de route (D : à droite, G : à gauche)">XTE</dt>
      <dd>{formatCrossTrack(status.crossTrackM)}</dd>
      <dt title="Heure estimée d'arrivée au waypoint">ETA</dt>
      <dd>{formatEta(status.etaTarget, now)}</dd>
      <dt title="Distance restante jusqu'à la fin de la route">Reste</dt>
      <dd>{formatDistance(status.remainingDistanceM)}</dd>
      <dt title="Heure estimée d'arrivée en fin de route">ETA fin</dt>
      <dd>{formatEta(status.etaEnd, now)}</dd>
    </dl>
  {/if}
  <button type="button" onclick={ondeactivate}>Arrêter la navigation</button>
</section>

<style>
  .navigation {
    margin: 0.5rem 0;
    padding: 0.5rem;
    border: 2px solid #d81b60;
    border-radius: 0.4rem;
  }

  header {
    display: flex;
    gap: 0.5rem;
    align-items: center;
  }

  p {
    margin: 0.25rem 0;
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 0.1rem 0.6rem;
    margin: 0.25rem 0 0.5rem;
    font:
      600 0.95rem/1.35 ui-monospace,
      'Cascadia Mono',
      Consolas,
      monospace;
  }

  dt {
    color: var(--muted);
  }

  dd {
    margin: 0;
  }

  .muted {
    color: var(--muted);
    font-size: 0.8rem;
  }

  .arrived {
    font-weight: 700;
  }

  .badge {
    padding: 0.05rem 0.4rem;
    border-radius: 0.25rem;
    font-size: 0.7rem;
    font-weight: 700;
  }

  .simulated {
    background: var(--sim-bg);
    color: var(--sim-fg);
  }

  button {
    padding: 0.3rem 0.6rem;
    border: 1px solid var(--border);
    border-radius: 0.3rem;
    background: var(--button-bg);
    color: var(--fg);
    font: inherit;
    font-size: 0.85rem;
    cursor: pointer;
  }
</style>
