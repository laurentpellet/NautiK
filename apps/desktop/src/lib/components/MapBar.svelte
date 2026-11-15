<script lang="ts">
  // Barre d'outils de l'onglet Carte : commandes de la carte, affichage
  // des panneaux, SOG et vent réel. Une valeur obsolète est estompée
  // et signalée ; une donnée absente reste « — ».
  import type { Snippet } from 'svelte';
  import Dropdown from './Dropdown.svelte';
  import Icon, { type IconName } from './Icon.svelte';
  import { instrumentTiles, systemTiles, type Tile } from '../instrumentTiles';
  import type { InstrumentsSnapshot } from '../services/instruments';
  import { PANEL_LABELS, type PanelId, type PanelLayout } from '../panels';
  import type { MapTool } from '../mapTools.svelte';

  interface Props {
    snapshot: InstrumentsSnapshot | null;
    silent: boolean;
    northUp: boolean;
    canCenter: boolean;
    layout: PanelLayout;
    /** Contenu du menu Calques, déroulé sous (ou sur) son bouton. */
    layersMenu: Snippet;
    /** Cartes et calques masqués : bouton marqué, liste dans l'infobulle. */
    hiddenLayers: string[];
    /** Deux cartes côte à côte. */
    split: boolean;
    /** Cartes côte à côte verrouillées (même vue). */
    locked: boolean;
    onsplit: () => void;
    onlock: () => void;
    /** Outil de la carte actif (un clic lui est destiné). */
    tool: MapTool;
    /** Le bateau peut être placé (mode simulateur). */
    canPlaceBoat: boolean;
    onplaceboat: () => void;
    onmeasure: () => void;
    onrangebearing: () => void;
    /** Contenu du menu Recherche. */
    searchMenu?: Snippet;
    onnorthup: () => void;
    oncenter: () => void;
    ontoggle: (id: PanelId) => void;
    /** Une trace est en cours d'enregistrement. */
    recording: boolean;
    /** Commande en cours (bouton désactivé). */
    recordBusy?: boolean;
    /** Démarre ou arrête l'enregistrement de la trace. */
    onrecord: () => void;
    /** Homme à la mer en cours. */
    mobActive: boolean;
    onmob: () => void;
    /** Ouvre la météo (fichiers GRIB) dans les Paramètres. */
    onweather?: () => void;
    /** Ouvre le journal de bord dans les Paramètres. */
    onlogbook?: () => void;
  }

  let {
    snapshot,
    silent,
    northUp,
    canCenter,
    layout,
    layersMenu,
    hiddenLayers,
    split,
    locked,
    onsplit,
    onlock,
    tool,
    canPlaceBoat,
    onplaceboat,
    onmeasure,
    onrangebearing,
    searchMenu,
    onnorthup,
    oncenter,
    ontoggle,
    recording,
    recordBusy = false,
    onrecord,
    mobActive,
    onmob,
    onweather,
    onlogbook,
  }: Props = $props();

  /** Valeurs affichées dans la barre, avec un libellé court. */
  const METRICS: { id: string; label: string }[] = [
    { id: 'sog', label: 'SOG' },
    { id: 'tws', label: 'VENT' },
  ];
  const TOGGLES: { id: PanelId; icon: IconName }[] = [
    { id: 'side', icon: 'panel' },
    { id: 'anchor', icon: 'anchor' },
    { id: 'live', icon: 'gauge' },
    { id: 'ais', icon: 'ais' },
    { id: 'route', icon: 'route' },
    { id: 'routeBar', icon: 'routebar' },
  ];
  const metrics = $derived.by(() => {
    // Vent réel : tuiles d'environnement (force).
    const all = [
      ...instrumentTiles(snapshot, silent),
      ...systemTiles(snapshot, silent).environment,
    ];
    const tiles = new Map(all.map((t) => [t.id, t]));
    return METRICS.flatMap(({ id, label }) => {
      const t = tiles.get(id);
      return t ? [{ ...t, label }] : [];
    }) satisfies Tile[];
  });
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
      title={hiddenLayers.length > 0 ? `Calques — masqué : ${hiddenLayers.join(', ')}` : 'Calques'}
      marked={hiddenLayers.length > 0}
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
    <button
      type="button"
      class="toggle"
      aria-label="Positionner le bateau ici"
      aria-pressed={tool === 'placeBoat'}
      disabled={!canPlaceBoat && tool !== 'placeBoat'}
      title={canPlaceBoat
        ? 'Positionner le bateau simulé : cliquer ensuite sur la carte (clic droit : menu)'
        : 'Positionner le bateau : seulement avec la source Simulateur'}
      onclick={onplaceboat}
    >
      <Icon name="boat" />
    </button>
    <button
      type="button"
      class="toggle"
      aria-label="Mesurer"
      aria-pressed={tool === 'measure'}
      title={tool === 'measure'
        ? 'Terminer la mesure'
        : 'Mesurer distances et relèvements : cliquer les points sur la carte'}
      onclick={onmeasure}
    >
      <Icon name="ruler" />
    </button>
    <button
      type="button"
      class="toggle"
      aria-label="Distance et relèvement (VRM/EBL)"
      aria-pressed={tool === 'rangeBearing'}
      title={tool === 'rangeBearing'
        ? 'Fermer VRM/EBL'
        : 'Cercle de distance et ligne de relèvement autour du bateau (VRM/EBL)'}
      onclick={onrangebearing}
    >
      <Icon name="vrm" />
    </button>
    {#if searchMenu}
      <Dropdown
        label="Rechercher"
        title="Rechercher une île, un amer, un waypoint, une position"
        testid="search-menu"
      >
        {#snippet button()}<Icon name="search" />{/snippet}
        {@render searchMenu()}
      </Dropdown>
    {/if}
  </div>

  <div class="group" role="toolbar" aria-label="Trace">
    <button
      type="button"
      class="record"
      class:on={recording}
      aria-label={recording ? "Arrêter l'enregistrement de la trace" : 'Enregistrer la trace'}
      title={recording
        ? "Enregistrement de la trace en cours (cliquer pour l'arrêter)"
        : 'Commencer à enregistrer la trace du bateau'}
      disabled={recordBusy}
      onclick={onrecord}
    >
      <Icon name={recording ? 'stop' : 'play'} />
    </button>
  </div>

  {#if onweather || onlogbook}
    <div class="group" role="toolbar" aria-label="Météo et journal">
      {#if onweather}
        <button
          type="button"
          aria-label="Météo"
          title="Météo : fichiers GRIB et vent"
          onclick={onweather}
        >
          <Icon name="weather" />
        </button>
      {/if}
      {#if onlogbook}
        <button
          type="button"
          aria-label="Journal de bord"
          title="Journal de bord"
          onclick={onlogbook}
        >
          <Icon name="log" />
        </button>
      {/if}
    </div>
  {/if}

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
  </div>

  <!-- Homme à la mer : en fin de barre, juste à gauche de l'état des
       données. -->
  <button
    type="button"
    class="mob"
    class:on={mobActive}
    aria-label="Homme à la mer"
    title={mobActive
      ? 'Homme à la mer en cours (le point n’est pas déplacé)'
      : 'Homme à la mer : marque le point MOB à la position du bateau (Ctrl+M)'}
    onclick={onmob}
  >
    <!-- Bouée couronne lancée vers la personne à la mer. -->
    <svg class="buoy" viewBox="0 0 32 32" aria-hidden="true" focusable="false">
      <path class="wave" d="M1 29.5q2.5-2.2 5 0t5 0t5 0t5 0t5 0t5 0" />
      <path class="rope" d="M8.7 21.3q-4.5 1.5-5.5 6.7" />
      <circle class="ring" cx="17" cy="13" r="9" />
      <path
        class="band"
        d="M21.2 17.2L25.5 21.5M12.8 17.2L8.5 21.5M12.8 8.8L8.5 4.5M21.2 8.8L25.5 4.5"
      />
      <circle class="edge" cx="17" cy="13" r="12" />
      <circle class="edge" cx="17" cy="13" r="6" />
    </svg>
  </button>
</div>

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.25rem 0.35rem;
    flex-wrap: wrap;
  }

  .mob {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0.1rem 0.25rem;
    border: 2px solid #e53935;
    border-radius: 0.35rem;
    background: #fff;
    cursor: pointer;
  }

  .buoy {
    width: 1.9rem;
    height: 1.9rem;
  }

  .buoy .ring {
    fill: none;
    stroke: #e53935;
    stroke-width: 6;
  }

  .buoy .band {
    fill: none;
    stroke: #fff;
    stroke-width: 4.2;
  }

  .buoy .edge {
    fill: none;
    stroke: #8e1b18;
    stroke-width: 0.8;
  }

  .buoy .rope {
    fill: none;
    stroke: #6d4c41;
    stroke-width: 1.3;
    stroke-linecap: round;
  }

  .buoy .wave {
    fill: none;
    stroke: #1e88e5;
    stroke-width: 1.4;
    stroke-linecap: round;
  }

  .mob:hover,
  .mob:focus-visible {
    background: #ffebee;
  }

  .mob.on {
    animation: mob-blink 1s steps(2, start) infinite;
  }

  @keyframes mob-blink {
    to {
      background: #e53935;
    }
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
    flex: none;
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
    white-space: nowrap;
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

  /* Enregistrement en cours : bouton rouge, bien visible. */
  .record.on {
    border-color: var(--alert-fg);
    background: var(--alert-bg);
    color: var(--alert-fg);
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
