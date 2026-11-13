<script lang="ts">
  // Tableau de bord des instruments : cartes sombres, grands chiffres
  // colorés, cadran de vent apparent et tendance du vent. Seules les
  // données reçues sont affichées ; une donnée absente reste « — ».
  import type { Snippet } from 'svelte';
  import type { History } from '../history';
  import {
    instrumentTiles,
    navigationTiles,
    positionTile,
    systemTiles,
    type Tile,
  } from '../instrumentTiles';
  import SystemGroup from './SystemGroup.svelte';
  import type { ConnectionState, InstrumentsSnapshot } from '../services/instruments';
  import type { NavigationStatus } from '../services/routing';
  import HistorySection from './HistorySection.svelte';
  import SystemsCard from './SystemsCard.svelte';
  import TrendChart from './TrendChart.svelte';
  import WindDial from './WindDial.svelte';

  interface Props {
    snapshot: InstrumentsSnapshot | null;
    /** Le moteur ne diffuse plus : toutes les valeurs sont douteuses. */
    silent: boolean;
    /** Navigation active, le cas échéant. */
    navigation: NavigationStatus | null;
    /** Alertes de sûreté, toujours visibles. */
    status: Snippet;
    /** Mesures reçues depuis le lancement (SI). */
    history?: History;
    /** `undefined` : état inconnu ; `null` : pas de liaison (simulateur). */
    connection?: ConnectionState | null;
    now?: number;
  }

  let {
    snapshot,
    silent,
    navigation,
    status,
    history = {},
    connection = undefined,
    now = Date.now(),
  }: Props = $props();

  /** Fenêtre de la tendance du vent. */
  const TREND_WINDOW_MS = 60 * 60 * 1000;

  const position = $derived(snapshot?.position ?? null);
  const positionStale = $derived(silent || (position?.stale ?? false));
  const tiles = $derived(new Map(instrumentTiles(snapshot, silent).map((t) => [t.id, t])));
  const navTiles = $derived(navigationTiles(navigation, positionStale));
  const posTile = $derived(positionTile(snapshot, silent));
  const groups = $derived(systemTiles(snapshot, silent));
  const awa = $derived(snapshot?.readings.find((r) => r.quantity === 'windAngleApparent') ?? null);
  const tile = (id: string): Tile | undefined => tiles.get(id);
</script>

