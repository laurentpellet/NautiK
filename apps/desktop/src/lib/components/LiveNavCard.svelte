<script lang="ts">
  // Carte « Navigation (temps réel) » de l'onglet Carte : grandes
  // valeurs, compas et valeurs secondaires. Les valeurs obsolètes sont
  // estompées et signalées ; une donnée absente reste « — ».
  import { instrumentTiles, type Tile } from '../instrumentTiles';
  import type { InstrumentsSnapshot } from '../services/instruments';
  import CompassRose from './CompassRose.svelte';
  import { formatBearing, formatSpeed } from '../format';
  import type { OwnShip } from '../services/ownShip';
  import { navBar, shownNavFields } from '../navFields.svelte';

  interface Props {
    snapshot: InstrumentsSnapshot | null;
    silent: boolean;
    /** VMG et autre amure (moteur) ; absents sans vent ou position fraîche. */
    ownShip?: OwnShip | null;
    /** Bandeau fixe en bas de la carte, en très grands caractères. */
    docked?: boolean;
    /** Barre réduite : valeurs principales seules, plus petites mais
        lisibles. */
    collapsed?: boolean;
    /** Réduit ou déplie la barre (même bouton que la barre de route). */
    oncollapse?: (collapsed: boolean) => void;
    /** Place de la barre : en bas (horizontale), à gauche ou à droite
        (verticale). */
    placement?: 'left' | 'bottom' | 'right';
    /** Clic droit (ou appui long) : choix de la position. */
    oncontextmenu?: (event: MouseEvent) => void;
  }

  let {
    snapshot,
    silent,
    ownShip = null,
    docked = false,
    collapsed: barCollapsed = false,
    oncollapse,
    placement = 'bottom',
    oncontextmenu,
  }: Props = $props();

  // Réduire pousse la barre vers son bord, comme la barre de route.
  const arrows = $derived(
    placement === 'right' ? { fold: '»', unfold: '«' } : { fold: '«', unfold: '»' },
  );

  /** VMG signée : « −1.2 kn » en s'éloignant. */
  const vmg = (mps: number | null | undefined) =>
    mps === null || mps === undefined ? '—' : `${mps < 0 ? '−' : ''}${formatSpeed(Math.abs(mps))}`;
  const sailing = $derived([
    { id: 'vmgWind', label: 'VMG vent', value: vmg(ownShip?.vmgWindMps) },
    { id: 'vmgWaypoint', label: 'VMG WP', value: vmg(ownShip?.vmgWaypointMps) },
    {
      id: 'otherTack',
      label: 'Autre amure',
      value: formatBearing(ownShip?.otherTackCogRad ?? null),
    },
  ]);

  let collapsed = $state(false);

  const tiles = $derived(new Map(instrumentTiles(snapshot, silent).map((t) => [t.id, t])));
  /** Valeurs de la barre, dans l'ordre choisi (Paramètres → Instruments). */
  const fields = $derived(shownNavFields(navBar.fields));
  const MAIN = ['sog', 'heading', 'cog', 'aws'];
  const SECONDARY = ['depth', 'stw', 'awa', 'rudder'];

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

