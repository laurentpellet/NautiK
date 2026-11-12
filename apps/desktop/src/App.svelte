<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import PositionPanel from './lib/components/PositionPanel.svelte';
  import { isStale } from './lib/format';
  import type { Theme } from './lib/map/style';
  import { fetchChartCatalog, type ChartCatalog } from './lib/services/charts';
  import { fetchEngineInfo, type EngineInfo, type InvokeFn } from './lib/services/engine';
  import { subscribePosition, type ListenFn, type PositionFix } from './lib/services/position';
  import { loadTheme, saveTheme } from './lib/theme';

  interface Props {
    /** Fonction IPC injectable (tests). Par défaut : Tauri `invoke`. */
    invoke?: InvokeFn;
    /** Abonnement aux événements, injectable (tests). Par défaut : Tauri `listen`. */
    listen?: ListenFn;
  }

  let { invoke: invokeFn = invoke, listen: listenFn = listen }: Props = $props();

  let engine = $state<EngineInfo | null>(null);
  let engineError = $state<string | null>(null);
  let catalog = $state<ChartCatalog | null>(null);
  let catalogError = $state<string | null>(null);
  let fix = $state<PositionFix | null>(null);
  let fixError = $state<string | null>(null);
  let now = $state(Date.now());
  let theme = $state<Theme>(loadTheme());
  let northUp = $state(true);
  let chartView = $state<{ recenter: () => void } | null>(null);

  const stale = $derived(fix !== null && isStale(fix.measuredAt, now));

  const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

  $effect(() => {
    fetchEngineInfo(invokeFn)
      .then((info) => (engine = info))
      .catch((e: unknown) => (engineError = message(e)));
    fetchChartCatalog(invokeFn)
      .then((c) => (catalog = c))
      .catch((e: unknown) => (catalogError = message(e)));
  });

  $effect(() => {
    let unlisten: (() => void) | null = null;
    let disposed = false;
    subscribePosition(
      (f) => {
        fix = f;
        fixError = null;
      },
      (e) => (fixError = e.message),
      listenFn,
    )
      .then((u) => {
        if (disposed) u();
        else unlisten = u;
      })
      .catch((e: unknown) => (fixError = message(e)));
    return () => {
      disposed = true;
      unlisten?.();
    };
  });

  // Horloge d'affichage de l'âge des données.
  $effect(() => {
    const timer = setInterval(() => (now = Date.now()), 1000);
    return () => clearInterval(timer);
  });

  $effect(() => {
    saveTheme(theme);
    document.documentElement.dataset.theme = theme;
  });
</script>

<div class="app">
  {#if catalog}
    {#await import('./lib/components/ChartView.svelte') then { default: ChartView }}
      <ChartView bind:this={chartView} {catalog} {fix} {stale} {theme} {northUp} />
    {/await}
  {/if}

  <aside class="panel">
    <header>
      <h1>NautiK</h1>
      {#if engine}
        <span class="version" data-testid="engine-status">
          Moteur {engine.version} — protocole v{engine.protocolVersion}
        </span>
      {/if}
    </header>

    {#if engineError}
      <p class="error" role="alert">Moteur indisponible : {engineError}</p>
    {/if}

    <PositionPanel {fix} {stale} {now} error={fixError} />

    <div class="toolbar">
      <button type="button" aria-pressed={northUp} onclick={() => (northUp = !northUp)}>
        {northUp ? 'Nord en haut ✓' : 'Rotation libre'}
      </button>
      <button type="button" onclick={() => (theme = theme === 'day' ? 'night' : 'day')}>
        {theme === 'day' ? 'Mode nuit' : 'Mode jour'}
      </button>
      <button type="button" disabled={!fix} onclick={() => chartView?.recenter()}> Centrer </button>
    </div>

    <section class="charts" aria-label="Cartes">
      {#if catalogError}
        <p class="error" role="alert">Catalogue indisponible : {catalogError}</p>
      {:else if catalog}
        {#if catalog.error}
          <p class="error" role="alert">Cartes indisponibles : {catalog.error}</p>
        {:else if catalog.charts.length === 0}
          <p class="hint" data-testid="no-charts">
            Aucune carte. Déposez des fichiers <code>.mbtiles</code> dans :
            <code class="path">{catalog.chartsDir}</code>
            puis redémarrez NautiK.
          </p>
        {:else}
          <ul>
            {#each [...catalog.charts].reverse() as chart (chart.id)}
              <li title={chart.description ?? chart.fileName}>
                {chart.name} <span class="muted">z{chart.minZoom}–{chart.maxZoom}</span>
                {#if chart.warnings.length > 0}
                  <span class="warn" title={chart.warnings.join('\n')}>⚠</span>
                {/if}
              </li>
            {/each}
          </ul>
        {/if}
        {#each catalog.failures as failure (failure.fileName)}
          <p class="error" role="alert">Carte ignorée {failure.fileName} : {failure.reason}</p>
        {/each}
      {:else}
        <p class="muted">Chargement des cartes…</p>
      {/if}
    </section>
  </aside>

  {#if engine}
    <footer class="safety" role="note">{engine.safetyNotice}</footer>
  {/if}
</div>

<style>
  .app {
    position: relative;
    height: 100%;
    overflow: hidden;
    background: var(--bg);
  }

  .panel {
    position: absolute;
    top: 0.75rem;
    left: 0.75rem;
    z-index: 2;
    width: min(20rem, calc(100% - 5rem));
    max-height: calc(100% - 5rem);
    overflow: auto;
    padding: 0.75rem 1rem;
    border-radius: 0.5rem;
    background: var(--panel-bg);
    color: var(--fg);
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.25);
  }

  header {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  h1 {
    margin: 0;
    font-size: 1.25rem;
  }

  .version,
  .muted {
    color: var(--muted);
    font-size: 0.75rem;
  }

  .toolbar {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    margin: 0.5rem 0;
  }

  button {
    padding: 0.3rem 0.6rem;
    border: 1px solid var(--border);
    border-radius: 0.3rem;
    background: var(--button-bg);
    color: var(--fg);
    font: inherit;
    font-size: 0.85rem;
    cursor: pointer;
  }

  button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .charts ul {
    margin: 0.25rem 0;
    padding-left: 1.1rem;
    font-size: 0.85rem;
  }

  .hint {
    font-size: 0.85rem;
  }

  .path {
    display: block;
    margin: 0.25rem 0;
    word-break: break-all;
  }

  .warn {
    color: var(--alert-fg);
  }

  .error {
    color: var(--error-fg);
    font-size: 0.85rem;
    font-weight: 600;
  }

  .safety {
    position: absolute;
    left: 50%;
    bottom: 0.4rem;
    z-index: 2;
    transform: translateX(-50%);
    max-width: min(40rem, calc(100% - 14rem));
    padding: 0.2rem 0.6rem;
    border-radius: 0.3rem;
    background: var(--warn-bg);
    color: var(--warn-fg);
    font-size: 0.7rem;
    text-align: center;
  }
</style>
