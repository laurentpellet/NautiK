<script lang="ts">
  // « Données historiques » : mesures reçues depuis le lancement,
  // conservées en mémoire (aucune persistance, aucune extrapolation).
  import type { History } from '../history';
  import HistoryChart from './HistoryChart.svelte';

  interface Props {
    history: History;
    now: number;
  }

  let { history, now }: Props = $props();

  const RANGES = [
    { label: '15 min', ms: 15 * 60_000 },
    { label: '1 h', ms: 60 * 60_000 },
    { label: '6 h', ms: 6 * 60 * 60_000 },
  ] as const;

  let windowMs = $state<number>(RANGES[1].ms);
  const h = $derived(history);
</script>

<section class="history" aria-label="Données historiques">
  <header>
    <h2>DONNÉES HISTORIQUES <span class="muted">— depuis le lancement, en mémoire</span></h2>
    <div class="ranges" role="radiogroup" aria-label="Période">
      {#each RANGES as r (r.ms)}
        <button
          type="button"
          role="radio"
          aria-checked={windowMs === r.ms}
          onclick={() => (windowMs = r.ms)}
        >
          {r.label}
        </button>
      {/each}
    </div>
  </header>
  <div class="grid">
    <HistoryChart
      title="Vitesse"
      kind="speed"
      {now}
      {windowMs}
      series={[
        { label: 'SOG', color: 'var(--tone-water)', samples: h.speedOverGround ?? [] },
        { label: 'Surface', color: 'var(--tone-good)', samples: h.speedThroughWater ?? [] },
      ]}
    />
    <HistoryChart
      title="Cap"
      kind="angle"
      {now}
      {windowMs}
      series={[
        { label: 'COG', color: 'var(--tone-water)', samples: h.courseOverGroundTrue ?? [] },
        {
          label: h.headingTrue ? 'Cap vrai' : 'Cap magn.',
          color: 'var(--tone-wind)',
          samples: h.headingTrue ?? h.headingMagnetic ?? [],
        },
      ]}
    />
    <HistoryChart
      title="Vent apparent"
      kind="speed"
      {now}
      {windowMs}
      series={[{ label: 'AWS', color: 'var(--tone-good)', samples: h.windSpeedApparent ?? [] }]}
    />
    <HistoryChart
      title="Profondeur"
      kind="depth"
      {now}
      {windowMs}
      series={[
        { label: 'Fond', color: 'var(--tone-depth)', samples: h.depthBelowTransducer ?? [] },
      ]}
    />
    <HistoryChart
      title="Batterie"
      kind="voltage"
      {now}
      {windowMs}
      series={[{ label: 'Tension', color: 'var(--tone-angle)', samples: h.batteryVoltage ?? [] }]}
    />
    <HistoryChart
      title="Régime moteur"
      kind="rpm"
      {now}
      {windowMs}
      series={[{ label: 'Régime', color: 'var(--tone-wind)', samples: h.engineSpeed ?? [] }]}
    />
  </div>
</section>

<style>
  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
    flex-wrap: wrap;
    margin-bottom: 0.6rem;
  }

  h2 {
    margin: 0;
    color: var(--fg);
    font-size: 0.95rem;
    letter-spacing: 0.08em;
  }

  .muted {
    color: var(--muted);
    font-size: 0.8rem;
    font-weight: 400;
    letter-spacing: 0;
  }

  .ranges {
    display: flex;
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    overflow: hidden;
  }

  .ranges button {
    padding: 0.25rem 0.9rem;
    border: 0;
    background: var(--button-bg);
    color: var(--muted);
    font: inherit;
    font-size: 0.85rem;
    cursor: pointer;
  }

  .ranges button[aria-checked='true'] {
    background: var(--accent-bg);
    color: var(--accent-fg);
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(22rem, 1fr));
    gap: 0.8rem;
  }
</style>
