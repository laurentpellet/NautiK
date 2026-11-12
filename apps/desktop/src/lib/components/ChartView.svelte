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
  import type { ChartCatalog } from '../services/charts';
  import type { PositionFix } from '../services/position';
  import type { LatLon, Route } from '../services/routing';
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
  }: Props = $props();

  let container: HTMLDivElement;
  let map = $state.raw<MapLibreMap | null>(null);
  let loaded = $state(false);
  let zoom = $state(0);
  let marker: Marker | null = null;
  let markerElement: HTMLDivElement | null = null;

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
    m.on('load', () => {
      for (const source of [ROUTES_SOURCE, DRAFT_SOURCE, TRACKS_SOURCE]) {
        m.addSource(source, { type: 'geojson', data: emptyCollection() });
      }
      for (const layer of overlayLayers()) {
        m.addLayer(layer);
      }
      loaded = true;
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
      marker?.remove();
      marker = null;
      m.remove();
      map = null;
      loaded = false;
    };
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
