<script lang="ts">
  // « Systèmes » : liaison et fraîcheur de chaque grandeur reçue
  // (source retenue, âge). Une donnée obsolète est signalée en clair.
  import { describeConnection, formatAge } from '../format';
  import type { ConnectionState, Quantity, Reading } from '../services/instruments';

  interface Props {
    readings: Reading[];
    silent: boolean;
    /** `undefined` : état inconnu ; `null` : pas de liaison (simulateur). */
    connection: ConnectionState | null | undefined;
  }

  let { readings, silent, connection }: Props = $props();

  const LABELS: Record<Quantity, string> = {
    speedOverGround: 'SOG',
    courseOverGroundTrue: 'COG',
    headingTrue: 'Cap vrai',
    headingMagnetic: 'Cap magn.',
    speedThroughWater: 'Surface',
    depthBelowTransducer: 'Fond',
    windSpeedApparent: 'Vent app.',
    windAngleApparent: 'Angle app.',
    windSpeedTrue: 'Vent réel',
    windDirectionTrue: 'Dir. vent réel',
    waterTemperature: 'Temp. eau',
    airTemperature: 'Temp. air',
    atmosphericPressure: 'Pression',
    engineSpeed: 'Régime',
    engineCoolantTemperature: 'Temp. moteur',
    engineOilPressure: 'Huile',
    fuelLevel: 'Carburant',
    freshWaterLevel: 'Eau douce',
    batteryVoltage: 'Tension',
    batteryCurrent: 'Courant',
    batteryStateOfCharge: 'Charge',
    solarPower: 'Solaire',
  };

  const link = $derived(
    connection === undefined
      ? { label: 'État inconnu', level: 'pending' as const }
      : connection === null
        ? { label: 'Simulateur local', level: 'ok' as const }
        : describeConnection(connection),
  );
</script>

<article class="card" aria-label="Systèmes" data-testid="systems">
  <h2>SYSTÈMES</h2>
  <ul>
    <li class={link.level === 'ok' ? 'ok' : link.level === 'alert' ? 'bad' : 'wait'}>
      <span class="dot" aria-hidden="true"></span>
      <span class="name">Liaison</span>
      <span class="info" title={link.label}>{link.label}</span>
    </li>
    {#each readings as r (r.quantity)}
      {@const stale = silent || r.stale}
      <li class={stale ? 'bad' : 'ok'} data-quantity={r.quantity}>
        <span class="dot" aria-hidden="true"></span>
        <span class="name">{LABELS[r.quantity]}</span>
        <span class="info">
          {r.sourceId} · {formatAge(r.ageMs)}{#if stale}<strong> — obsolète</strong>{/if}
        </span>
      </li>
    {:else}
      <li class="wait">
        <span class="dot" aria-hidden="true"></span>
        <span class="name">Instruments</span>
        <span class="info">Aucune donnée reçue</span>
      </li>
    {/each}
  </ul>
</article>

<style>
  .card {
    padding: 1rem 1.2rem;
    border: 1px solid var(--border);
    border-radius: 0.8rem;
    background: var(--card-bg);
  }

  h2 {
    margin: 0 0 0.5rem;
    color: var(--muted);
    font-size: 0.9rem;
    letter-spacing: 0.12em;
  }

  ul {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: 0.85rem;
  }

  li {
    display: grid;
    grid-template-columns: 0.8rem 6rem 1fr;
    align-items: center;
    gap: 0.4rem;
  }

  .dot {
    width: 0.6rem;
    height: 0.6rem;
    border-radius: 50%;
    background: var(--muted);
  }

  .ok .dot {
    background: var(--ok-fg);
  }

  .bad .dot {
    background: var(--alert-fg);
  }

  .wait .dot {
    background: var(--sim-fg);
  }

  .info {
    overflow: hidden;
    color: var(--muted);
    text-align: right;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .bad .info {
    color: var(--alert-fg);
  }
</style>
