<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import AisPanel from './lib/components/AisPanel.svelte';
  import InstrumentsPanel from './lib/components/InstrumentsPanel.svelte';
  import NavigationPanel from './lib/components/NavigationPanel.svelte';
  import PositionPanel from './lib/components/PositionPanel.svelte';
  import RoutesPanel from './lib/components/RoutesPanel.svelte';
  import SourcePanel from './lib/components/SourcePanel.svelte';
  import TracksPanel from './lib/components/TracksPanel.svelte';
  import { isEngineSilent } from './lib/format';
  import type { Theme } from './lib/map/style';
  import { subscribeAis, type AisSnapshot } from './lib/services/ais';
  import { fetchChartCatalog, type ChartCatalog } from './lib/services/charts';
  import { fetchEngineInfo, type EngineInfo, type InvokeFn } from './lib/services/engine';
  import {
    SourcesApi,
    subscribeConnection,
    subscribeInstruments,
    subscribeRejections,
    type ConnectionState,
    type InstrumentsSnapshot,
    type Rejections,
  } from './lib/services/instruments';
  import type { ListenFn } from './lib/services/position';
  import { NavigationController } from './lib/navigation.svelte';
  import { RoutingApi, subscribeNavigation } from './lib/services/routing';
  import { holdSubscription } from './lib/subscription';
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
  let snapshot = $state<InstrumentsSnapshot | null>(null);
  /** Réception du dernier instantané (horloge locale). */
  let snapshotAt = $state<number | null>(null);
  let snapshotError = $state<string | null>(null);
  let connection = $state<ConnectionState | null | undefined>(undefined);
  let connectionError = $state<string | null>(null);
  let rejections = $state<Rejections | undefined>(undefined);
  let ais = $state.raw<AisSnapshot | null>(null);
  let aisError = $state<string | null>(null);
  let selectedTarget = $state<string | null>(null);
  let now = $state(Date.now());
  let theme = $state<Theme>(loadTheme());
  let northUp = $state(true);
  let chartView = $state<{ recenter: () => void } | null>(null);

  // Fermetures : les appels utilisent toujours la fonction IPC courante.
  const sources = new SourcesApi((command, args) => invokeFn(command, args));
  const nav = new NavigationController(new RoutingApi((command, args) => invokeFn(command, args)));
  let navEventError = $state<string | null>(null);

  /** Période de rafraîchissement de la trace en cours d'enregistrement. */
  const RECORDING_REFRESH_MS = 5_000;
  /** Durée d'affichage des messages d'information. */
  const NOTICE_MS = 6_000;

  // Fraîcheur calculée par le moteur ; le silence du moteur lui-même est
  // détecté ici pour qu'un arrêt du flux reste visible.
  // Avant le premier instantané, le délai court depuis le démarrage.
  const startedAt = Date.now();
  const silent = $derived(isEngineSilent(snapshotAt ?? startedAt, now));
  const position = $derived(snapshot?.position ?? null);
  const fix = $derived(position?.fix ?? null);
  const stale = $derived(fix !== null && (silent || (position?.stale ?? false)));

  const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

  $effect(() => {
    fetchEngineInfo(invokeFn)
      .then((info) => (engine = info))
      .catch((e: unknown) => (engineError = message(e)));
    fetchChartCatalog(invokeFn)
      .then((c) => (catalog = c))
      .catch((e: unknown) => (catalogError = message(e)));
  });

  $effect(() =>
    holdSubscription(
      () =>
        subscribeInstruments(
          (s) => {
            snapshot = s;
            snapshotAt = Date.now();
            now = snapshotAt;
            snapshotError = null;
          },
          (e) => (snapshotError = e.message),
          listenFn,
        ),
      (e) => (snapshotError = message(e)),
    ),
  );

  $effect(() =>
    holdSubscription(
      () =>
        subscribeConnection(
          (c) => {
            connection = c;
            connectionError = null;
          },
          (e) => (connectionError = e.message),
          listenFn,
        ),
      (e) => (connectionError = message(e)),
    ),
  );

  $effect(() =>
    holdSubscription(
      () =>
        subscribeRejections(
          (r) => {
            rejections = r;
            connectionError = null;
          },
          (e) => (connectionError = e.message),
          listenFn,
        ),
      (e) => (connectionError = message(e)),
    ),
  );

  $effect(() =>
    holdSubscription(
      () =>
        subscribeAis(
          (a) => {
            ais = a;
            aisError = null;
          },
          (e) => (aisError = e.message),
          listenFn,
        ),
      (e) => (aisError = message(e)),
    ),
  );

  $effect(() => {
    void nav.refresh();
  });

  $effect(() =>
    holdSubscription(
      () =>
        subscribeNavigation(
          (status) => {
            nav.onStatus(status);
            navEventError = null;
          },
          (e) => (navEventError = e.message),
          listenFn,
        ),
      (e) => (navEventError = message(e)),
    ),
  );

  // Affichage en direct de la trace en cours d'enregistrement.
  $effect(() => {
    if (!nav.recordingId) return;
    const timer = setInterval(() => void nav.refreshRecording(), RECORDING_REFRESH_MS);
    return () => clearInterval(timer);
  });

  $effect(() => {
    if (!nav.notice) return;
    const timer = setTimeout(() => (nav.notice = null), NOTICE_MS);
    return () => clearTimeout(timer);
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
      <ChartView
        bind:this={chartView}
        {catalog}
        {fix}
        {stale}
        {theme}
        {northUp}
        routes={nav.routes}
        targetIndex={nav.status?.targetIndex ?? null}
        draft={nav.draft}
        tracks={nav.displayedTracks}
        drawing={nav.drawing}
        onmapclick={(p) => nav.addPoint(p)}
        {ais}
        onaisclick={(mmsi) => (selectedTarget = mmsi)}
      />
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

    <PositionPanel {position} {silent} error={snapshotError} />
    <InstrumentsPanel readings={snapshot?.readings ?? []} {silent} />

    <div class="toolbar">
      <button type="button" aria-pressed={northUp} onclick={() => (northUp = !northUp)}>
        {northUp ? 'Nord en haut ✓' : 'Rotation libre'}
      </button>
      <button type="button" onclick={() => (theme = theme === 'day' ? 'night' : 'day')}>
        {theme === 'day' ? 'Mode nuit' : 'Mode jour'}
      </button>
      <button type="button" disabled={!fix} onclick={() => chartView?.recenter()}> Centrer </button>
    </div>

    {#if nav.status}
      <NavigationPanel status={nav.status} {now} ondeactivate={() => nav.deactivate()} />
    {/if}
    {#if navEventError}
      <p class="error" role="alert">État de navigation rejeté : {navEventError}</p>
    {/if}
    {#if nav.error}
      <p class="error" role="alert">{nav.error}</p>
    {/if}
    {#if nav.notice}
      <p class="notice" role="status">{nav.notice}</p>
    {/if}

    <SourcePanel api={sources} {connection} {rejections} />
    {#if connectionError}
      <p class="error" role="alert">État de la source rejeté : {connectionError}</p>
    {/if}

    <AisPanel
      snapshot={ais}
      {silent}
      selected={selectedTarget}
      onselect={(mmsi) => (selectedTarget = mmsi)}
    />
    {#if aisError}
      <p class="error" role="alert">Cibles AIS rejetées : {aisError}</p>
    {/if}

    <RoutesPanel {nav} />
    <TracksPanel {nav} />

    <section class="charts" aria-label="Cartes">
      <h2>Cartes</h2>
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

  h2 {
    margin: 0.75rem 0 0.25rem;
    font-size: 1rem;
  }

  .notice {
    margin: 0.25rem 0;
    padding: 0.2rem 0.4rem;
    border-radius: 0.25rem;
    background: var(--warn-bg);
    color: var(--warn-fg);
    font-size: 0.85rem;
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
