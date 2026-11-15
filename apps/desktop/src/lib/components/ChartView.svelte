<script module lang="ts">
  import { setWorkerUrl } from 'maplibre-gl';
  import workerUrl from 'maplibre-gl/dist/maplibre-gl-worker.mjs?url';

  // Worker MapLibre empaqueté localement (aucun CDN, fonctionnement hors ligne).
  setWorkerUrl(workerUrl);
</script>

<script lang="ts">
  import {
    AttributionControl,
    Map as MapLibreMap,
    Marker,
    NavigationControl,
    ScaleControl,
  } from 'maplibre-gl';
  import 'maplibre-gl/dist/maplibre-gl.css';
  import { untrack } from 'svelte';
  import type { GeoJSONSource } from 'maplibre-gl';
  import type { AisSnapshot } from '../services/ais';
  import type { ChartCatalog } from '../services/charts';
  import type { PositionFix } from '../services/position';
  import type { GeoBounds, LatLon, Route } from '../services/routing';
  import { AIS_SOURCE, AIS_SYMBOL_LAYER, aisData, aisIcons, aisLayers } from '../map/ais';
  import { WIND_SOURCE, viewBounds, windData, windIcon, windLayers } from '../map/wind';
  import type { Bounds, WindVector } from '../services/weather';
  import { ANCHOR_SOURCE, anchorData, anchorLayers } from '../map/anchor';
  import type { AnchorView } from '../services/alarms';
  import type { MapCamera } from '../split';
  import type { EncFeature } from '../services/enc';
  import {
    ENC_SOURCE,
    emptyEncCollection,
    encLayers,
    encThemeChanges,
    parseSoundingImageId,
    soundingImage,
  } from '../map/enc';
  import type { FeatureCollection } from 'geojson';
  import {
    DRAFT_POINTS_LAYER,
    DEFAULT_ACTIVE_ROUTE_COLOR,
    DRAFT_SOURCE,
    ROUTES_SOURCE,
    activeRouteColorChanges,
    TRACKS_SOURCE,
    draftData,
    emptyCollection,
    overlayLayers,
    overlayThemeChanges,
    routesData,
    tracksData,
    type DraftPoint,
  } from '../map/overlays';
  import { labelImage, parseLabelImageId } from '../map/labels';
  import {
    POI_SOURCE,
    emptyPoiCollection,
    poiLayers,
    poiThemeChanges,
    type PoiFeatures,
  } from '../map/poi';
  import { POI_ATTRIBUTION } from '../services/poi';
  import { MEASURE_SOURCE, measureData, measureLayers } from '../map/measure';
  import { AIDS_SOURCE, aidsLayers, type aidsData } from '../map/aids';
  import { MARKS_SOURCE, marksLayers, marksThemeChanges, type MarkFeatures } from '../map/marks';
  import { REPLAY_SOURCE, replayLayers, type ReplayFeatures } from '../map/replay';
  import {
    BACKGROUND_COLOR,
    BACKGROUND_LAYER_ID,
    buildStyle,
    chartLayerId,
    initialView,
    maxMapZoom,
    rasterPaint,
    startZoom,
    type Theme,
  } from '../map/style';

  interface Props {
    catalog: ChartCatalog;
    fix: PositionFix | null;
    stale: boolean;
    theme: Theme;
    northUp: boolean;
    /** Routes enregistrées à dessiner. */
    routes?: Route[];
    /** Index du waypoint visé sur la route active. */
    targetIndex?: number | null;
    /** Points de la route en cours d'édition. */
    draft?: DraftPoint[];
    /** Route en cours d'édition : dessinée par le brouillon, pas en double. */
    editedRouteId?: string | null;
    /** Point de la route en édition déplacé (glisser sur la carte). */
    onpointmove?: (index: number, position: LatLon) => void;
    /** Traces à dessiner. */
    tracks?: { id: string; points: LatLon[]; recording: boolean }[];
    /** Mode édition : un clic sur la carte ajoute un point, un point se déplace en le glissant. */
    drawing?: boolean;
    onmapclick?: (position: LatLon) => void;
    /** Cibles AIS à dessiner. */
    ais?: AisSnapshot | null;
    /** Clic sur une cible AIS (hors mode tracé). */
    onaisclick?: (mmsi: string) => void;
    /** Vent prévu à dessiner (fichier GRIB). */
    wind?: WindVector[] | null;
    /** Emprise affichée, après chaque déplacement de la carte. */
    onviewchange?: (bounds: Bounds) => void;
    /** Mouillage surveillé (cercle d'évitage). */
    anchor?: AnchorView | null;
    /** L'alarme de mouillage est déclenchée. */
    anchorAlerting?: boolean;
    /** Cartes MBTiles masquées (identifiants du catalogue). */
    hiddenCharts?: string[];
    /** Vue imposée (carte partagée verrouillée) ; `null` : carte libre. */
    camera?: MapCamera | null;
    /** Vue après chaque mouvement ; `user` : mouvement fait par l'utilisateur. */
    oncamera?: (camera: MapCamera, user: boolean) => void;
    /** Entités des cartes ENC à dessiner (symbologie simplifiée). */
    enc?: FeatureCollection<EncFeature['geometry'], EncFeature['properties']>;
    /** Îles et amers (OpenStreetMap) à dessiner. */
    pois?: PoiFeatures;
    /** Couleur de la route active (`#rrggbb`). */
    activeRouteColor?: string;
    /** Points de l'outil de mesure. */
    measure?: LatLon[];
    /**
     * Clic droit (ou appui long) sans glisser : position visée et point
     * écran relatif à la carte, pour un menu contextuel.
     */
    oncontextmenu?: (position: LatLon, point: { x: number; y: number }) => void;
    /** Aides à la navigation (vecteurs, laylines, MOB, VRM/EBL). */
    aids?: ReturnType<typeof aidsData>;
    /** Homme à la mer en cours : marqueur MOB. */
    mob?: { position: LatLon; approximate: boolean } | null;
    /** Marques personnelles. */
    marks?: MarkFeatures;
    /** Trace en relecture (vitesse en couleur, curseur). */
    replay?: ReplayFeatures;
  }

  let {
    catalog,
    fix,
    stale,
    theme,
    northUp,
    routes = [],
    targetIndex = null,
    draft = [],
    editedRouteId = null,
    onpointmove,
    tracks = [],
    drawing = false,
    onmapclick,
    ais = null,
    onaisclick,
    wind = null,
    onviewchange,
    anchor = null,
    anchorAlerting = false,
    hiddenCharts = [],
    camera = null,
    oncamera,
    enc = emptyEncCollection(),
    pois = emptyPoiCollection(),
    activeRouteColor = DEFAULT_ACTIVE_ROUTE_COLOR,
    measure = [],
    oncontextmenu,
    aids = emptyCollection() as ReturnType<typeof aidsData>,
    mob = null,
    marks = { type: 'FeatureCollection', features: [] },
    replay = { type: 'FeatureCollection', features: [] },
  }: Props = $props();

  function emitView(m: MapLibreMap): void {
    const b = m.getBounds();
    onviewchange?.(viewBounds(b.getWest(), b.getSouth(), b.getEast(), b.getNorth()));
  }

  let container: HTMLDivElement;
  let map = $state.raw<MapLibreMap | null>(null);
  let loaded = $state(false);
  let zoom = $state(0);
  let marker: Marker | null = null;
  let markerElement: HTMLDivElement | null = null;
  /** La carte a déjà été centrée sur la première position reçue. */
  let centeredOnBoat = false;
  /** L'utilisateur a déplacé la carte : plus de recentrage automatique. */
  let userMoved = false;
  /** Mouvement en cours commencé par l'utilisateur (souris, doigt, clavier). */
  let userMoving = false;
  /** Vue imposée en cours d'application : pas de réémission. */
  let applying = false;
  /** Dernier geste sur cette carte (molette, pointeur, clavier, doigt). */
  let lastInput = -Infinity;
  let pointerHeld = false;
  /** Délai pendant lequel un mouvement suit encore un geste (animation de zoom). */
  const INPUT_WINDOW_MS = 600;
  /** Point de la route en édition en cours de déplacement. */
  let dragIndex: number | null = null;
  /** Le point a bougé : le clic qui termine le glisser n'ajoute rien. */
  let dragged = false;

  /** Mouvement dû à l'utilisateur : ce volet mène l'autre (cartes verrouillées). */
  function byUser(): boolean {
    return userMoving || pointerHeld || performance.now() - lastInput < INPUT_WINDOW_MS;
  }

  function cameraOf(m: MapLibreMap): MapCamera {
    const c = m.getCenter();
    return { center: [c.lng, c.lat], zoom: m.getZoom(), bearing: m.getBearing() };
  }

  // Création unique de la carte : les changements ultérieurs passent par
  // des mises à jour ciblées, sans reconstruire le style.
  $effect(() => {
    const initialCatalog = untrack(() => catalog);
    const view = initialView(initialCatalog);
    const m = new MapLibreMap({
      container,
      style: buildStyle(
        initialCatalog,
        untrack(() => theme),
      ),
      ...(view.kind === 'bounds'
        ? { bounds: view.bounds, fitBoundsOptions: { padding: 24 } }
        : { center: view.center, zoom: view.zoom }),
      maxZoom: maxMapZoom(initialCatalog),
      maxPitch: 0,
      attributionControl: false,
    });
    m.addControl(new NavigationControl({ visualizePitch: false }), 'top-right');
    m.addControl(new ScaleControl({ unit: 'nautical' }), 'bottom-left');
    m.addControl(new AttributionControl({ compact: true }), 'bottom-right');
    m.touchPitch.disable();
    // Gestes de l'utilisateur, repérés sur le conteneur : certains
    // mouvements MapLibre (molette) ne portent pas l'événement d'origine.
    const markInput = (): void => {
      lastInput = performance.now();
    };
    const press = (): void => {
      pointerHeld = true;
      markInput();
    };
    const release = (): void => {
      if (!pointerHeld) return;
      pointerHeld = false;
      markInput();
    };
    const capture = { capture: true, passive: true };
    container.addEventListener('wheel', markInput, capture);
    container.addEventListener('keydown', markInput, capture);
    container.addEventListener('pointerdown', press, capture);
    container.addEventListener('touchstart', press, capture);
    window.addEventListener('pointerup', release);
    window.addEventListener('pointercancel', release);
    window.addEventListener('touchend', release);
    // Une emprise de carte étendue donnerait un zoom trop faible.
    m.jumpTo({ zoom: startZoom(m.getZoom()) });
    m.on('movestart', (event) => {
      userMoving = !applying && event.originalEvent !== undefined;
      if (userMoving) userMoved = true;
    });
    m.on('move', () => {
      if (!applying) oncamera?.(cameraOf(m), byUser());
    });
    // Sondes ENC et étiquettes (noms des waypoints) : images créées à la
    // demande, pas de polices hors ligne. Depuis MapLibre 6, seul un
    // résolveur peut fournir l'image à la requête en cours.
    m.setMissingStyleImageResolver((id) => {
      if (m.hasImage(id)) return;
      const ratio = window.devicePixelRatio || 1;
      const sounding = parseSoundingImageId(id);
      const label = sounding ? null : parseLabelImageId(id);
      const image = sounding
        ? soundingImage(sounding.label, sounding.shallow, sounding.theme, ratio)
        : label
          ? labelImage(label.text, label.style, label.theme, ratio)
          : null;
      if (image) m.addImage(id, image, { pixelRatio: ratio });
    });
    m.on('load', () => {
      // Cartes ENC au-dessus des cartes raster, sous toutes les surimpressions.
      m.addSource(ENC_SOURCE, { type: 'geojson', data: emptyEncCollection() });
      for (const layer of encLayers(untrack(() => theme))) {
        m.addLayer(layer);
      }
      // Îles et amers (OSM) au-dessus des cartes, sous les surimpressions.
      m.addSource(POI_SOURCE, {
        type: 'geojson',
        data: emptyPoiCollection(),
        attribution: POI_ATTRIBUTION,
      });
      for (const layer of poiLayers(untrack(() => theme))) {
        m.addLayer(layer);
      }
      for (const source of [ROUTES_SOURCE, DRAFT_SOURCE, TRACKS_SOURCE]) {
        m.addSource(source, { type: 'geojson', data: emptyCollection() });
      }
      const overlays = overlayLayers(
        untrack(() => theme),
        untrack(() => activeRouteColor),
      );
      for (const layer of overlays) {
        m.addLayer(layer);
      }
      // Vent sous les routes, traces et cibles.
      const icon = windIcon();
      m.addImage(icon.id, icon.image, { sdf: true });
      m.addSource(WIND_SOURCE, { type: 'geojson', data: emptyCollection() });
      for (const layer of windLayers()) {
        m.addLayer(layer, overlays[0]?.id);
      }
      // Cercle d'évitage au-dessus du vent, sous les routes.
      m.addSource(ANCHOR_SOURCE, { type: 'geojson', data: emptyCollection() });
      for (const layer of anchorLayers()) {
        m.addLayer(layer, overlays[0]?.id);
      }
      // Trace relue et marques au-dessus des routes et traces.
      m.addSource(REPLAY_SOURCE, { type: 'geojson', data: emptyCollection() });
      for (const layer of replayLayers()) {
        m.addLayer(layer);
      }
      m.addSource(MARKS_SOURCE, { type: 'geojson', data: emptyCollection() });
      for (const layer of marksLayers(untrack(() => theme))) {
        m.addLayer(layer);
      }
      // Aides (vecteurs, laylines, MOB, VRM/EBL) sous les cibles AIS.
      m.addSource(AIDS_SOURCE, { type: 'geojson', data: emptyCollection() });
      for (const layer of aidsLayers()) {
        m.addLayer(layer);
      }
      for (const { id, image } of aisIcons()) {
        m.addImage(id, image, { sdf: true });
      }
      m.addSource(AIS_SOURCE, { type: 'geojson', data: emptyCollection() });
      for (const layer of aisLayers()) {
        m.addLayer(layer);
      }
      // Mesure au-dessus de tout : c'est l'outil en cours d'utilisation.
      m.addSource(MEASURE_SOURCE, { type: 'geojson', data: measureData([]) });
      for (const layer of measureLayers()) {
        m.addLayer(layer);
      }
      loaded = true;
      emitView(m);
      oncamera?.(cameraOf(m), false);
    });
    m.on('moveend', () => {
      userMoving = false;
      emitView(m);
    });
    m.on('click', AIS_SYMBOL_LAYER, (event) => {
      const mmsi = event.features?.[0]?.properties.mmsi;
      if (!drawing && typeof mmsi === 'string') onaisclick?.(mmsi);
    });
    m.on('mouseenter', AIS_SYMBOL_LAYER, () => {
      if (!drawing) m.getCanvas().style.cursor = 'pointer';
    });
    m.on('mouseleave', AIS_SYMBOL_LAYER, () => {
      if (!drawing) m.getCanvas().style.cursor = '';
    });
    // Édition : un point de la route se déplace en le glissant (souris ou doigt).
    const startDrag = (event: {
      features?: { properties: Record<string, unknown> }[];
      preventDefault: () => void;
    }): void => {
      const index = event.features?.[0]?.properties.index;
      if (!drawing || typeof index !== 'number') return;
      event.preventDefault();
      dragIndex = index;
      dragged = false;
      m.getCanvas().style.cursor = 'grabbing';
    };
    const moveDrag = (event: { lngLat: { wrap: () => { lat: number; lng: number } } }): void => {
      if (dragIndex === null) return;
      dragged = true;
      const { lat, lng } = event.lngLat.wrap();
      onpointmove?.(dragIndex, { latitudeDeg: lat, longitudeDeg: lng });
    };
    const endDrag = (): void => {
      if (dragIndex === null) return;
      dragIndex = null;
      m.getCanvas().style.cursor = drawing ? 'crosshair' : '';
      // Le clic éventuel qui suit le relâchement arrive avant ce délai ;
      // après un vrai glisser, MapLibre n'en émet pas : le clic suivant compte.
      setTimeout(() => {
        dragged = false;
      }, 0);
    };
    m.on('mousedown', DRAFT_POINTS_LAYER, startDrag);
    m.on('touchstart', DRAFT_POINTS_LAYER, (event) => {
      if (event.points.length === 1) startDrag(event);
    });
    m.on('mousemove', moveDrag);
    m.on('touchmove', moveDrag);
    m.on('mouseup', endDrag);
    m.on('touchend', endDrag);
    m.on('mouseenter', DRAFT_POINTS_LAYER, () => {
      if (drawing && dragIndex === null) m.getCanvas().style.cursor = 'move';
    });
    m.on('mouseleave', DRAFT_POINTS_LAYER, () => {
      if (drawing && dragIndex === null) m.getCanvas().style.cursor = 'crosshair';
    });
    m.on('click', (event) => {
      if (!drawing) return;
      // Fin d'un glisser, ou clic sur un point existant : pas de nouveau point.
      if (dragged) {
        dragged = false;
        return;
      }
      if (m.queryRenderedFeatures(event.point, { layers: [DRAFT_POINTS_LAYER] }).length > 0) return;
      const { lat, lng } = event.lngLat.wrap();
      onmapclick?.({ latitudeDeg: lat, longitudeDeg: lng });
    });
    // Menu contextuel : clic droit sans glisser (le glisser droit tourne la
    // carte) ou appui long.
    let rightDown: { x: number; y: number } | null = null;
    m.on('mousedown', (event) => {
      if (event.originalEvent.button === 2) rightDown = event.point;
    });
    m.on('contextmenu', (event) => {
      event.preventDefault();
      event.originalEvent.preventDefault();
      const start = rightDown;
      rightDown = null;
      if (start && Math.hypot(event.point.x - start.x, event.point.y - start.y) > 5) return;
      const { lat, lng } = event.lngLat.wrap();
      oncontextmenu?.(
        { latitudeDeg: lat, longitudeDeg: lng },
        { x: event.point.x, y: event.point.y },
      );
    });
    zoom = m.getZoom();
    m.on('zoom', () => {
      zoom = m.getZoom();
    });
    map = m;
    return () => {
      container.removeEventListener('wheel', markInput, capture);
      container.removeEventListener('keydown', markInput, capture);
      container.removeEventListener('pointerdown', press, capture);
      container.removeEventListener('touchstart', press, capture);
      window.removeEventListener('pointerup', release);
      window.removeEventListener('pointercancel', release);
      window.removeEventListener('touchend', release);
      marker?.remove();
      marker = null;
      mobMarker?.remove();
      mobMarker = null;
      m.remove();
      map = null;
      loaded = false;
    };
  });

  // Vue imposée par l'autre volet (cartes verrouillées).
  $effect(() => {
    const m = map;
    const c = camera;
    if (!m || !c) return;
    applying = true;
    try {
      m.jumpTo({ center: c.center, zoom: c.zoom, bearing: c.bearing });
    } finally {
      applying = false;
    }
  });

  // Thème jour / nuit.
  $effect(() => {
    const m = map;
    const t = theme;
    if (!m || !loaded) return;
    m.setPaintProperty(BACKGROUND_LAYER_ID, 'background-color', BACKGROUND_COLOR[t]);
    const paint = rasterPaint(t);
    for (const chart of untrack(() => catalog.charts)) {
      for (const property of Object.keys(paint) as (keyof typeof paint)[]) {
        m.setPaintProperty(chartLayerId(chart), property, paint[property]);
      }
    }
    for (const [layer, kind, property, value] of encThemeChanges(t)) {
      // Propriétés issues de `encLayers`, donc valides pour ces couches.
      if (kind === 'paint') m.setPaintProperty(layer, property as never, value as never);
      else m.setLayoutProperty(layer, property as never, value as never);
    }
    for (const [layer, image] of [
      ...overlayThemeChanges(t),
      ...poiThemeChanges(t),
      ...marksThemeChanges(t),
    ]) {
      m.setLayoutProperty(layer, 'icon-image', image);
    }
  });

  $effect(() => {
    const data = enc;
    if (map && loaded) map.getSource<GeoJSONSource>(ENC_SOURCE)?.setData(data);
  });

  // Couleur de la route active, changée dans les Paramètres.
  $effect(() => {
    const changes = activeRouteColorChanges(activeRouteColor);
    if (!map || !loaded) return;
    for (const [layer, property, value] of changes) map.setPaintProperty(layer, property, value);
  });

  $effect(() => {
    const data = pois;
    if (map && loaded) map.getSource<GeoJSONSource>(POI_SOURCE)?.setData(data);
  });

  // Cartes masquées : visibilité des couches raster, sans reconstruire
  // le style.
  $effect(() => {
    const m = map;
    const hidden = hiddenCharts;
    if (!m || !loaded) return;
    for (const chart of untrack(() => catalog.charts)) {
      m.setLayoutProperty(
        chartLayerId(chart),
        'visibility',
        hidden.includes(chart.id) ? 'none' : 'visible',
      );
    }
  });

  // Surimpressions : mises à jour ciblées des sources GeoJSON.
  function setData(id: string, data: ReturnType<typeof emptyCollection>): void {
    const source = map?.getSource<GeoJSONSource>(id);
    source?.setData(data);
  }

  $effect(() => {
    const data = routesData(routes, targetIndex, editedRouteId);
    if (map && loaded) setData(ROUTES_SOURCE, data);
  });

  $effect(() => {
    const data = draftData(draft);
    if (map && loaded) setData(DRAFT_SOURCE, data);
  });

  $effect(() => {
    const data = tracksData(tracks);
    if (map && loaded) setData(TRACKS_SOURCE, data);
  });

  $effect(() => {
    const data = aisData(ais);
    if (map && loaded) setData(AIS_SOURCE, data);
  });

  $effect(() => {
    const data = anchorData(anchor, anchorAlerting);
    if (map && loaded) map.getSource<GeoJSONSource>(ANCHOR_SOURCE)?.setData(data);
  });

  $effect(() => {
    const data = marks;
    if (map && loaded) map.getSource<GeoJSONSource>(MARKS_SOURCE)?.setData(data);
  });

  $effect(() => {
    const data = replay;
    if (map && loaded) map.getSource<GeoJSONSource>(REPLAY_SOURCE)?.setData(data);
  });

  $effect(() => {
    const data = aids;
    if (map && loaded) map.getSource<GeoJSONSource>(AIDS_SOURCE)?.setData(data);
  });

  // Homme à la mer : marqueur bien visible, au-dessus de tout.
  let mobMarker: Marker | null = null;
  $effect(() => {
    const m = map;
    const current = mob;
    if (!m || !current) {
      mobMarker?.remove();
      mobMarker = null;
      return;
    }
    const lngLat: [number, number] = [current.position.longitudeDeg, current.position.latitudeDeg];
    if (!mobMarker) {
      const element = document.createElement('div');
      element.className = 'mob-marker';
      element.innerHTML = '<span class="mob-ring">MOB</span>';
      mobMarker = new Marker({ element }).setLngLat(lngLat).addTo(m);
    } else {
      mobMarker.setLngLat(lngLat);
    }
    const element = mobMarker.getElement();
    element.classList.toggle('approximate', current.approximate);
    element.title = current.approximate
      ? 'Homme à la mer — point approximatif (position obsolète à l’appui)'
      : 'Homme à la mer';
  });

  $effect(() => {
    const data = measureData(measure);
    if (map && loaded) map.getSource<GeoJSONSource>(MEASURE_SOURCE)?.setData(data);
  });

  $effect(() => {
    const data = windData(wind);
    if (map && loaded) map.getSource<GeoJSONSource>(WIND_SOURCE)?.setData(data);
  });

  $effect(() => {
    const m = map;
    if (!m) return;
    m.getCanvas().style.cursor = drawing ? 'crosshair' : '';
    // Pas de zoom au double-clic pendant le tracé (double point).
    if (drawing) m.doubleClickZoom.disable();
    else m.doubleClickZoom.enable();
  });

  // Mode Nord en haut : cap carte bloqué à 0.
  $effect(() => {
    const m = map;
    if (!m) return;
    if (northUp) {
      m.easeTo({ bearing: 0 });
      m.dragRotate.disable();
      m.touchZoomRotate.disableRotation();
      m.keyboard.disableRotation();
    } else {
      m.dragRotate.enable();
      m.touchZoomRotate.enableRotation();
      m.keyboard.enableRotation();
    }
  });

  // Position du bateau.
  $effect(() => {
    const m = map;
    const current = fix;
    if (!m || !current) return;
    const lngLat: [number, number] = [current.longitudeDeg, current.latitudeDeg];
    if (!centeredOnBoat) {
      centeredOnBoat = true;
      if (!userMoved) m.jumpTo({ center: lngLat });
    }
    if (!marker || !markerElement) {
      markerElement = document.createElement('div');
      markerElement.className = 'boat-marker';
      markerElement.innerHTML = '<span class="boat-dot"></span><span class="boat-label"></span>';
      marker = new Marker({ element: markerElement }).setLngLat(lngLat).addTo(m);
    } else {
      marker.setLngLat(lngLat);
    }
    const simulated = current.source === 'simulator';
    markerElement.classList.toggle('simulated', simulated);
    markerElement.classList.toggle('stale', stale);
    const label = markerElement.querySelector('.boat-label');
    if (label) {
      label.textContent = stale ? 'OBSOLÈTE' : simulated ? 'SIMULATION' : '';
    }
    markerElement.title = simulated ? 'Position simulée' : 'Position du bateau';
  });

  /**
   * Cadre la carte sur une emprise (route, trace), en laissant de la place
   * aux panneaux. Une emprise qui traverse l'antiméridien est déroulée.
   */
  export function fitBounds(bounds: GeoBounds): void {
    const m = map;
    if (!m) return;
    const east = bounds.west > bounds.east ? bounds.east + 360 : bounds.east;
    const width = container.clientWidth;
    const height = container.clientHeight;
    const roomy = width > 700 && height > 400;
    const padding = roomy
      ? { top: 60, bottom: 120, left: 60, right: Math.min(380, Math.round(width * 0.4)) }
      : 30;
    m.fitBounds(
      [
        [bounds.west, bounds.south],
        [east, bounds.north],
      ],
      { padding, maxZoom: 15, duration: 600 },
    );
  }

  /** Centre la carte sur la dernière position connue. */
  export function recenter(): void {
    if (map && fix) {
      map.easeTo({ center: [fix.longitudeDeg, fix.latitudeDeg] });
    }
  }