{#snippet metric(t: Tile | undefined, tone: string, size: 'xl' | 'l' | 'm')}
  {#if t}
    <div
      class="metric {size} {tone}"
      class:stale={t.stale}
      class:missing={t.missing}
      data-testid="tile-{t.id}"
    >
      <span class="label">
        {t.label}
        {#if t.stale}<span class="flag" aria-label="donnée obsolète">⚠</span>{/if}
      </span>
      <span class="reading">
        <span class="value">{t.value}</span>
        {#if t.unit}<span class="unit">{t.unit}</span>{/if}
      </span>
      {#if t.detail}<span class="detail">{t.detail}</span>{/if}
    </div>
  {/if}
{/snippet}

<section class="dashboard" aria-label="Instruments">
  <div class="status">{@render status()}</div>

  <div class="layout">
    <div class="column">
      <article class="card">
        {@render metric(tile('sog'), 'white', 'xl')}
        <hr />
        <div class="pair">
          {@render metric(tile('depth'), 'cyan', 'l')}
          {@render metric(tile('stw'), 'blue', 'l')}
        </div>
      </article>
      <article class="card">
        <div class="pair">
          {@render metric(tile('cog'), 'white', 'l')}
          {@render metric(tile('heading'), 'white', 'l')}
        </div>
      </article>
    </div>

    <article class="card wind">
      <div class="pair center">
        {@render metric(tile('aws'), 'orange', 'xl')}
        {@render metric(tile('awa'), 'yellow', 'xl')}
      </div>
      <WindDial awaRad={awa?.value ?? null} stale={silent || (awa?.stale ?? false)} />
      <TrendChart
        title="VENT APPARENT — 1 H"
        samples={history.windSpeedApparent ?? []}
        {now}
        windowMs={TREND_WINDOW_MS}
      />
    </article>

    <div class="column">
      <article class="card">
        <h2>
          NAVIGATION
          {#if navigation?.simulated}<span class="badge">SIMULÉE</span>{/if}
        </h2>
        {#if navigation}
          <p class="route">{navigation.routeName}</p>
          <div class="grid2">
            {#each navTiles as t, i (t.id)}
              <div class="cell">
                {@render metric(t, ['green', 'yellow', 'red', 'blue'][i] ?? 'white', 'm')}
              </div>
            {/each}
          </div>
        {:else}
          <p class="empty">Aucune route active.</p>
        {/if}
      </article>
      <article class="card">
        {@render metric(posTile, 'white', 'm')}
      </article>
      <SystemsCard readings={snapshot?.readings ?? []} {silent} {connection} />
    </div>
  </div>

  <div class="groups">
    <SystemGroup title="MOTEUR" tiles={groups.engine} tone="var(--tone-wind)" />
    <SystemGroup title="ÉNERGIE" tiles={groups.energy} tone="var(--tone-good)" />
    <SystemGroup title="RÉSERVOIRS" tiles={groups.tanks} tone="var(--tone-depth)" />
    <SystemGroup title="ENVIRONNEMENT" tiles={groups.environment} tone="var(--tone-angle)" />
  </div>

  <div class="history">
    <HistorySection {history} {now} />
  </div>
</section>

<style>
  .dashboard {
    position: absolute;
    inset: 0;
    z-index: 3;
    overflow: auto;
    padding: 1rem 1rem 2.5rem;
    background: var(--bg);
    color: var(--fg);
    font-family: system-ui, sans-serif;
  }

  .history {
    margin-top: 1.2rem;
  }

  .groups {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(20rem, 1fr));
    gap: 0.9rem;
    margin-top: 0.9rem;
  }

  .status {
    min-height: 1.8rem;
    margin-bottom: 0.75rem;
  }

  .layout {
    display: grid;
    grid-template-columns: minmax(16rem, 1fr) minmax(20rem, 1.4fr) minmax(16rem, 1fr);
    gap: 0.9rem;
    align-items: start;
  }

  @media (max-width: 1100px) {
    .layout {
      grid-template-columns: 1fr 1fr;
    }

    .wind {
      grid-column: 1 / -1;
      order: -1;
    }
  }

  .column {
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
  }

  .card {
    padding: 1rem 1.2rem;
    border: 1px solid var(--border);
    border-radius: 0.8rem;
    background: var(--card-bg);
  }

  .wind {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.6rem;
  }

  .wind :global(.trend) {
    align-self: stretch;
  }

  hr {
    margin: 0.9rem 0;
    border: 0;
    border-top: 1px solid var(--border);
  }

  .pair {
    display: flex;
    justify-content: space-between;
    gap: 1rem;
  }

  .pair.center {
    justify-content: center;
    gap: 3rem;
    text-align: center;
  }

  h2 {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin: 0 0 0.4rem;
    color: var(--muted);
    font-size: 0.9rem;
    letter-spacing: 0.12em;
  }

  .badge {
    padding: 0.15rem 0.6rem;
    border-radius: 1rem;
    background: var(--sim-bg);
    color: var(--sim-fg);
    font-size: 0.75rem;
  }

  .route {
    margin: 0 0 0.6rem;
    font-size: 0.95rem;
  }

  .grid2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.6rem;
  }

  .cell {
    padding: 0.6rem 0.8rem;
    border-radius: 0.5rem;
    background: var(--inset-bg);
  }

  .empty {
    color: var(--muted);
  }

  .metric {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .label {
    color: var(--muted);
    font-size: 0.85rem;
    font-weight: 600;
    letter-spacing: 0.12em;
  }

  .reading {
    display: flex;
    align-items: baseline;
    gap: 0.35rem;
  }

  .center .reading {
    justify-content: center;
  }

  .value {
    font-weight: 800;
    line-height: 1.05;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .xl .value {
    font-size: clamp(3rem, 7vw, 6rem);
  }

  .l .value {
    font-size: clamp(2.2rem, 4.5vw, 4rem);
  }

  .m .value {
    font-size: clamp(1.4rem, 2.6vw, 2.2rem);
  }

  .unit {
    color: var(--muted);
    font-size: clamp(1rem, 1.8vw, 1.6rem);
  }

  .detail {
    color: var(--muted);
    font-size: 0.8rem;
  }

  .white .value {
    color: var(--tone-main);
  }

  .cyan .value {
    color: var(--tone-depth);
  }

  .blue .value {
    color: var(--tone-water);
  }

  .yellow .value {
    color: var(--tone-angle);
  }

  .orange .value {
    color: var(--tone-wind);
  }

  .green .value {
    color: var(--tone-good);
  }

  .red .value {
    color: var(--tone-bad);
  }

  .missing .value {
    color: var(--muted);
  }

  .stale .value,
  .stale .unit {
    opacity: 0.4;
  }

  .stale .detail,
  .flag {
    color: var(--warn-fg);
  }
</style>
