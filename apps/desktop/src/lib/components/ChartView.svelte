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
  import type { LatLon, Route } from '../services/routing';
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
    DRAFT_SOURCE,
    ROUTES_SOURCE,
    TRACKS_SOURCE,
    draftData,
    emptyCollection,
    overlayLayers,
    routesData,
    tracksData,
  } from '../map/overlays';
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
    /** Points de la route en cours de tracé. */
    draft?: LatLon[];
    /** Traces à dessiner. */
    tracks?: { id: string; points: LatLon[]; recording: boolean }[];
    /** Mode tracé : un clic sur la carte ajoute un point. */
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
    // Sondes ENC : images créées à la demande (pas de polices hors ligne).
    m.on('styleimagemissing', (event: { id: string }) => {
      const sounding = parseSoundingImageId(event.id);
      if (!sounding || m.hasImage(event.id)) return;
      const ratio = window.devicePixelRatio || 1;
      const image = soundingImage(sounding.label, sounding.shallow, sounding.theme, ratio);
      if (image) m.addImage(event.id, image, { pixelRatio: ratio });
    });
    m.on('load', () => {
      // Cartes ENC au-dessus des cartes raster, sous toutes les surimpressions.
      m.addSource(ENC_SOURCE, { type: 'geojson', data: emptyEncCollection() });
      for (const layer of encLayers(untrack(() => theme))) {
        m.addLayer(layer);
      }
      for (const source of [ROUTES_SOURCE, DRAFT_SOURCE, TRACKS_SOURCE]) {
        m.addSource(source, { type: 'geojson', data: emptyCollection() });
      }
      const overlays = overlayLayers();
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
      for (const { id, image } of aisIcons()) {
        m.addImage(id, image, { sdf: true });
      }
      m.addSource(AIS_SOURCE, { type: 'geojson', data: emptyCollection() });
      for (const layer of aisLayers()) {
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
    m.on('click', (event) => {
      if (drawing) {
        const { lat, lng } = event.lngLat.wrap();
        onmapclick?.({ latitudeDeg: lat, longitudeDeg: lng });
      }
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
  });

  $effect(() => {
    const data = enc;
    if (map && loaded) map.getSource<GeoJSONSource>(ENC_SOURCE)?.setData(data);
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
    const data = routesData(routes, targetIndex);
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
</style>
