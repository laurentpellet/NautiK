<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { untrack } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import AisPanel from './lib/components/AisPanel.svelte';
  import AppHeader from './lib/components/AppHeader.svelte';
  import InstrumentsView from './lib/components/InstrumentsView.svelte';
  import LiveNavCard from './lib/components/LiveNavCard.svelte';
  import MapLayersMenu from './lib/components/MapLayersMenu.svelte';
  import ChartsFolderPanel from './lib/components/ChartsFolderPanel.svelte';
  import EncFolderPanel from './lib/components/EncFolderPanel.svelte';
  import { EncApi } from './lib/services/enc';
  import { EncMapController } from './lib/encMap.svelte';
  import FloatingPanel from './lib/components/FloatingPanel.svelte';
  import MapBar from './lib/components/MapBar.svelte';
  import {
    PANEL_LABELS,
    defaultLayout,
    loadLayout,
    saveLayout,
    type PanelId,
    type PanelLayout,
    type PanelState,
    type Rect,
  } from './lib/panels';
  import Dropdown from './lib/components/Dropdown.svelte';
  import {
    followedCamera,
    loadSplit,
    saveSplit,
    type MapCamera,
    type Pane,
    type SplitState,
  } from './lib/split';
  import SettingsView from './lib/components/SettingsView.svelte';
  import StatusStrip from './lib/components/StatusStrip.svelte';
  import { recordSnapshot, type History } from './lib/history';
  import {
    hiddenChartNames,
    hiddenLayers,
    loadHiddenCharts,
    loadLayers,
    saveHiddenCharts,
    saveLayers,
    vectorLabel,
    type MapLayers,
    type VectorCharts,
  } from './lib/layers';
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
  import { ChartsApi, fetchChartCatalog, type ChartCatalog } from './lib/services/charts';
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
  let hiddenCharts = $state<string[]>(loadHiddenCharts());
  // Une carte seule est toujours affichée : rien à choisir, et aucune
  // carte ne doit rester masquée sans moyen de la réafficher.
  const shownHiddenCharts = $derived((catalog?.charts.length ?? 0) > 1 ? hiddenCharts : []);
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
  /** Fenêtre en plein écran : en-tête, panneaux et alertes conservés. */
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
  let chartViewRight = $state<{ recenter: () => void } | null>(null);

  /** Deux cartes côte à côte (mémorisé). */
  let split = $state<SplitState>(loadSplit());
  /** Dernière vue de chaque volet, et volet manipulé en dernier. */
  let cameras = $state.raw<Record<Pane, MapCamera | null>>({ left: null, right: null });
  let leader = $state<Pane>('left');
  // Une carte seule est toujours affichée, dans les deux volets.
  const rightHiddenCharts = $derived((catalog?.charts.length ?? 0) > 1 ? split.hiddenCharts : []);
  const rightChartsLabel = $derived.by(() => {
    const shown = (catalog?.charts ?? []).filter((c) => !rightHiddenCharts.includes(c.id));
    const names = [...shown].reverse().map((c) => c.name);
    if (encFolderSet) names.unshift(...vectorLabel(rightVector, cm93Available));
    return names.length === 0 ? 'aucune carte' : names.join(', ');
  });

  function setSplit(next: SplitState): void {
    split = next;
    saveSplit(next);
  }

  function toggleSplit(): void {
    // À l'ouverture, le volet de droite part de la vue de gauche.
    leader = 'left';
    setSplit({ ...split, enabled: !split.enabled });
  }

  function toggleLock(): void {
    // Au verrouillage, le volet de droite rejoint celui de gauche.
    leader = 'left';
    setSplit({ ...split, locked: !split.locked });
  }

  function onCamera(pane: Pane, camera: MapCamera, user: boolean): void {
    // Appelé depuis les événements de la carte : lecture non suivie.
    cameras = { ...untrack(() => cameras), [pane]: camera };
    if (user) leader = pane;
  }

  function recenter(): void {
    leader = 'left';
    chartView?.recenter();
    if (split.enabled && !split.locked) chartViewRight?.recenter();
  }
  /** Disposition des panneaux de l'onglet Carte (mémorisée). */
  let layout = $state<PanelLayout>(loadLayout());
  let stageElement = $state<HTMLElement | null>(null);
  const mapArea = (): Rect | null => stageElement?.getBoundingClientRect() ?? null;

  function setLayout(next: PanelLayout): void {
    layout = next;
    saveLayout(next);
  }

  function updatePanel(id: PanelId, patch: Partial<PanelState>): void {
    setLayout({ ...layout, [id]: { ...layout[id], ...patch } });
  }

  /** Propriétés communes d'un panneau déplaçable. */
  function panel(id: PanelId) {
    return {
      title: PANEL_LABELS[id],
      panel: layout[id],
      area: mapArea,
      onmove: (x: number, y: number) => updatePanel(id, { x, y }),
      onreset: () => updatePanel(id, { x: null, y: null }),
      onhide: () => updatePanel(id, { visible: false }),
    };
  }

  // Fermetures : les appels utilisent toujours la fonction IPC courante.
  const sourcesApi = new SourcesApi((command, args) => invokeFn(command, args));
  const simulator = new SimulatorApi((command, args) => invokeFn(command, args));
  const chartsApi = new ChartsApi((command, args) => invokeFn(command, args));
  const encApi = new EncApi((command, args) => invokeFn(command, args));
  /** Cartes ENC de la vue (cellules décodées par le moteur). */
  const encMap = new EncMapController(encApi);

  /** Une base CM93 est trouvée dans le dossier ENC : son choix est proposé. */
  const cm93Available = $derived(Boolean(encMap.index?.cm93));
  /** Un dossier ENC est choisi : le choix ENC/CM93 a un sens. */
  const encFolderSet = $derived(Boolean(encMap.index?.dir));
  /** Cartes vectorielles de chaque volet (ENC officielles, CM93). */
  const leftVector = $derived<VectorCharts>({ enc: layers.enc, cm93: layers.cm93 });
  const rightVector = $derived<VectorCharts>(split.vector);
  /** Cartes vectorielles voulues dans au moins un volet affiché. */
  const wantedVector = $derived<VectorCharts>({
    enc: leftVector.enc || (split.enabled && rightVector.enc),
    cm93: leftVector.cm93 || (split.enabled && rightVector.cm93),
  });
  const leftEnc = $derived(encMap.layer(leftVector));
  const rightEnc = $derived(encMap.layer(rightVector));
  /** Cartes et calques masqués, rappelés sur la carte. */
  const hiddenOnMap = $derived([
    ...hiddenChartNames(catalog?.charts ?? [], shownHiddenCharts),
    ...hiddenLayers(layers, cm93Available),
  ]);

  // ENC de la zone affichée, après un court délai (déplacements rapides).
  $effect(() => {
    const view = mapView;
    const wanted = wantedVector;
    if (!wanted.enc && !wanted.cm93) {
      encMap.hide();
      return;
    }
    const timer = setTimeout(() => void encMap.show(view, wanted), 200);
    return () => clearTimeout(timer);
  });

  async function reloadEnc(): Promise<void> {
    await encMap.refresh();
    if (wantedVector.enc || wantedVector.cm93) await encMap.show(mapView, wantedVector);
  }
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

  function setHiddenCharts(next: string[]): void {
    hiddenCharts = next;
    saveHiddenCharts(next);
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
  <!-- En-tête conservé en plein écran : onglets, état des données, heure. -->
  <AppHeader
    tabs={TABS}
    {tab}
    onselect={(id) => (tab = id as typeof tab)}
    {pills}
    {now}
    {theme}
    ontheme={() => (theme = theme === 'day' ? 'night' : 'day')}
    {fullscreen}
    onfullscreen={() => setFullscreen(!fullscreen)}
  />
  <div class="stage" bind:this={stageElement}>
    {#if catalog}
      <!-- Nouveau dossier de cartes : la carte est reconstruite. -->
      {#key catalog}
        {#await import('./lib/components/ChartView.svelte') then { default: ChartView }}
          <!-- Un volet, ou deux côte à côte ; le volet de droite a son propre
               choix de cartes et les mêmes objets (bateau, routes, AIS…). -->
          <div class="maps" class:split={split.enabled}>
            <div class="pane">
              <ChartView
                bind:this={chartView}
                {catalog}
                hiddenCharts={shownHiddenCharts}
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
                enc={leftEnc}
                anchor={alarms?.anchor ?? null}
                {anchorAlerting}
                camera={followedCamera('left', split, leader, cameras)}
                oncamera={(c, user) => onCamera('left', c, user)}
              />
            </div>
            {#if split.enabled}
              <div class="pane" data-testid="split-pane">
                <ChartView
                  bind:this={chartViewRight}
                  {catalog}
                  hiddenCharts={rightHiddenCharts}
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
                  enc={rightEnc}
                  anchor={alarms?.anchor ?? null}
                  {anchorAlerting}
                  camera={followedCamera('right', split, leader, cameras)}
                  oncamera={(c, user) => onCamera('right', c, user)}
                />
                {#if catalog.charts.length > 1 || encFolderSet}
                  <div class="pane-charts">
                    <Dropdown
                      label="Cartes du volet droit"
                      title="Choisir les cartes du volet droit"
                      variant="chip"
                      marked={rightHiddenCharts.length > 0 ||
                        (encFolderSet && !rightVector.enc) ||
                        (cm93Available && !rightVector.cm93)}
                      testid="right-charts-menu"
                    >
                      {#snippet button()}
                        <span class="pane-label">Droite : {rightChartsLabel}</span>
                      {/snippet}
                      <MapLayersMenu
                        {layers}
                        onchange={setLayers}
                        charts={catalog.charts}
                        hiddenCharts={rightHiddenCharts}
                        onchartschange={(hidden) => setSplit({ ...split, hiddenCharts: hidden })}
                        vector={rightVector}
                        onvectorchange={(vector) => setSplit({ ...split, vector })}
                        cm93={cm93Available}
                        objects={false}
                      />
                    </Dropdown>
                  </div>
                {/if}
              </div>
            {/if}
          </div>
        {/await}
      {/key}
    {/if}

    {#snippet statusStrip()}
      <StatusStrip {position} {silent} {connection} />
    {/snippet}

    {#snippet chartsSection()}
      <ChartsFolderPanel api={chartsApi} {catalog} onchange={(c) => (catalog = c)} />
      <EncFolderPanel api={encApi} onchange={() => void reloadEnc()} />
    {/snippet}

    {#snippet alarmsSection()}
      <AlarmsPanel api={alarmsApi} {alarms} onchange={(a) => (alarms = a)} />
    {/snippet}

    <!-- Alarmes : visibles dans tous les onglets, en plein écran compris. -->
    <AlarmBanner
      --banner-top="0.6rem"
      alarms={alerting}
      simulated={alarms?.simulated ?? false}
      {soundUnavailable}
      onacknowledge={acknowledge}
      error={alarmsError}
    />

    {#if tab === 'chart'}
      <div class="map-side">
        <!-- Rappel des calques masqués : toujours visible, pas un panneau. -->
        {#if hiddenOnMap.length > 0}
          <p class="hidden-note" role="status" data-testid="hidden-layers">
            Masqué : {hiddenOnMap.join(', ')}
          </p>
        {/if}
        <!-- Cartes ENC : rien n'est omis en silence (pas à jour, annulée…). -->
        {#if encMap.warnings.length > 0}
          <p class="hidden-note enc-note" role="status" data-testid="enc-warnings">
            {encMap.warnings.join(' · ')}
          </p>
        {/if}
        {#if layout.anchor.visible}
          <FloatingPanel {...panel('anchor')} bare>
            <AnchorCard
              {alarms}
              canDrop={fix !== null && !stale}
              ondrop={dropAnchor}
              onraise={raiseAnchor}
            />
          </FloatingPanel>
        {/if}
        {#if layout.live.visible}
          <FloatingPanel {...panel('live')} bare>
            <LiveNavCard {snapshot} {silent} />
          </FloatingPanel>
        {/if}
      </div>
      {#if layout.bar.visible}
        <div class="bar-slot">
          <FloatingPanel {...panel('bar')} compact>
            <MapBar
              {snapshot}
              {silent}
              {northUp}
              canCenter={fix !== null}
              {layout}
              layersHidden={hiddenOnMap.length > 0}
              onnorthup={() => (northUp = !northUp)}
              oncenter={recenter}
              split={split.enabled}
              locked={split.locked}
              onsplit={toggleSplit}
              onlock={toggleLock}
              ontoggle={(id) => updatePanel(id, { visible: !layout[id].visible })}
              onresetlayout={() => setLayout(defaultLayout())}
            >
              {#snippet layersMenu()}
                <MapLayersMenu
                  {layers}
                  onchange={setLayers}
                  charts={catalog?.charts ?? []}
                  hiddenCharts={shownHiddenCharts}
                  onchartschange={setHiddenCharts}
                  cm93={cm93Available}
                />
              {/snippet}
            </MapBar>
          </FloatingPanel>
        </div>
      {:else}
        <button
          type="button"
          class="show-bar"
          title="Afficher la barre de la carte"
          onclick={() => updatePanel('bar', { visible: true })}
        >
          ▴ Barre
        </button>
      {/if}
      {#if weatherShown}
        <div class="wind-bar" class:above-bar={layout.bar.visible && layout.bar.x === null}>
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
      <SettingsView {simulator} status={statusStrip} alarms={alarmsSection} charts={chartsSection}>
        {#snippet sources()}
          <SourcePanel api={sourcesApi} {connection} {rejections} />
          {#if connectionError}
            <p class="error" role="alert">État de la source rejeté : {connectionError}</p>
          {/if}
        {/snippet}
      </SettingsView>
    {/if}

    <!-- Panneau latéral toujours monté (brouillon de route, sélection). -->
    <div class="side-slot" hidden={tab !== 'chart' || !layout.side.visible}>
      <FloatingPanel {...panel('side')}>
        <aside class="panel">
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
                  ou choisissez un autre dossier dans Paramètres → Cartes.
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
                <p class="error" role="alert">
                  Carte ignorée {failure.fileName} : {failure.reason}
                </p>
              {/each}
            {:else}
              <p class="muted">Chargement des cartes…</p>
            {/if}
          </section>
        </aside>
      </FloatingPanel>
    </div>
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

  .maps {
    position: absolute;
    inset: 0;
    display: flex;
  }

  .pane {
    position: relative;
    flex: 1;
    min-width: 0;
    min-height: 0;
  }

  .maps.split .pane + .pane {
    border-left: 2px solid var(--border);
  }

  /* Volet droit : échelle et zoom en bas à droite, hors de la barre. */
  .maps.split .pane + .pane :global(.maplibregl-ctrl-bottom-left) {
    right: 0;
    bottom: 1.6rem;
    left: auto;
  }

  .maps.split .pane + .pane :global(.zoom-level) {
    right: 10px;
    bottom: 3.8rem;
    left: auto;
  }

  .pane-charts {
    position: absolute;
    top: 0.6rem;
    left: 0.6rem;
    z-index: 2;
    max-width: calc(100% - 4rem);
  }

  .pane-label {
    overflow: hidden;
    max-width: 18rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Fenêtre étroite : volets l'un au-dessus de l'autre. */
  @media (max-width: 700px) {
    .maps.split {
      flex-direction: column;
    }

    .maps.split .pane + .pane {
      border-top: 2px solid var(--border);
      border-left: 0;
    }
  }

  .map-side {
    position: absolute;
    top: 0.75rem;
    right: 3.2rem;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 0.5rem;
    max-height: calc(100% - 6.5rem);
    overflow: auto;
  }

  .hidden-note {
    margin: 0;
    padding: 0.15rem 0.5rem;
    border-radius: 0.3rem;
    background: var(--warn-bg);
    color: var(--warn-fg);
    font-size: 0.75rem;
    font-weight: 600;
  }

  .enc-note {
    max-width: 28rem;
  }

  .wind-bar {
    position: absolute;
    bottom: 0.75rem;
    left: 50%;
    z-index: 2;
    transform: translateX(-50%);
  }

  .wind-bar.above-bar {
    bottom: 4.2rem;
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

  .side-slot[hidden] {
    display: none;
  }

  .side-slot {
    position: absolute;
    top: 0.75rem;
    left: 0.75rem;
    display: flex;
    flex-direction: column;
    max-height: calc(100% - 6.5rem);
  }

  .panel {
    width: min(20rem, calc(100vw - 5rem));
    padding: 0.5rem 1rem 0.75rem;
  }

  /* Barre centrée en bas, au-dessus de l'avertissement de sûreté. Pas de
     transform : un panneau déplacé est positionné dans la fenêtre. */
  .bar-slot {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0.6rem;
    width: fit-content;
    max-width: calc(100% - 1.5rem);
    margin: 0 auto;
  }

  .show-bar {
    position: absolute;
    left: 0.75rem;
    bottom: 0.4rem;
    z-index: 2;
    padding: 0.15rem 0.5rem;
    font-size: 0.75rem;
    opacity: 0.8;
  }

  .show-bar:hover,
  .show-bar:focus-visible {
    opacity: 1;
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
</style>
