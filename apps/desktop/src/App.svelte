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
  import PoiPanel from './lib/components/PoiPanel.svelte';
  import { routeStyle } from './lib/routeColor.svelte';
  import { PoiApi, type Poi } from './lib/services/poi';
  import { emptyPoiCollection, poiData } from './lib/map/poi';
  import { EncMapController } from './lib/encMap.svelte';
  import FloatingPanel from './lib/components/FloatingPanel.svelte';
  import MapBar from './lib/components/MapBar.svelte';
  import MobButton from './lib/components/MobButton.svelte';
  import PlacementMenu from './lib/components/PlacementMenu.svelte';
  import {
    PANEL_LABELS,
    loadLayout,
    withNavigationShown,
    saveLayout,
    type PanelId,
    type PanelLayout,
    loadRouteBarCollapsed,
    saveRouteBarCollapsed,
    loadNavDock,
    saveNavDock,
    loadNavCollapsed,
    saveNavCollapsed,
    loadRouteBarSide,
    saveRouteBarSide,
    type NavDock,
    type RouteBarSide,
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
  import SettingsView, {
    loadSection,
    saveSection,
    type SettingsSection,
  } from './lib/components/SettingsView.svelte';
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
    visibleRoutes,
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
  import RoutesTracksPanel from './lib/components/RoutesTracksPanel.svelte';
  import { followBoat } from './lib/followBoat.svelte';
  import { driftProjection } from './lib/drift.svelte';
  import { chartNames, chartSort, cm93Name, withChartNames } from './lib/chartNames.svelte';
  import RouteBar from './lib/components/RouteBar.svelte';
  import SourcePanel from './lib/components/SourcePanel.svelte';
  import { describeConnection, formatAge, formatDistance, isEngineSilent } from './lib/format';
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
  import {
    RoutingApi,
    subscribeNavigation,
    type GeoBounds,
    type LatLon,
  } from './lib/services/routing';
  import { MapToolsController, type FromBoat } from './lib/mapTools.svelte';
  import MeasurePanel from './lib/components/MeasurePanel.svelte';
  import MapContextMenu from './lib/components/MapContextMenu.svelte';
  import MobPanel from './lib/components/MobPanel.svelte';
  import RangeBearingPanel from './lib/components/RangeBearingPanel.svelte';
  import { subscribeOwnShip, type OwnShip } from './lib/services/ownShip';
  import { aidsData } from './lib/map/aids';
  import { marksData } from './lib/map/marks';
  import { replayData } from './lib/map/replay';
  import { MarksController, boundsAround } from './lib/marks.svelte';
  import { MarksApi } from './lib/services/marks';
  import { ReplayController } from './lib/replay.svelte';
  import { getTrackProfile } from './lib/services/trackProfile';
  import { LogbookController } from './lib/logbook.svelte';
  import { LogbookApi } from './lib/services/logbook';
  import TrackReplayPanel from './lib/components/TrackReplayPanel.svelte';
  import SearchBox from './lib/components/SearchBox.svelte';
  import LogbookView from './lib/components/LogbookView.svelte';
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
  /** Cartes avec le nom choisi dans Paramètres → Cartes (affichage seul :
   *  la carte n'est pas recréée quand un nom change). */
  const namedCharts = $derived(withChartNames(catalog?.charts ?? [], chartNames.names));
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
  let tab = $state<'chart' | 'instruments' | 'settings'>('chart');
  /** Section des Paramètres affichée (Météo et Journal compris). */
  let settingsSection = $state<SettingsSection>(loadSection());

  /** Ouvre une section des Paramètres (Météo, Journal depuis la barre). */
  function openSettings(section: SettingsSection): void {
    settingsSection = section;
    saveSection(section);
    tab = 'settings';
  }
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
    { id: 'chart', label: 'Carte', icon: 'map' },
    { id: 'instruments', label: 'Instruments', icon: 'gauge' },
    { id: 'settings', label: 'Paramètres', icon: 'settings' },
  ] as const;
  let now = $state(Date.now());
  let theme = $state<Theme>(loadTheme());
  let northUp = $state(true);
  type ChartHandle = { recenter: () => void; fitBounds: (bounds: GeoBounds) => void };
  let chartView = $state<ChartHandle | null>(null);
  let chartViewRight = $state<ChartHandle | null>(null);

  /** Deux cartes côte à côte (mémorisé). */
  let split = $state<SplitState>(loadSplit());
  /** Dernière vue de chaque volet, et volet manipulé en dernier. */
  let cameras = $state.raw<Record<Pane, MapCamera | null>>({ left: null, right: null });
  let leader = $state<Pane>('left');
  // Une carte seule est toujours affichée, dans les deux volets.
  const rightHiddenCharts = $derived((catalog?.charts.length ?? 0) > 1 ? split.hiddenCharts : []);
  const rightChartsLabel = $derived.by(() => {
    const shown = namedCharts.filter((c) => !rightHiddenCharts.includes(c.id));
    const names = [...shown].reverse().map((c) => c.name);
    if (encFolderSet)
      names.unshift(...vectorLabel(rightVector, cm93Available, cm93Name.name, encAvailable));
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

  /** La carte défile avec le bateau (option « Suivre le bateau »). */
  let following = $state(false);
  const follow = $derived(following && followBoat.enabled);

  /** Centrer : une fois, ou suivi du bateau (interrupteur) si l'option est active. */
  function recenter(): void {
    if (followBoat.enabled) {
      following = !following;
      if (!following) return;
    }
    leader = 'left';
    chartView?.recenter();
    if (split.enabled && !split.locked) chartViewRight?.recenter();
  }

  // Cadrage demandé (route, trace) : volet gauche, et droit s'il est libre.
  $effect(() => {
    const focus = nav.focus;
    if (!focus) return;
    untrack(() => {
      leader = 'left';
      chartView?.fitBounds(focus.bounds);
      if (split.enabled && !split.locked) chartViewRight?.fitBounds(focus.bounds);
    });
  });
  /** Disposition des panneaux de l'onglet Carte (mémorisée). */
  // Le panneau Navigation est toujours affiché au lancement.
  let layout = $state<PanelLayout>(withNavigationShown(loadLayout()));
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
  /** Le dossier ENC contient des cellules S-57 : leur choix est proposé. */
  const encAvailable = $derived((encMap.index?.cells.length ?? 0) > 0);
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
    ...hiddenChartNames(namedCharts, shownHiddenCharts),
    ...hiddenLayers(layers, cm93Available, cm93Name.name, encAvailable),
  ]);

  // ENC de la zone affichée, après un court délai (déplacements rapides).
  $effect(() => {
    const view = mapView;
    const wanted = wantedVector;
    if (!wanted.enc && !wanted.cm93) {
      encMap.hide();
      // Index lu quand même : sans lui, la CM93 ne serait pas proposée
      // dans le menu Calques tant que les ENC restent décochées.
      if (untrack(() => encMap.index === null && encMap.error === null)) void encMap.refresh();
      return;
    }
    const timer = setTimeout(() => void encMap.show(view, wanted), 200);
    return () => clearTimeout(timer);
  });

  async function reloadEnc(): Promise<void> {
    await encMap.refresh();
    if (wantedVector.enc || wantedVector.cm93) await encMap.show(mapView, wantedVector);
  }

  // Îles et amers (OpenStreetMap) enregistrés : chargés au démarrage puis
  // après chaque téléchargement ou suppression.
  const poiApi = new PoiApi((command, args) => invokeFn(command, args));
  let pois = $state.raw<Poi[]>([]);
  const poiFeatures = $derived(poiData(pois));
  const shownPois = $derived(layers.poi ? poiFeatures : emptyPoiCollection());

  async function reloadPois(): Promise<void> {
    try {
      pois = (await poiApi.getPois()).pois;
    } catch (e) {
      // Sans amers enregistrés, la carte reste utilisable.
      console.warn('îles et amers indisponibles', e);
    }
  }

  $effect(() => {
    void reloadPois();
  });
  const weatherApi = new WeatherApi((command, args) => invokeFn(command, args));
  const alarmsApi = new AlarmsApi((command, args) => invokeFn(command, args));

  /** État des alarmes, évaluées par le moteur. */
  let alarms = $state.raw<AlarmsSnapshot | null>(null);
  let alarmsError = $state<string | null>(null);
  let soundUnavailable = $state(false);
  const sound = new AlarmSound();
  // L'homme à la mer a son propre grand panneau : pas en double dans le bandeau.
  const alerting = $derived(alertingAlarms(alarms).filter((a) => a.kind !== 'manOverboard'));
  const mob = $derived(alarms?.mob ?? null);
  const mobState = $derived(alarms?.alarms.find((a) => a.kind === 'manOverboard')?.state ?? 'off');
  let mobError = $state<string | null>(null);

  /** Homme à la mer : point posé par le moteur à la dernière position connue. */
  async function markMob(): Promise<void> {
    try {
      alarms = await alarmsApi.markMob();
      mobError = null;
    } catch (e) {
      mobError = `Homme à la mer : ${message(e)}`;
    }
  }

  async function endMob(): Promise<void> {
    alarms = await alarmsApi.clearMob();
  }
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
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'm') {
      event.preventDefault();
      if (!mob) void markMob();
      return;
    }
    if (event.key === 'Escape' && tools.picking) {
      event.preventDefault();
      tools.close();
      return;
    }
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
  const marks = new MarksController(new MarksApi((command, args) => invokeFn(command, args)));
  const replay = new ReplayController((id) =>
    getTrackProfile(id, (command, args) => invokeFn(command, args)),
  );
  const logbook = new LogbookController(
    new LogbookApi((command, args) => invokeFn(command, args)),
    (limit) => alarmsApi.events(limit),
  );
  const shownMarks = $derived(marksData(layers.marks ? marks.marks : []));
  const replayFeatures = $derived(
    replay.profile ? replayData(replay.profile.points, replay.index) : replayData([], null),
  );
  // Relecture arrêtée en quittant l'application.
  $effect(() => () => replay.stop());

  /** Cadre la carte sur un point (marque, résultat de recherche). */
  function zoomToPoint(position: LatLon): void {
    tab = 'chart';
    nav.zoomTo(boundsAround(position));
  }

  /**
   * Cible choisie dans le panneau AIS : sélectionnée et cadrée sur sa
   * dernière position connue (sans position valide : sélection seule).
   */
  function selectTarget(mmsi: string): void {
    selectedTarget = mmsi;
    const position = ais?.targets.find((t) => t.mmsi === mmsi)?.position;
    if (position) zoomToPoint(position);
  }

  /** Nouvelle marque : panneau d'édition affiché sur la carte. */
  function newMark(position: LatLon): void {
    marks.startNew(position);
  }

  // Outils de la carte : mesure et « bateau ici » (barre d'outils, clic droit).
  const tools = new MapToolsController(
    new RoutingApi((command, args) => invokeFn(command, args)),
    simulator,
    () => nav.speedMps,
    () => (fix && !stale ? { latitudeDeg: fix.latitudeDeg, longitudeDeg: fix.longitudeDeg } : null),
  );
  /** Mesure convertie en route enregistrée ; l'outil se ferme si c'est fait. */
  async function convertMeasure(): Promise<void> {
    if (await nav.createFromPoints(tools.points)) tools.close();
  }

  /** Longueurs des segments de la mesure (moteur), si le calcul est à jour. */
  const measureLegs = $derived(
    tools.plan && tools.plan.legs.length === tools.points.length - 1
      ? tools.plan.legs.map((l) => formatDistance(l.distanceM))
      : null,
  );
  /** Le bateau simulé peut être placé (le moteur refuse hors simulateur, rejeu compris). */
  const canPlaceBoat = $derived((alarms?.simulated ?? false) && !nav.drawing);

  interface ContextMenuState {
    pane: 'left' | 'right';
    position: LatLon;
    x: number;
    y: number;
  }
  let contextMenu = $state<ContextMenuState | null>(null);
  /** Distance et relèvement du bateau au point du menu (moteur). */
  let menuFromBoat = $state.raw<FromBoat>({ kind: 'pending' });

  // Calculés une fois à l'ouverture du menu ; une réponse tardive est ignorée.
  $effect(() => {
    const menu = contextMenu;
    if (!menu) return;
    let current = true;
    menuFromBoat = { kind: 'pending' };
    void untrack(() => tools.fromBoat(menu.position)).then((r) => {
      if (current) menuFromBoat = r;
    });
    return () => {
      current = false;
    };
  });

  /** Tracé de route ou marque en cours : édition sur la carte. */
  const editing = $derived(nav.editor !== null || marks.draft !== null);

  /** Fermer le panneau d'édition annule l'édition en cours. */
  function cancelEditing(): void {
    if (nav.editor) nav.cancelDrawing();
    if (marks.draft) marks.cancel();
  }

  // Depuis le gestionnaire (Paramètres → Routes) : cadrer, tracer, poser
  // une marque ou relire une trace se fait sur la carte, affichée aussitôt.
  let lastFocus = untrack(() => nav.focus?.seq ?? 0);
  let wasEditing = untrack(() => editing);
  let wasReplaying = untrack(() => replay.active);
  $effect(() => {
    const focus = nav.focus?.seq ?? 0;
    const edit = editing;
    const replaying = replay.active;
    untrack(() => {
      const started = focus !== lastFocus || (edit && !wasEditing) || (replaying && !wasReplaying);
      if (started && tab === 'settings') tab = 'chart';
    });
    lastFocus = focus;
    wasEditing = edit;
    wasReplaying = replaying;
  });

  /** « Naviguer jusqu'ici » : route directe activée (barre de route). */
  async function goTo(target: LatLon): Promise<void> {
    await nav.goTo(freshBoat, target);
  }

  /** Clic sur la carte : tracé de route d'abord, sinon l'outil actif. */
  function onMapClick(position: LatLon): void {
    contextMenu = null;
    if (nav.drawing) nav.addPoint(position);
    else void tools.click(position);
  }

  // Un tracé de route prend la main sur les outils.
  $effect(() => {
    if (nav.drawing) untrack(() => tools.close());
  });

  // Message « bateau déplacé » effacé après quelques secondes.
  $effect(() => {
    if (!tools.notice) return;
    const timer = setTimeout(() => (tools.notice = null), NOTICE_MS);
    return () => clearTimeout(timer);
  });
  /** Aides du bateau (vecteurs, VMG, laylines), calculées par le moteur. */
  let ownShip = $state.raw<OwnShip | null>(null);
  const aids = $derived(
    aidsData({
      own: ownShip,
      vectors: layers.vectors,
      laylines: layers.laylines,
      mob: mob?.position ?? null,
      rangeBearing: tools.tool === 'rangeBearing' ? tools.rangeBearing : null,
      driftMinutes: layers.drift ? driftProjection.minutes : null,
    }),
  );
  const mobMarker = $derived(mob ? { position: mob.position, approximate: mob.approximate } : null);

  /** Routes de la carte : la route active reste affichée même calque masqué. */
  const shownRoutes = $derived(visibleRoutes(nav.routes, layers));
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
  /** Position fraîche du bateau (marque au bateau). */
  const freshBoat = $derived<LatLon | null>(
    fix && !stale ? { latitudeDeg: fix.latitudeDeg, longitudeDeg: fix.longitudeDeg } : null,
  );
  /** Navigation calculée sur une position obsolète ou absente : signalée. */
  const navStale = $derived(silent || position === null || position.stale);
  /** Barre de route latérale, collée à gauche de la carte (route active). */
  const routeSide = $derived(nav.status !== null && layout.routeBar.visible && tab === 'chart');
  let routeBarCollapsed = $state(loadRouteBarCollapsed());

  function collapseRouteBar(collapsed: boolean): void {
    routeBarCollapsed = collapsed;
    saveRouteBarCollapsed(collapsed);
  }

  // Positions des barres (clic droit) : Navigation à gauche, en bas ou à
  // droite ; barre de route à gauche ou à droite de la carte. Mémorisées.
  let navDock = $state<NavDock>(loadNavDock());
  let routeBarSide = $state<RouteBarSide>(loadRouteBarSide());
  let placementMenu = $state<{ kind: 'nav' | 'route'; x: number; y: number } | null>(null);

  const NAV_DOCK_OPTIONS = [
    { id: 'left', label: 'À gauche' },
    { id: 'bottom', label: 'En bas' },
    { id: 'right', label: 'À droite' },
  ] as const satisfies readonly { id: NavDock; label: string }[];
  const ROUTE_SIDE_OPTIONS = [
    { id: 'left', label: 'À gauche de la carte' },
    { id: 'right', label: 'À droite de la carte' },
  ] as const satisfies readonly { id: RouteBarSide; label: string }[];

  function openPlacementMenu(kind: 'nav' | 'route', event: MouseEvent): void {
    event.preventDefault();
    placementMenu = { kind, x: event.clientX, y: event.clientY };
  }

  let navCollapsed = $state(loadNavCollapsed());

  function collapseNav(collapsed: boolean): void {
    navCollapsed = collapsed;
    saveNavCollapsed(collapsed);
  }

  function setNavDock(dock: NavDock): void {
    navDock = dock;
    saveNavDock(dock);
  }

  function setRouteBarSide(side: RouteBarSide): void {
    routeBarSide = side;
    saveRouteBarSide(side);
  }

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

  $effect(() =>
    holdSubscription(
      () =>
        subscribeOwnShip(
          (o) => (ownShip = o),
          (e) => console.warn('aides du bateau rejetées', e),
          listenFn,
        ),
      (e) => console.warn('aides du bateau indisponibles', e),
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

  $effect(() => {
    void marks.refresh();
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

<!-- Erreur et avis des routes et traces (gestionnaire et édition). -->
{#snippet navMessages()}
  {#if nav.error}
    <p class="error" role="alert">{nav.error}</p>
  {/if}
  {#if nav.notice}
    <p class="notice" role="status">{nav.notice}</p>
  {/if}
{/snippet}

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
    tools={tab === 'chart' ? mapTools : undefined}
    afterStatus={mobButton}
  />

  <!-- Homme à la mer : à droite de l'état des données, dans tous les onglets. -->
  {#snippet mobButton()}
    <MobButton
      active={mob !== null}
      onmob={() => {
        if (!mob) void markMob();
      }}
    />
  {/snippet}

  <!-- Outils de la carte, intégrés à la barre du haut (onglet Carte). -->
  {#snippet mapTools()}
    <MapBar
      {snapshot}
      {silent}
      {northUp}
      canCenter={fix !== null}
      following={followBoat.enabled ? following : null}
      routeActive={nav.status !== null}
      {layout}
      hiddenLayers={hiddenOnMap}
      onnorthup={() => (northUp = !northUp)}
      oncenter={recenter}
      split={split.enabled}
      locked={split.locked}
      onsplit={toggleSplit}
      onlock={toggleLock}
      tool={tools.tool}
      {canPlaceBoat}
      onplaceboat={() => tools.togglePlaceBoat()}
      onmeasure={() => {
        if (!nav.drawing) tools.toggleMeasure();
      }}
      onrangebearing={() => {
        if (!nav.drawing) tools.toggleRangeBearing();
      }}
      ontoggle={(id) => updatePanel(id, { visible: !layout[id].visible })}
      recording={nav.recordingId !== null}
      recordBusy={nav.busy}
      onrecord={() => void (nav.recordingId ? nav.stopRecording() : nav.startRecording())}
      onroutes={() => openSettings('routes')}
      onweather={() => openSettings('weather')}
      onlogbook={() => openSettings('logbook')}
    >
      {#snippet searchMenu()}
        <SearchBox onselect={(place) => zoomToPoint(place.position)} />
      {/snippet}
      {#snippet layersMenu()}
        <MapLayersMenu
          {layers}
          onchange={setLayers}
          charts={namedCharts}
          hiddenCharts={shownHiddenCharts}
          onchartschange={setHiddenCharts}
          cm93={cm93Available}
          cm93Name={cm93Name.name}
          sort={chartSort.value}
          enc={encAvailable}
        />
      {/snippet}
    </MapBar>
  {/snippet}
  <!-- Carte et barre Navigation (à gauche, en bas ou à droite). -->
  <div class="workspace nav-{navDock}">
    <div
      class="stage"
      class:route-side={routeSide}
      class:route-side-collapsed={routeSide && routeBarCollapsed}
      class:route-right={routeBarSide === 'right'}
      bind:this={stageElement}
    >
      <!-- Route active : barre latérale collée à gauche (ou à droite) de la
         carte, qui se décale ; clic droit : choix du côté. -->
      {#if routeSide && nav.status}
        <aside
          class="route-side-bar"
          class:right={routeBarSide === 'right'}
          aria-label="Barre de route"
          title="Clic droit : côté de la barre de route"
          oncontextmenu={(e) => openPlacementMenu('route', e)}
        >
          <RouteBar
            status={nav.status}
            {now}
            stale={navStale}
            collapsed={routeBarCollapsed}
            onroutes={() => openSettings('routes')}
            ondeactivate={() => nav.deactivate()}
            oncollapse={collapseRouteBar}
            side={routeBarSide}
          />
        </aside>
      {/if}
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
                  {follow}
                  onunfollow={() => (following = false)}
                  {catalog}
                  hiddenCharts={shownHiddenCharts}
                  {fix}
                  {stale}
                  {theme}
                  {northUp}
                  routes={shownRoutes}
                  targetIndex={nav.status?.targetIndex ?? null}
                  activeLegs={nav.activeLegs}
                  draft={nav.draft}
                  editedRouteId={nav.editor?.routeId ?? null}
                  onpointmove={(i, p) => nav.movePoint(i, p)}
                  tracks={layers.tracks ? nav.displayedTracks : []}
                  drawing={nav.drawing || tools.picking}
                  onmapclick={onMapClick}
                  measure={tools.points}
                  {measureLegs}
                  oncontextmenu={(position, point) =>
                    (contextMenu = { pane: 'left', position, ...point })}
                  ais={layers.ais ? ais : null}
                  onaisclick={(mmsi) => (selectedTarget = mmsi)}
                  wind={weatherShown ? (windField?.points ?? null) : null}
                  onviewchange={(b) => (mapView = b)}
                  enc={leftEnc}
                  pois={shownPois}
                  activeRouteColor={routeStyle.activeColor}
                  anchor={alarms?.anchor ?? null}
                  {anchorAlerting}
                  {aids}
                  mob={mobMarker}
                  marks={shownMarks}
                  replay={replayFeatures}
                  camera={followedCamera('left', split, leader, cameras)}
                  oncamera={(c, user) => onCamera('left', c, user)}
                />
                {#if contextMenu?.pane === 'left'}
                  {@render mapMenu(contextMenu)}
                {/if}
              </div>
              {#if split.enabled}
                <div class="pane" data-testid="split-pane">
                  <ChartView
                    bind:this={chartViewRight}
                    follow={follow && !split.locked}
                    onunfollow={() => (following = false)}
                    {catalog}
                    hiddenCharts={rightHiddenCharts}
                    {fix}
                    {stale}
                    {theme}
                    {northUp}
                    routes={shownRoutes}
                    targetIndex={nav.status?.targetIndex ?? null}
                    activeLegs={nav.activeLegs}
                    draft={nav.draft}
                    editedRouteId={nav.editor?.routeId ?? null}
                    onpointmove={(i, p) => nav.movePoint(i, p)}
                    tracks={layers.tracks ? nav.displayedTracks : []}
                    drawing={nav.drawing || tools.picking}
                    onmapclick={onMapClick}
                    measure={tools.points}
                    {measureLegs}
                    oncontextmenu={(position, point) =>
                      (contextMenu = { pane: 'right', position, ...point })}
                    ais={layers.ais ? ais : null}
                    onaisclick={(mmsi) => (selectedTarget = mmsi)}
                    wind={weatherShown ? (windField?.points ?? null) : null}
                    enc={rightEnc}
                    pois={shownPois}
                    activeRouteColor={routeStyle.activeColor}
                    anchor={alarms?.anchor ?? null}
                    {anchorAlerting}
                    {aids}
                    mob={mobMarker}
                    marks={shownMarks}
                    replay={replayFeatures}
                    camera={followedCamera('right', split, leader, cameras)}
                    oncamera={(c, user) => onCamera('right', c, user)}
                  />
                  {#if contextMenu?.pane === 'right'}
                    {@render mapMenu(contextMenu)}
                  {/if}
                  {#if catalog.charts.length > 1 || encFolderSet}
                    <div class="pane-charts">
                      <Dropdown
                        label="Cartes du volet droit"
                        title="Choisir les cartes du volet droit"
                        variant="chip"
                        marked={rightHiddenCharts.length > 0 ||
                          (encAvailable && !rightVector.enc) ||
                          (cm93Available && !rightVector.cm93)}
                        testid="right-charts-menu"
                      >
                        {#snippet button()}
                          <span class="pane-label">Droite : {rightChartsLabel}</span>
                        {/snippet}
                        <MapLayersMenu
                          {layers}
                          onchange={setLayers}
                          charts={namedCharts}
                          hiddenCharts={rightHiddenCharts}
                          onchartschange={(hidden) => setSplit({ ...split, hiddenCharts: hidden })}
                          vector={rightVector}
                          onvectorchange={(vector) => setSplit({ ...split, vector })}
                          cm93={cm93Available}
                          cm93Name={cm93Name.name}
                          sort={chartSort.value}
                          enc={encAvailable}
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

      {#snippet mapMenu(menu: ContextMenuState)}
        <MapContextMenu
          position={menu.position}
          x={menu.x}
          y={menu.y}
          {canPlaceBoat}
          onplaceboat={(at) => void tools.placeBoat(at)}
          canMeasure={!nav.drawing}
          onmeasure={(at) => void tools.measureFrom(at)}
          onmark={newMark}
          fromBoat={menuFromBoat}
          ongoto={(at) => void goTo(at)}
          goToBlocked={nav.drawing
            ? 'Indisponible pendant le tracé d’une route'
            : freshBoat
              ? null
              : 'Position du bateau inconnue ou obsolète'}
          onclose={() => (contextMenu = null)}
        />
      {/snippet}

      {#snippet statusStrip()}
        <StatusStrip {position} {silent} {connection} />
        {#if engine}
          <p class="muted" data-testid="engine-status">
            Moteur {engine.version} — protocole v{engine.protocolVersion}
          </p>
        {/if}
      {/snippet}

      {#snippet chartsSection()}
        <ChartsFolderPanel api={chartsApi} {catalog} onchange={(c) => (catalog = c)} />
        <EncFolderPanel api={encApi} onchange={() => void reloadEnc()} />
        <PoiPanel api={poiApi} onchange={() => void reloadPois()} />
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

      <!-- Homme à la mer : grand affichage, dans tous les onglets. -->
      {#if mob}
        <div class="mob-slot">
          <MobPanel
            {mob}
            alarmState={mobState}
            {now}
            stale={fix === null || stale}
            onacknowledge={() => void acknowledge('manOverboard')}
            onend={endMob}
          />
        </div>
      {/if}
      {#if mobError}
        <p class="error floating mob-error" role="alert">{mobError}</p>
      {/if}

      {#if tab === 'chart'}
        <!-- Outils de la carte : mesure, « bateau ici » et leurs messages. -->
        {#if tools.picking || tools.error || tools.notice || replay.active}
          <div class="tool-side">
            {#if replay.active}
              <TrackReplayPanel {replay} />
            {/if}
            {#if tools.measuring}
              <MeasurePanel {tools} onconvert={convertMeasure} />
            {:else if tools.tool === 'rangeBearing'}
              <RangeBearingPanel {tools} boatKnown={fix !== null && !stale} />
            {:else}
              {#if tools.tool === 'placeBoat'}
                <p class="tool-note" role="status">
                  Cliquez sur la carte pour placer le bateau simulé (Échap : annuler).
                </p>
              {/if}
              {#if tools.error}
                <p class="tool-note error" role="alert">{tools.error}</p>
              {/if}
            {/if}
            {#if tools.notice}
              <p class="tool-note" role="status">{tools.notice}</p>
            {/if}
          </div>
        {/if}
        <div class="map-side">
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
          <!-- Cibles AIS : ouvert par l'icône AIS de la barre d'outils. -->
          {#if layout.ais.visible}
            <FloatingPanel {...panel('ais')}>
              <section class="panel ais-panel" aria-label="Panneau Cibles AIS">
                <AisPanel
                  snapshot={ais}
                  {silent}
                  selected={selectedTarget}
                  onselect={selectTarget}
                />
                {#if aisError}
                  <p class="error" role="alert">Cibles AIS rejetées : {aisError}</p>
                {/if}
              </section>
            </FloatingPanel>
          {/if}
          <!-- État de navigation rejeté : jamais masqué avec un panneau. -->
          {#if navEventError}
            <p class="hidden-note nav-error" role="alert">
              État de navigation rejeté : {navEventError}
            </p>
          {/if}
          <!-- Édition sur la carte (tracé de route, marque) : petit panneau
               affiché le temps de l'édition ; la gestion complète est dans
               Paramètres → Routes. Fermer annule l'édition. -->
          {#if editing}
            <div class="route-slot">
              <FloatingPanel
                {...panel('route')}
                panel={{ ...layout.route, visible: true }}
                onhide={cancelEditing}
              >
                <section class="panel" aria-label="Édition sur la carte">
                  {@render navMessages()}
                  <RoutesTracksPanel
                    {nav}
                    {marks}
                    boat={freshBoat}
                    onzoom={zoomToPoint}
                    onreplay={(id, name) => void replay.load(id, name)}
                  />
                </section>
              </FloatingPanel>
            </div>
          {/if}
        </div>
        <!-- Barres du bas empilées (vent, route, outils) : une barre qui passe
           sur plusieurs lignes repousse les autres, sans recouvrement. -->
        <div class="bottom-bars">
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
        </div>
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
      {:else if tab === 'settings'}
        <SettingsView
          {simulator}
          status={statusStrip}
          alarms={alarmsSection}
          charts={chartsSection}
          bind:section={settingsSection}
        >
          {#snippet weatherView()}
            <WeatherView
              api={weatherApi}
              view={mapView}
              active={weatherShown?.id ?? null}
              onshow={showWeather}
              {now}
              listen={listenFn}
              embedded
            />
          {/snippet}
          {#snippet routesView()}
            {#if nav.status}
              <NavigationPanel status={nav.status} {now} ondeactivate={() => nav.deactivate()} />
            {/if}
            {@render navMessages()}
            <RoutesTracksPanel
              {nav}
              {marks}
              boat={freshBoat}
              onzoom={zoomToPoint}
              onreplay={(id, name) => void replay.load(id, name)}
            />
          {/snippet}
          {#snippet logbookView()}
            <LogbookView {logbook} embedded />
          {/snippet}
          {#snippet sources()}
            <SourcePanel api={sourcesApi} {connection} {rejections} />
            {#if connectionError}
              <p class="error" role="alert">État de la source rejeté : {connectionError}</p>
            {/if}
          {/snippet}
        </SettingsView>
      {/if}

      <!-- Alertes de la carte (ex-panneau latéral) : seulement ce qui ne
           va pas, jamais masquable. Position, liaison et moteur sont aussi
           résumés par l'état des données de l'en-tête. -->
      {#if tab === 'chart'}
        <div class="map-alerts" aria-label="Alertes de la carte">
          {#if engineError}
            <p class="error" role="alert">Moteur indisponible : {engineError}</p>
          {/if}
          {#if silent}
            <p class="error" role="alert">Moteur muet : données non actualisées</p>
          {/if}
          {#if !position}
            <p class="error" role="alert">Aucune position</p>
          {:else if position.stale}
            <p class="error" role="alert">Position obsolète ({formatAge(position.ageMs)})</p>
          {/if}
          {#if position?.fix.source === 'simulator'}
            <p class="note" role="status">POSITION SIMULÉE</p>
          {:else if position?.fix.source === 'replay'}
            <p class="note" role="status">REJEU — POSITION NON ACTUELLE</p>
          {/if}
          {#if snapshotError}
            <p class="error" role="alert">Données rejetées : {snapshotError}</p>
          {/if}
          {#if linkAlert}
            <p class="error" role="alert" data-testid="link-alert">Liaison : {linkAlert}</p>
          {/if}
          {#if connectionError}
            <p class="error" role="alert">État de la source rejeté : {connectionError}</p>
          {/if}
          <!-- Erreur des routes et traces : ici, hors édition (son panneau l'affiche). -->
          {#if nav.error && !editing}
            <p class="error" role="alert">{nav.error}</p>
          {:else if nav.notice && !editing}
            <p role="status">{nav.notice}</p>
          {/if}
          <!-- Erreur AIS : dans le panneau des cibles, ici s'il est fermé. -->
          {#if aisError && !layout.ais.visible}
            <p class="error" role="alert">Cibles AIS rejetées : {aisError}</p>
          {/if}
          {#if catalogError}
            <p class="error" role="alert">Catalogue indisponible : {catalogError}</p>
          {:else if catalog?.error}
            <p class="error" role="alert">Cartes indisponibles : {catalog.error}</p>
          {:else if catalog && catalog.charts.length === 0}
            <p class="hint" role="status" data-testid="no-charts">
              Aucune carte. Déposez des fichiers <code>.mbtiles</code> dans
              <code class="path">{catalog.chartsDir}</code> ou choisissez un autre dossier dans Paramètres
              → Cartes.
            </p>
          {/if}
          {#each catalog?.failures ?? [] as failure (failure.fileName)}
            <p class="error" role="alert">Carte ignorée {failure.fileName} : {failure.reason}</p>
          {/each}
        </div>
      {/if}
    </div>
    <!-- Navigation : barre fixe à côté de la carte (en bas par défaut,
         comme la barre du haut), en grands caractères ; affichée au
         lancement, masquable par la barre d'outils ; clic droit : position.
         La carte se réduit d'autant : rien n'est recouvert. -->
    {#if tab === 'chart' && layout.live.visible}
      <LiveNavCard
        {snapshot}
        {silent}
        {ownShip}
        docked
        placement={navDock}
        collapsed={navCollapsed}
        oncollapse={collapseNav}
        oncontextmenu={(e) => openPlacementMenu('nav', e)}
      />
    {/if}
  </div>
  {#if placementMenu?.kind === 'nav'}
    <PlacementMenu
      title="Position de la barre Navigation"
      x={placementMenu.x}
      y={placementMenu.y}
      options={NAV_DOCK_OPTIONS}
      current={navDock}
      onselect={setNavDock}
      onclose={() => (placementMenu = null)}
    />
  {:else if placementMenu?.kind === 'route'}
    <PlacementMenu
      title="Côté de la barre de route"
      x={placementMenu.x}
      y={placementMenu.y}
      options={ROUTE_SIDE_OPTIONS}
      current={routeBarSide}
      onselect={setRouteBarSide}
      onclose={() => (placementMenu = null)}
    />
  {/if}
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
    background: var(--bg);
  }

  /* Carte et barre Navigation : en colonne (barre en bas) ou en ligne
     (barre à gauche ou à droite). */
  .workspace {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-height: 0;
  }

  .workspace.nav-right {
    flex-direction: row;
  }

  .workspace.nav-left {
    flex-direction: row-reverse;
  }

  .stage {
    position: relative;
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    /* Largeur de la barre de route latérale (0 sans route active), et
       décalage de la carte du côté où elle se trouve. */
    --route-side: 0px;
    --route-left: var(--route-side);
    --route-right: 0px;
  }

  .stage.route-right {
    --route-left: 0px;
    --route-right: var(--route-side);
  }

  .stage.route-side {
    --route-side: min(12.5rem, 45vw);
  }

  .stage.route-side-collapsed {
    --route-side: 3.4rem;
  }

  .route-side-bar {
    position: absolute;
    top: 0;
    bottom: 0;
    left: 0;
    z-index: 2;
    width: var(--route-side);
    border-right: 1px solid var(--border);
    background: var(--panel-bg);
    color: var(--fg);
  }

  .route-side-bar.right {
    right: 0;
    left: auto;
    border-right: 0;
    border-left: 1px solid var(--border);
  }

  .maps {
    position: absolute;
    inset: 0;
    left: var(--route-left);
    right: var(--route-right);
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

  .mob-slot {
    position: absolute;
    top: 3.2rem;
    left: 50%;
    z-index: 6;
    transform: translateX(-50%);
  }

  .mob-error {
    top: 0.6rem;
    right: auto;
    left: 50%;
    transform: translateX(-50%);
  }

  .tool-side {
    position: absolute;
    top: 0.6rem;
    left: 0.6rem;
    z-index: 3;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    max-width: calc(100% - 1.2rem);
  }

  /* Volet droit avec son choix de cartes en haut à gauche : outils dessous. */
  .maps.split ~ .tool-side {
    top: 3rem;
  }

  .tool-note {
    margin: 0;
    padding: 0.35rem 0.6rem;
    border: 1px solid var(--border);
    border-radius: 0.35rem;
    background: var(--panel-bg);
    color: var(--fg);
    font-size: 0.85rem;
  }

  .tool-note.error {
    color: var(--error-fg);
    font-weight: 600;
  }

  .map-side {
    position: absolute;
    top: 0.75rem;
    right: calc(var(--route-right) + 3.2rem);
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

  /* Barres du bas (vent, route, outils), centrées et empilées. Pas de
     transform : un panneau déplacé est positionné dans la fenêtre. */
  .bottom-bars {
    position: absolute;
    left: var(--route-left);
    right: var(--route-right);
    bottom: 0.6rem;
    z-index: 2;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.4rem;
    padding: 0 0.75rem;
    /* Les côtés vides laissent passer les gestes sur la carte. */
    pointer-events: none;
  }

  .bottom-bars > * {
    max-width: 100%;
    pointer-events: auto;
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

  /* Alertes : en haut de la carte, à droite de la barre de route. */
  .map-alerts {
    position: absolute;
    top: 0.75rem;
    left: calc(var(--route-left) + 0.75rem);
    z-index: 6;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    max-width: min(26rem, calc(100% - 2rem));
    pointer-events: none;
  }

  .map-alerts:empty {
    display: none;
  }

  .map-alerts p {
    margin: 0;
    padding: 0.3rem 0.6rem;
    border-radius: 0.3rem;
    background: var(--panel-bg);
    box-shadow: 0 1px 4px rgb(0 0 0 / 0.25);
    font-size: 0.85rem;
    pointer-events: auto;
  }

  .map-alerts .note {
    color: var(--alert-fg);
    font-weight: 700;
    letter-spacing: 0.04em;
  }

  .panel {
    width: min(20rem, calc(100vw - 5rem));
    padding: 0.5rem 1rem 0.75rem;
  }

  /* Colonne de droite : Mouillage et Navigation gardent leur taille, le
     panneau Route se réduit et défile (aucun recouvrement). */
  .map-side > :global(.floating) {
    flex-shrink: 0;
  }

  /* Liste des cibles : défile au-delà de la hauteur disponible. */
  .ais-panel {
    max-height: 60vh;
    overflow: auto;
  }

  .route-slot {
    display: flex;
    flex-direction: column;
    /* Au moins le début du panneau reste visible ; la colonne défile. */
    min-height: 10rem;
  }

  .nav-error {
    background: var(--warn-bg);
    color: var(--error-fg);
  }

  /* Barre de route affichée en bas : colonnes raccourcies d'autant. */

  .muted {
    color: var(--muted);
    font-size: 0.75rem;
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
