<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import AisPanel from './lib/components/AisPanel.svelte';
  import AppHeader from './lib/components/AppHeader.svelte';
  import InstrumentsView from './lib/components/InstrumentsView.svelte';
  import LiveNavCard from './lib/components/LiveNavCard.svelte';
  import MapLayersMenu from './lib/components/MapLayersMenu.svelte';
  import SettingsView from './lib/components/SettingsView.svelte';
  import StatusStrip from './lib/components/StatusStrip.svelte';
  import { recordSnapshot, type History } from './lib/history';
  import { loadLayers, saveLayers, type MapLayers } from './lib/layers';
  import { statusPills } from './lib/shell';
  import { applyFullscreen, currentWindow, fullscreenShortcut } from './lib/fullscreen';
  import { SimulatorApi } from './lib/services/simulator';
  import {
    WeatherApi,
    type Bounds,
    type WeatherFile,
    type WindField,
  } from './lib/services/weather';
  import WeatherView from './lib/components/WeatherView.svelte';
  import WindTimeBar from './lib/components/WindTimeBar.svelte';
  import AlarmBanner from './lib/components/AlarmBanner.svelte';
  import AlarmsPanel from './lib/components/AlarmsPanel.svelte';
  import AnchorCard from './lib/components/AnchorCard.svelte';
  import { AlarmSound } from './lib/alarmSound';
  import {
    AlarmsApi,
    alertingAlarms,
    subscribeAlarms,
    type AlarmKind,
    type AlarmsSnapshot,
  } from './lib/services/alarms';
  import NavigationPanel from './lib/components/NavigationPanel.svelte';
  import PositionPanel from './lib/components/PositionPanel.svelte';
  import RoutesPanel from './lib/components/RoutesPanel.svelte';
  import SourcePanel from './lib/components/SourcePanel.svelte';
  import TracksPanel from './lib/components/TracksPanel.svelte';
  import { describeConnection, isEngineSilent } from './lib/format';
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
  /** Mesures fraîches reçues depuis le lancement (mémoire seule). */
  let history = $state.raw<History>({});
  let layers = $state<MapLayers>(loadLayers());
  let ais = $state.raw<AisSnapshot | null>(null);
  let aisError = $state<string | null>(null);
  let selectedTarget = $state<string | null>(null);
  /** Onglet affiché ; la carte reste montée sous les autres onglets. */
  let tab = $state<'chart' | 'instruments' | 'weather' | 'settings'>('chart');
  /** Emprise affichée sur la carte. */
  let mapView = $state.raw<Bounds | null>(null);
  /** Fichier météo affiché sur la carte et échéance choisie. */
  let weatherShown = $state.raw<WeatherFile | null>(null);
  let weatherStep = $state(0);
  let windField = $state.raw<WindField | null>(null);
  let windError = $state<string | null>(null);
  /** Nombre maximal de flèches de vent affichées. */
  const WIND_MAX_POINTS = 1_500;
  /** Fenêtre en plein écran : panneau latéral masqué, alertes conservées. */
  let fullscreen = $state(false);
  let fullscreenError = $state<string | null>(null);
  const TABS = [
    { id: 'chart', label: 'Carte' },
    { id: 'instruments', label: 'Instruments' },
    { id: 'weather', label: 'Météo' },
    { id: 'settings', label: 'Paramètres' },
  ] as const;
  let now = $state(Date.now());
  let theme = $state<Theme>(loadTheme());
  let northUp = $state(true);
  let chartView = $state<{ recenter: () => void } | null>(null);

  // Fermetures : les appels utilisent toujours la fonction IPC courante.
  const sourcesApi = new SourcesApi((command, args) => invokeFn(command, args));
  const simulator = new SimulatorApi((command, args) => invokeFn(command, args));
  const weatherApi = new WeatherApi((command, args) => invokeFn(command, args));
  const alarmsApi = new AlarmsApi((command, args) => invokeFn(command, args));

  /** État des alarmes, évaluées par le moteur. */
  let alarms = $state.raw<AlarmsSnapshot | null>(null);
  let alarmsError = $state<string | null>(null);
  let soundUnavailable = $state(false);
  const sound = new AlarmSound();
  const alerting = $derived(alertingAlarms(alarms));
  const anchorAlerting = $derived(alerting.some((a) => a.kind === 'anchor'));

  // Son piloté par l'état du moteur : seul l'acquittement ou la
  // résolution l'arrête.
  $effect(() => {
    sound.set(alarms?.sounding ?? false);
    soundUnavailable = sound.unavailable;
  });
  $effect(() => () => sound.dispose());

  async function acknowledge(kind: AlarmKind): Promise<void> {
    try {
      alarms = await alarmsApi.acknowledge(kind);
      alarmsError = null;
    } catch (e) {
      alarmsError = `Acquittement impossible : ${message(e)}`;
    }
  }

  async function dropAnchor(radiusM: number, delayS: number): Promise<void> {
    alarms = await alarmsApi.dropAnchor(radiusM, delayS);
  }

  async function raiseAnchor(): Promise<void> {
    alarms = await alarmsApi.raiseAnchor();
  }

  function showWeather(file: WeatherFile | null): void {
    weatherShown = file;
    windField = null;
    windError = null;
    // Échéance la plus proche de maintenant.
    const t = Date.now();
    const times = file?.validTimes ?? [];
    let best = 0;
    times.forEach((v, i) => {
      if (Math.abs(v - t) < Math.abs((times[best] ?? v) - t)) best = i;
    });
    weatherStep = best;
    if (file) tab = 'chart';
  }

  // Vent de l'échéance choisie dans la zone affichée (dernière demande seule).
  let windRequest = 0;
  $effect(() => {
    const file = weatherShown;
    const step = weatherStep;
    const area = mapView;
    if (!file) return;
    const id = ++windRequest;
    const timer = setTimeout(() => {
      weatherApi
        .windField(file.id, step, area, WIND_MAX_POINTS)
        .then((f) => {
          if (id !== windRequest) return;
          windField = f;
          windError = null;
        })
        .catch((e: unknown) => {
          if (id !== windRequest) return;
          windField = null;
          windError = `Vent indisponible : ${message(e)}`;
        });
    }, 150);
    return () => clearTimeout(timer);
  });
  const fullscreenWindow = currentWindow();

  async function setFullscreen(on: boolean): Promise<void> {
    try {
      fullscreen = await applyFullscreen(fullscreenWindow, on);
      fullscreenError = null;
    } catch (e) {
      fullscreenError = `Plein écran impossible : ${message(e)}`;
    }
  }

  function onKeydown(event: KeyboardEvent): void {
    const next = fullscreenShortcut(event.key, fullscreen);
    if (next === null) return;
    event.preventDefault();
    void setFullscreen(next);
  }

  // Alerte de liaison visible même si le panneau des sources est ailleurs.
  const linkAlert = $derived.by(() => {
    if (!connection) return null;
    const d = describeConnection(connection);
    return d.level === 'alert' ? d.label : null;
  });
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

  const pills = $derived(statusPills({ position, silent, connection, ais, alarms }));

  function setLayers(next: MapLayers): void {
    layers = next;
    saveLayers(next);
  }

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
            history = recordSnapshot(history, s);
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
    alarmsApi
      .get()
      .then((a) => (alarms = a))
      .catch((e: unknown) => (alarmsError = `Alarmes illisibles : ${message(e)}`));
  });

  $effect(() =>
    holdSubscription(
      () =>
        subscribeAlarms(
          (a) => {
            alarms = a;
            alarmsError = null;
          },
          (e) => (alarmsError = `Alarmes illisibles : ${e.message}`),
          listenFn,
        ),
      (e) => (alarmsError = `Alarmes illisibles : ${message(e)}`),
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

<svelte:window onkeydown={onKeydown} />

<div class="app">
  {#if !fullscreen}
    <AppHeader
      tabs={TABS}
      {tab}
      onselect={(id) => (tab = id as typeof tab)}
      {pills}
      {now}
      {theme}
      ontheme={() => (theme = theme === 'day' ? 'night' : 'day')}
      {fullscreen}
      onfullscreen={() => setFullscreen(true)}
    />
  {/if}
  <div class="stage">
    {#if catalog}
      {#await import('./lib/components/ChartView.svelte') then { default: ChartView }}
        <ChartView
          bind:this={chartView}
          {catalog}
          {fix}
          {stale}
          {theme}
          {northUp}
          routes={layers.routes ? nav.routes : []}
          targetIndex={nav.status?.targetIndex ?? null}
          draft={nav.draft}
          tracks={layers.tracks ? nav.displayedTracks : []}
          drawing={nav.drawing}
          onmapclick={(p) => nav.addPoint(p)}
          ais={layers.ais ? ais : null}
          onaisclick={(mmsi) => (selectedTarget = mmsi)}
          wind={weatherShown ? (windField?.points ?? null) : null}
          onviewchange={(b) => (mapView = b)}
          anchor={alarms?.anchor ?? null}
          {anchorAlerting}
        />
      {/await}
    {/if}

    {#snippet statusStrip()}
      <StatusStrip {position} {silent} {connection} />
    {/snippet}

    {#snippet alarmsSection()}
      <AlarmsPanel api={alarmsApi} {alarms} onchange={(a) => (alarms = a)} />
    {/snippet}

    <!-- Alarmes : visibles dans tous les onglets, en plein écran compris. -->
    <AlarmBanner
      --banner-top={fullscreen ? '3.4rem' : '0.6rem'}
      alarms={alerting}
      simulated={alarms?.simulated ?? false}
      {soundUnavailable}
      onacknowledge={acknowledge}
      error={alarmsError}
    />

    {#if fullscreen}
      <!-- Plein écran : en-tête masqué, onglets et sortie toujours accessibles. -->
      <div class="tabs compact">
        <div role="tablist" aria-label="Affichage">
          {#each TABS as t (t.id)}
            <button
              type="button"
              role="tab"
              aria-selected={tab === t.id}
              onclick={() => (tab = t.id)}
            >
              {t.label}
            </button>
          {/each}
        </div>
        <button
          type="button"
          class="fullscreen"
          aria-pressed="true"
          title="Quitter le plein écran (Échap)"
          onclick={() => setFullscreen(false)}
        >
          Quitter le plein écran
        </button>
      </div>
    {/if}
    {#if tab === 'chart'}
      <div class="map-side">
        <MapLayersMenu {layers} onchange={setLayers} />
        <AnchorCard
          {alarms}
          canDrop={fix !== null && !stale}
          ondrop={dropAnchor}
          onraise={raiseAnchor}
        />
        <LiveNavCard {snapshot} {silent} />
      </div>
      {#if weatherShown}
        <div class="wind-bar">
          <WindTimeBar
            file={weatherShown}
            step={weatherStep}
            field={windField}
            error={windError}
            {now}
            onstep={(s) => (weatherStep = s)}
            onclose={() => showWeather(null)}
          />
        </div>
      {/if}
    {/if}
    {#if fullscreenError}
      <p class="error floating" role="alert">{fullscreenError}</p>
    {/if}

    {#if tab === 'instruments'}
      <InstrumentsView
        {snapshot}
        {silent}
        navigation={nav.status}
        status={statusStrip}
        {history}
        {connection}
        {now}
      />
    {:else if tab === 'weather'}
      <WeatherView
        api={weatherApi}
        view={mapView}
        active={weatherShown?.id ?? null}
        onshow={showWeather}
        {now}
        status={statusStrip}
        listen={listenFn}
      />
    {:else if tab === 'settings'}
      <SettingsView {simulator} status={statusStrip} alarms={alarmsSection}>
        {#snippet sources()}
          <SourcePanel api={sourcesApi} {connection} {rejections} />
          {#if connectionError}
            <p class="error" role="alert">État de la source rejeté : {connectionError}</p>
          {/if}
        {/snippet}
      </SettingsView>
    {:else if fullscreen}
      <div class="fs-status" data-testid="fullscreen-status">
        <StatusStrip {position} {silent} {connection} compact />
      </div>
    {/if}

    <aside class="panel" hidden={tab !== 'chart' || fullscreen}>
      <header>
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

      <div class="toolbar">
        <button type="button" aria-pressed={northUp} onclick={() => (northUp = !northUp)}>
          {northUp ? 'Nord en haut ✓' : 'Rotation libre'}
        </button>
        <button type="button" disabled={!fix} onclick={() => chartView?.recenter()}>
          Centrer
        </button>
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

      {#if linkAlert}
        <p class="error" role="alert" data-testid="link-alert">Liaison : {linkAlert}</p>
      {/if}
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
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
    background: var(--bg);
  }

  .stage {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  .map-side {
    position: absolute;
    top: 0.75rem;
    right: 3.2rem;
    z-index: 2;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 0.5rem;
    max-height: calc(100% - 4rem);
    overflow: auto;
  }

  .wind-bar {
    position: absolute;
    bottom: 2.2rem;
    left: 50%;
    z-index: 2;
    transform: translateX(-50%);
  }

  .tabs {
    position: absolute;
    top: 0.6rem;
    left: 50%;
    z-index: 4;
    display: flex;
    gap: 0.4rem;
    transform: translateX(-50%);
  }

  .tabs [role='tablist'] {
    display: flex;
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    overflow: hidden;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.25);
  }

  .tabs button {
    padding: 0.4rem 1.1rem;
    border: 0;
    border-radius: 0;
    background: var(--panel-bg);
    font-size: 0.95rem;
    font-weight: 600;
  }

  .tabs button[aria-selected='true'] {
    background: var(--fg);
    color: var(--bg);
  }

  .tabs .fullscreen {
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.25);
  }

  .tabs.compact {
    opacity: 0.75;
  }

  .tabs.compact:hover,
  .tabs.compact:focus-within {
    opacity: 1;
  }

  .fs-status {
    position: absolute;
    top: 3.2rem;
    left: 0.75rem;
    z-index: 2;
    max-width: calc(100% - 5rem);
  }

  .floating {
    position: absolute;
    top: 3.2rem;
    right: 0.75rem;
    z-index: 5;
    padding: 0.2rem 0.5rem;
    border-radius: 0.3rem;
    background: var(--panel-bg);
  }

  .panel[hidden] {
    display: none;
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