{#snippet sailingValues()}
  <div class="secondary" data-testid="sailing-values">
    {#each sailing as s (s.id)}
      <div class="value" class:stale={silent} class:missing={s.value === '—'} data-tile={s.id}>
        <span class="number">{s.value}</span>
        <span class="label">{s.label}</span>
      </div>
    {/each}
  </div>
{/snippet}

{#if docked}
  <!-- Bandeau du bas : compas, valeurs principales en très grand, puis
       valeurs secondaires ; lisible d'un coup d'œil depuis la barre. -->
  <section
    class="card docked {placement}"
    class:folded={barCollapsed}
    aria-label="Navigation temps réel"
    data-testid="instruments"
    title="Clic droit : position de la barre (gauche, bas, droite)"
    {oncontextmenu}
  >
    {#if oncollapse}
      <button
        type="button"
        class="fold"
        aria-label={barCollapsed ? 'Déplier la barre Navigation' : 'Réduire la barre Navigation'}
        aria-expanded={!barCollapsed}
        title={barCollapsed ? 'Déplier la barre Navigation' : 'Réduire la barre Navigation'}
        onclick={() => oncollapse(!barCollapsed)}
        >{barCollapsed ? arrows.unfold : arrows.fold}</button
      >
    {/if}
    <!-- Valeurs choisies dans les Paramètres, dans leur ordre. Réduite :
         compas masqué, mêmes valeurs aux tailles de la barre de route. -->
    {#if !barCollapsed}
      <div class="compass"><CompassRose {...compass} /></div>
    {/if}
    <div class="main">
      {#each fields as id (id)}
        {@render value(tiles.get(id), true)}
      {/each}
    </div>
  </section>
{:else}
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
      {@render sailingValues()}
    {/if}
  </section>
{/if}

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

  /* Barre fixe sous la carte, comme la barre du haut : toute la largeur,
     sans ombre ni coins arrondis ; valeurs très grandes et contrastées. */
  .docked {
    position: relative;
    flex: none;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: center;
    gap: 0.4rem 1.75rem;
    width: auto;
    min-width: 0;
    padding: 0.35rem 1rem 0.35rem 2.8rem;
    border: 0;
    border-top: 1px solid var(--border);
    border-radius: 0;
    background: var(--header-bg);
    box-shadow: none;
  }

  .docked .compass {
    width: 5.5rem;
    flex: none;
  }

  .docked .main,
  .docked .secondary {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 0.4rem 1.75rem;
  }

  .docked .value {
    min-width: 5.5rem;
  }

  /* Toutes les valeurs de la barre à la même taille, très lisible. */
  .docked .number,
  .docked .big .number {
    font-size: 3rem;
    font-weight: 800;
    line-height: 1.05;
  }

  .docked .label {
    color: var(--fg);
    font-size: 0.85rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    opacity: 0.75;
  }

  /* À gauche ou à droite : colonne sous la barre du haut, défilante. */
  .docked.left,
  .docked.right {
    flex-direction: column;
    flex-wrap: nowrap;
    justify-content: flex-start;
    width: 13.5rem;
    padding: 0.3rem 0.5rem 0.75rem;
    overflow-y: auto;
    border-top: 0;
  }

  .docked.left {
    border-right: 1px solid var(--border);
  }

  .docked.right {
    border-left: 1px solid var(--border);
  }

  .docked.left .main,
  .docked.left .secondary,
  .docked.right .main,
  .docked.right .secondary {
    flex-direction: column;
    flex-wrap: nowrap;
    align-items: center;
  }

  /* Bouton Réduire / Déplier, comme celui de la barre de route : en bas,
     en haut à gauche du bandeau. */
  .fold {
    position: absolute;
    top: 0.3rem;
    left: 0.35rem;
    z-index: 1;
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
    font: inherit;
    font-size: 1.1rem;
    font-weight: 700;
    cursor: pointer;
  }

  .fold:hover,
  .fold:focus-visible {
    border-color: var(--border);
    background: var(--button-bg);
  }

  /* Sur le côté : première ligne de la colonne, en haut, comme le bouton
     de la barre de route (rien ne le recouvre, même réduite). */
  .docked.left .fold,
  .docked.right .fold {
    position: static;
    flex: none;
    align-self: flex-start;
  }

  .docked.left.folded .fold,
  .docked.right.folded .fold {
    align-self: center;
  }

  /* Réduite : mêmes tailles de texte que la barre de route (valeur
     réduite : nom en haut 0.6rem, valeur 0.8rem, unité dessous 0.7rem). */
  .docked.folded {
    padding-top: 0.25rem;
    padding-bottom: 0.25rem;
  }

  .docked.folded .number,
  .docked.folded .big .number {
    font-size: 0.8rem;
    font-weight: 600;
    line-height: 1.2;
  }

  .docked.folded small {
    display: block;
    margin: 0;
    color: var(--muted);
    text-align: center;
    font-size: 0.7rem;
    font-weight: 500;
  }

  .docked.folded .label {
    order: -1;
    color: var(--muted);
    opacity: 1;
    font-size: 0.6rem;
    letter-spacing: 0.1em;
  }

  .docked.folded .value {
    min-width: 0;
  }

  /* Sur le côté, réduite : largeur au plus juste du contenu. */
  .docked.left.folded,
  .docked.right.folded {
    width: max-content;
    padding-right: 0.3rem;
    padding-left: 0.3rem;
  }
</style>
