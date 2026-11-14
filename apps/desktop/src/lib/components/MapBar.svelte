<script lang="ts">
  // Barre d'outils de l'onglet Carte : commandes de la carte, affichage
  // des panneaux et métriques de base. Une valeur obsolète est estompée et
  // signalée ; une donnée absente reste « — ».
  import type { Snippet } from 'svelte';
  import Dropdown from './Dropdown.svelte';
  import Icon, { type IconName } from './Icon.svelte';
  import { instrumentTiles, positionTile, type Tile } from '../instrumentTiles';
  import { PANEL_LABELS, type PanelId, type PanelLayout } from '../panels';
  import type { InstrumentsSnapshot } from '../services/instruments';

  interface Props {
    snapshot: InstrumentsSnapshot | null;
    silent: boolean;
    northUp: boolean;
    canCenter: boolean;
    layout: PanelLayout;
    /** Contenu du menu Calques, déroulé sous (ou sur) son bouton. */
    layersMenu: Snippet;
    /** Au moins une carte ou un calque masqué : bouton marqué. */
    layersHidden: boolean;
    /** Deux cartes côte à côte. */
    split: boolean;
    /** Cartes côte à côte verrouillées (même vue). */
    locked: boolean;
    onsplit: () => void;
    onlock: () => void;
    onnorthup: () => void;
    oncenter: () => void;
    ontoggle: (id: PanelId) => void;
    onresetlayout: () => void;
  }

  let {
    snapshot,
    silent,
    northUp,
    canCenter,
    layout,
    layersMenu,
    layersHidden,
    split,
    locked,
    onsplit,
    onlock,
    onnorthup,
    oncenter,
    ontoggle,
    onresetlayout,
  }: Props = $props();

  const METRICS = ['sog', 'cog', 'heading', 'depth'];
  const TOGGLES: { id: PanelId; icon: IconName }[] = [
    { id: 'side', icon: 'panel' },
    { id: 'anchor', icon: 'anchor' },
    { id: 'live', icon: 'gauge' },
  ];

  const metrics = $derived.by(() => {
    const tiles = new Map(instrumentTiles(snapshot, silent).map((t) => [t.id, t]));
    return METRICS.map((id) => tiles.get(id)).filter((t): t is Tile => t !== undefined);
  });
  const position = $derived(positionTile(snapshot, silent));
</script>

<div class="bar" data-testid="map-bar">
  <div class="group" role="toolbar" aria-label="Carte">
    <button
      type="button"
      aria-label="Nord en haut"
      aria-pressed={northUp}
      title={northUp
        ? 'Nord en haut (cliquer : rotation libre)'
        : 'Rotation libre (cliquer : nord en haut)'}
      onclick={onnorthup}
    >
      <Icon name={northUp ? 'north' : 'rotate'} />
    </button>
    <button
      type="button"
      aria-label="Centrer sur le bateau"
      title={canCenter ? 'Centrer sur le bateau' : 'Centrer : aucune position'}
      disabled={!canCenter}
      onclick={oncenter}
    >
      <Icon name="center" />
    </button>
    <Dropdown
      label="Calques"
      title={layersHidden ? 'Calques (certains sont masqués)' : 'Calques'}
      marked={layersHidden}
      testid="layers-menu"
    >
      {#snippet button()}<Icon name="layers" />{/snippet}
      {@render layersMenu()}
    </Dropdown>
    <button
      type="button"
      class="toggle"
      aria-label="Deux cartes côte à côte"
      aria-pressed={split}
      title={split ? 'Revenir à une seule carte' : 'Afficher deux cartes côte à côte'}
      onclick={onsplit}
    >
      <Icon name="split" />
    </button>
    {#if split}
      <button
        type="button"
        class="toggle"
        aria-label="Verrouiller les deux cartes"
        aria-pressed={locked}
        title={locked
          ? 'Cartes verrouillées : même zoom et même position (cliquer pour libérer)'
          : 'Cartes libres (cliquer pour verrouiller zoom et position)'}
        onclick={onlock}
      >
        <Icon name={locked ? 'lock' : 'unlock'} />
      </button>
    {/if}
  </div>

  <span class="sep" aria-hidden="true"></span>

  <div class="metrics" aria-label="Métriques">
    {#each metrics as t (t.id)}
      <div
        class="metric"
        class:stale={t.stale}
        class:missing={t.missing}
        data-metric={t.id}
        title={t.detail ?? undefined}
      >
        <span class="name">{t.label}</span>
        <span class="value"
          >{t.value}{#if t.unit}<small>{t.unit}</small>{/if}{#if t.stale}<span
              class="flag"
              aria-label="donnée obsolète">⚠</span
            >{/if}</span
        >
      </div>
    {/each}
    <div
      class="metric position"
      class:stale={position.stale}
      class:missing={position.missing}
      data-metric="position"
      title={position.detail ?? undefined}
    >
      <span class="name">POSITION</span>
      <span class="value"
        >{position.value}{#if position.stale}<span class="flag" aria-label="donnée obsolète">⚠</span
          >{/if}</span
      >
    </div>
  </div>

  <span class="sep" aria-hidden="true"></span>

  <div class="group" role="toolbar" aria-label="Panneaux">
    {#each TOGGLES as { id, icon } (id)}
      <button
        type="button"
        aria-label={PANEL_LABELS[id]}
        aria-pressed={layout[id].visible}
        title={layout[id].visible ? `Masquer ${PANEL_LABELS[id]}` : `Afficher ${PANEL_LABELS[id]}`}
        onclick={() => ontoggle(id)}
      >
        <Icon name={icon} />
      </button>
    {/each}
    <button
      type="button"
      aria-label="Disposition par défaut"
      title="Remettre tous les panneaux en place"
      onclick={onresetlayout}
    >
      <Icon name="reset" />
    </button>
  </div>
</div>

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.25rem 0.35rem;
    flex-wrap: wrap;
  }

  .group {
    display: flex;
    gap: 0.15rem;
  }

  .sep {
    align-self: stretch;
    width: 1px;
    margin: 0.2rem 0;
    background: var(--border);
  }

  .metrics {
    display: flex;
    gap: 0.9rem;
    flex: 1;
    justify-content: center;
    flex-wrap: wrap;
    padding: 0 0.3rem;
  }

  .metric {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    min-width: 3.4rem;
  }

  .name {
    color: var(--muted);
    font-size: 0.6rem;
    font-weight: 700;
    letter-spacing: 0.1em;
  }

  .value {
    font-size: 1.05rem;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .value small {
    margin-left: 0.15rem;
    color: var(--muted);
    font-size: 0.7rem;
    font-weight: 500;
  }

  .position .value {
    font-size: 0.9rem;
  }

  .stale .value {
    opacity: 0.5;
  }

  .missing .value {
    color: var(--muted);
  }

  .flag {
    margin-left: 0.2rem;
    color: var(--warn-fg);
    opacity: 1;
  }

  button {
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
    cursor: pointer;
  }

  button:hover:not(:disabled),
  button:focus-visible {
    border-color: var(--border);
    background: var(--button-bg);
  }

  button:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .toggle[aria-pressed='true'] {
    border-color: var(--border);
    background: var(--button-bg);
  }

  /* Panneau affiché : bouton marqué ; masqué : icône estompée. */
  [aria-label='Panneaux'] button[aria-pressed='true'] {
    border-color: var(--border);
    background: var(--button-bg);
  }

  [aria-label='Panneaux'] button[aria-pressed='false'] {
    color: var(--muted);
    opacity: 0.6;
  }
</style>
