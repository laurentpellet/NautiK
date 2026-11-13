<script lang="ts">
  // Carte « Navigation (temps réel) » de l'onglet Carte : grandes
  // valeurs, compas et valeurs secondaires. Les valeurs obsolètes sont
  // estompées et signalées ; une donnée absente reste « — ».
  import { instrumentTiles, type Tile } from '../instrumentTiles';
  import type { InstrumentsSnapshot } from '../services/instruments';
  import CompassRose from './CompassRose.svelte';

  interface Props {
    snapshot: InstrumentsSnapshot | null;
    silent: boolean;
  }

  let { snapshot, silent }: Props = $props();

  let collapsed = $state(false);

  const tiles = $derived(new Map(instrumentTiles(snapshot, silent).map((t) => [t.id, t])));
  const MAIN = ['sog', 'heading', 'cog', 'aws'];
  const SECONDARY = ['depth', 'stw', 'awa'];

  // Compas : cap vrai si reçu, sinon route fond.
  const compass = $derived.by(() => {
    const readings = snapshot?.readings ?? [];
    const heading = readings.find((r) => r.quantity === 'headingTrue');
    const cog = readings.find((r) => r.quantity === 'courseOverGroundTrue');
    const r = heading ?? cog ?? null;
    return {
      angleRad: r?.value ?? null,
      label: heading ? 'Cap vrai' : cog ? 'COG' : 'Cap',
      stale: silent || (r?.stale ?? false),
    };
  });
</script>

{#snippet value(t: Tile | undefined, big: boolean)}
  {#if t}
    <div
      class="value"
      class:big
      class:stale={t.stale}
      class:missing={t.missing}
      data-tile={t.id}
      title={t.detail ?? undefined}
    >
      <span class="number">
        {t.value}{#if t.unit}<small>{t.unit}</small>{/if}
        {#if t.stale}<span class="flag" aria-label="donnée obsolète">⚠</span>{/if}
      </span>
      <span class="label">{t.label}</span>
    </div>
  {/if}
{/snippet}

<section class="card" aria-label="Navigation temps réel" data-testid="instruments">
  <header>
    <h2>NAVIGATION <span class="muted">(temps réel)</span></h2>
    <button type="button" aria-expanded={!collapsed} onclick={() => (collapsed = !collapsed)}>
      {collapsed ? 'Afficher' : 'Réduire'}
    </button>
  </header>
  {#if !collapsed}
    <div class="top">
      <div class="main">
        {#each MAIN as id (id)}
          {@render value(tiles.get(id), true)}
        {/each}
      </div>
      <CompassRose {...compass} />
    </div>
    <hr />
    <div class="secondary">
      {#each SECONDARY as id (id)}
        {@render value(tiles.get(id), false)}
      {/each}
    </div>
  {/if}
</section>

<style>
  .card {
    width: min(30rem, calc(100vw - 26rem));
    min-width: 18rem;
    padding: 0.7rem 1rem;
    border: 1px solid var(--border);
    border-radius: 0.6rem;
    background: var(--card-bg);
    color: var(--fg);
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.25);
  }

  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  h2 {
    margin: 0;
    font-size: 0.9rem;
    letter-spacing: 0.08em;
  }

  .muted {
    color: var(--muted);
    font-weight: 400;
    letter-spacing: 0;
  }

  header button {
    padding: 0.15rem 0.5rem;
    border: 1px solid var(--border);
    border-radius: 0.3rem;
    background: var(--button-bg);
    color: var(--fg);
    font: inherit;
    font-size: 0.75rem;
    cursor: pointer;
  }

  .top {
    display: grid;
    grid-template-columns: 1fr minmax(8rem, 11rem);
    align-items: center;
    gap: 0.5rem;
    margin-top: 0.4rem;
  }

  .main {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.6rem 1rem;
  }

  hr {
    margin: 0.5rem 0;
    border: 0;
    border-top: 1px solid var(--border);
  }

  .secondary {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 0.5rem;
  }

  .value {
    display: flex;
    flex-direction: column;
    align-items: center;
    min-width: 0;
  }

  .number {
    font-size: 1.35rem;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .big .number {
    font-size: 2rem;
    font-weight: 800;
  }

  small {
    margin-left: 0.2rem;
    color: var(--muted);
    font-size: 0.55em;
    font-weight: 600;
  }

  .label {
    color: var(--muted);
    font-size: 0.8rem;
  }

  .missing .number {
    color: var(--muted);
  }

  .stale .number {
    opacity: 0.45;
  }

  .flag {
    margin-left: 0.2rem;
    color: var(--alert-fg);
    font-size: 0.8em;
  }
</style>
