<script lang="ts">
  import {
    formatAge,
    formatBearing,
    formatDepth,
    formatRelativeAngle,
    formatSpeed,
  } from '../format';
  import type { Quantity, Reading } from '../services/instruments';

  interface Props {
    readings: Reading[];
    /** Le moteur ne diffuse plus : toutes les valeurs sont douteuses. */
    silent: boolean;
  }

  let { readings, silent }: Props = $props();

  const ROWS: { quantity: Quantity; label: string; format: (v: number) => string }[] = [
    { quantity: 'speedOverGround', label: 'SOG', format: formatSpeed },
    { quantity: 'courseOverGroundTrue', label: 'COG', format: formatBearing },
    { quantity: 'headingTrue', label: 'Cap vrai', format: formatBearing },
    { quantity: 'headingMagnetic', label: 'Cap magn.', format: formatBearing },
    { quantity: 'speedThroughWater', label: 'Surface', format: formatSpeed },
    { quantity: 'depthBelowTransducer', label: 'Fond', format: formatDepth },
    { quantity: 'windSpeedApparent', label: 'Vent app.', format: formatSpeed },
    { quantity: 'windAngleApparent', label: 'Angle app.', format: formatRelativeAngle },
  ];

  const byQuantity = $derived(new Map(readings.map((r) => [r.quantity, r])));
</script>

<section class="instruments" aria-label="Instruments">
  <h2>Instruments</h2>
  {#if readings.length === 0}
    <p class="muted">Aucune donnée d'instrument.</p>
  {:else}
    <dl data-testid="instruments">
      {#each ROWS as row (row.quantity)}
        {@const reading = byQuantity.get(row.quantity)}
        {#if reading}
          {@const stale = silent || reading.stale}
          <div
            class:stale
            title={`${reading.sourceId} — il y a ${formatAge(reading.ageMs)}${stale ? ' (obsolète)' : ''}`}
            data-quantity={row.quantity}
          >
            <dt>{row.label}</dt>
            <dd>
              {row.format(reading.value)}
              {#if stale}<span class="flag" aria-label="donnée obsolète">⚠</span>{/if}
            </dd>
          </div>
        {/if}
      {/each}
    </dl>
  {/if}
</section>

<style>
  h2 {
    margin: 0.75rem 0 0.25rem;
    font-size: 1rem;
  }

  dl {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 0.25rem 0.75rem;
    margin: 0;
  }

  dl > div {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 0.4rem;
  }

  dt {
    color: var(--muted);
    font-size: 0.75rem;
  }

  dd {
    margin: 0;
    font:
      600 0.95rem ui-monospace,
      'Cascadia Mono',
      Consolas,
      monospace;
    white-space: nowrap;
  }

  .stale dd {
    opacity: 0.5;
  }

  .flag {
    color: var(--alert-fg);
  }

  .muted {
    color: var(--muted);
    font-size: 0.8rem;
  }
</style>
