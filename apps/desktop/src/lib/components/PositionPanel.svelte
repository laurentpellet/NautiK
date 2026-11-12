<script lang="ts">
  import { formatAge, formatLatitude, formatLongitude } from '../format';
  import type { PositionFix } from '../services/position';

  interface Props {
    fix: PositionFix | null;
    stale: boolean;
    now: number;
    error: string | null;
  }

  let { fix, stale, now, error }: Props = $props();
</script>

<section class="position" aria-label="Position">
  {#if fix}
    {#if fix.source === 'simulator'}
      <p class="badge simulated" role="status">POSITION SIMULÉE</p>
    {/if}
    {#if stale}
      <p class="badge stale" role="alert">
        Position obsolète ({formatAge(now - fix.measuredAt)})
      </p>
    {/if}
    <p class="coords" class:dimmed={stale} data-testid="position">
      <span>{formatLatitude(fix.latitudeDeg)}</span>
      <span>{formatLongitude(fix.longitudeDeg)}</span>
    </p>
    <p class="meta">Source : {fix.sourceId} — il y a {formatAge(now - fix.measuredAt)}</p>
  {:else}
    <p class="badge stale" role="alert">Aucune position</p>
  {/if}
  {#if error}
    <p class="error" role="alert">Relevé rejeté : {error}</p>
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