</script>

<div class="chart-view" bind:this={container}></div>
<div class="zoom-level" data-testid="zoom-level" aria-label="Niveau de zoom">
  Zoom {zoom.toFixed(1)}
</div>

<style>
  .chart-view {
    position: absolute;
    inset: 0;
  }

  .zoom-level {
    position: absolute;
    left: 10px;
    bottom: 2.4rem;
    z-index: 1;
    padding: 0 0.4rem;
    border-radius: 0.25rem;
    background: var(--panel-bg);
    color: var(--fg);
    font:
      600 0.75rem/1.6 ui-monospace,
      'Cascadia Mono',
      Consolas,
      monospace;
    pointer-events: none;
  }

  :global(.boat-marker) {
    display: flex;
    flex-direction: column;
    align-items: center;
    pointer-events: auto;
  }

  :global(.boat-dot) {
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: #d81b60;
    border: 3px solid #fff;
    box-shadow: 0 0 0 2px #d81b60;
  }

  :global(.boat-marker.simulated .boat-dot) {
    background: #ff9800;
    box-shadow: 0 0 0 2px #ff9800;
    border-style: dashed;
  }

  :global(.boat-marker.stale .boat-dot) {
    background: #9e9e9e;
    box-shadow: 0 0 0 2px #616161;
  }

  :global(.boat-label) {
    margin-top: 4px;
    padding: 0 4px;
    font:
      600 10px/1.4 system-ui,
      sans-serif;
    letter-spacing: 0.05em;
    color: #fff;
    background: rgba(0, 0, 0, 0.65);
    border-radius: 3px;
  }

  :global(.boat-label:empty) {
    display: none;
  }

  :global(.mob-marker) {
    pointer-events: auto;
  }

  :global(.mob-ring) {
    display: grid;
    place-items: center;
    width: 38px;
    height: 38px;
    border: 3px solid #fff;
    border-radius: 50%;
    background: #e53935;
    box-shadow: 0 0 0 3px #e53935;
    color: #fff;
    font:
      800 11px/1 system-ui,
      sans-serif;
    animation: mob-pulse 1s ease-in-out infinite alternate;
  }

  :global(.mob-marker.approximate .mob-ring) {
    border-style: dashed;
  }

  @keyframes -global-mob-pulse {
    to {
      box-shadow: 0 0 0 9px rgba(229, 57, 53, 0.35);
    }
  }
</style>
