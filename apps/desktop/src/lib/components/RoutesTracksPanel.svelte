<script lang="ts">
  // Onglets « Routes », « Traces » et « Marques » du panneau Routes et
  // traces. Les onglets restent montés (édition en cours conservée).
  import type { NavigationController } from '../navigation.svelte';
  import RoutesPanel from './RoutesPanel.svelte';
  import TracksPanel from './TracksPanel.svelte';
  import MarksPanel from './MarksPanel.svelte';
  import type { MarksController } from '../marks.svelte';
  import type { LatLon } from '../services/validate';

  interface Props {
    nav: NavigationController;
    marks?: MarksController;
    /** Position fraîche du bateau (marque au bateau). */
    boat?: LatLon | null;
    onzoom?: (position: LatLon) => void;
    onreplay?: (id: string, name: string) => void;
  }

  let { nav, marks, boat = null, onzoom = () => {}, onreplay }: Props = $props();

  type Tab = 'routes' | 'tracks' | 'marks';
  const TAB_KEY = 'nautik.routesTab';

  function loadTab(): Tab {
    try {
      const saved = globalThis.localStorage?.getItem(TAB_KEY);
      return saved === 'tracks' || saved === 'marks' ? saved : 'routes';
    } catch {
      return 'routes';
    }
  }

  let chosen = $state<Tab>(loadTab());
  // Une route ou une marque en cours d'édition s'affiche toujours.
  const tab = $derived<Tab>(nav.editor ? 'routes' : marks?.draft ? 'marks' : chosen);

  function select(next: Tab) {
    chosen = next;
    try {
      globalThis.localStorage?.setItem(TAB_KEY, next);
    } catch {
      // Préférence non mémorisée : sans conséquence.
    }
  }
</script>

<div class="tabs" role="tablist" aria-label="Routes et traces">
  <button
    type="button"
    role="tab"
    id="tab-routes"
    aria-selected={tab === 'routes'}
    aria-controls="tabpanel-routes"
    onclick={() => select('routes')}
  >
    Routes <span class="count">{nav.routes.length}</span>
  </button>
  <button
    type="button"
    role="tab"
    id="tab-tracks"
    aria-selected={tab === 'tracks'}
    aria-controls="tabpanel-tracks"
    disabled={nav.editor !== null}
    onclick={() => select('tracks')}
  >
    Traces <span class="count">{nav.tracks.length}</span>
    {#if nav.recordingId}<span class="rec" title="Enregistrement en cours">●</span>{/if}
  </button>
  {#if marks}
    <button
      type="button"
      role="tab"
      id="tab-marks"
      aria-selected={tab === 'marks'}
      aria-controls="tabpanel-marks"
      disabled={nav.editor !== null}
      onclick={() => select('marks')}
    >
      Marques <span class="count">{marks.marks.length}</span>
    </button>
  {/if}
</div>

<div id="tabpanel-routes" role="tabpanel" aria-labelledby="tab-routes" hidden={tab !== 'routes'}>
  <RoutesPanel {nav} />
</div>
<div id="tabpanel-tracks" role="tabpanel" aria-labelledby="tab-tracks" hidden={tab !== 'tracks'}>
  <TracksPanel {nav} {onreplay} />
</div>
{#if marks}
  <div id="tabpanel-marks" role="tabpanel" aria-labelledby="tab-marks" hidden={tab !== 'marks'}>
    <MarksPanel {marks} {boat} {onzoom} />
  </div>
{/if}

<style>
  .tabs {
    display: flex;
    gap: 0.2rem;
    margin: 0.4rem 0 0.3rem;
    border-bottom: 1px solid var(--border);
  }

  [role='tab'] {
    padding: 0.3rem 0.7rem;
    border: 1px solid transparent;
    border-bottom: 0;
    border-radius: 0.35rem 0.35rem 0 0;
    background: transparent;
    color: var(--muted);
    font: inherit;
    font-size: 0.85rem;
    font-weight: 600;
    cursor: pointer;
  }

  [role='tab'][aria-selected='true'] {
    margin-bottom: -1px;
    border-color: var(--border);
    background: var(--panel-bg);
    color: var(--fg);
  }

  [role='tab']:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .count {
    margin-left: 0.2rem;
    color: var(--muted);
    font-size: 0.75rem;
    font-weight: 500;
  }

  .rec {
    color: #2e7d32;
  }
</style>
