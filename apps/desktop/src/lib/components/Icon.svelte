<script lang="ts" module>
  /** Tracés des icônes (repère 24 × 24, trait de la couleur du texte). */
  const PATHS = {
    north: ['M12 21V4', 'M6 10l6-6 6 6'],
    rotate: ['M20 12a8 8 0 1 1-2.6-5.9', 'M20 4v5h-5'],
    center: ['M12 2v4', 'M12 18v4', 'M2 12h4', 'M18 12h4', 'M12 5a7 7 0 1 0 0 14a7 7 0 1 0 0-14'],
    panel: ['M4 4h16a1 1 0 0 1 1 1v14a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1z', 'M9 4v16'],
    layers: ['M12 3 L22 8 L12 13 L2 8 Z', 'M2 12.5 L12 17.5 L22 12.5', 'M2 17 L12 22 L22 17'],
    anchor: [
      'M12 3a2 2 0 1 0 0 4a2 2 0 1 0 0-4',
      'M12 7v14',
      'M8 10h8',
      'M4 13a8 8 0 0 0 16 0',
      'M2 15l2-2 2 2',
      'M18 15l2-2 2 2',
    ],
    gauge: ['M3.5 18a9.5 9.5 0 1 1 17 0', 'M12 14l4.5-5', 'M12 13a1 1 0 1 0 0 2a1 1 0 1 0 0-2'],
    reset: ['M4 12a8 8 0 1 0 2.6-5.9', 'M4 4v5h5'],
    up: ['M6 15l6-6 6 6'],
    split: ['M4 4h16a1 1 0 0 1 1 1v14a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1z', 'M12 4v16'],
    lock: ['M6 11h12v9H6z', 'M8.5 11V8a3.5 3.5 0 0 1 7 0v3'],
    unlock: ['M6 11h12v9H6z', 'M8.5 11V8a3.5 3.5 0 0 1 6.8-1.2'],
    route: [
      'M5 19L10 9L15 14L19 5',
      'M5 17a2 2 0 1 0 0 4a2 2 0 1 0 0-4',
      'M19 3a2 2 0 1 0 0 4a2 2 0 1 0 0-4',
    ],
    routebar: ['M3 8h18v8H3z', 'M6 12h3', 'M11 12h2', 'M15 12h3'],
    stop: ['M7 7h10v10H7z'],
    play: ['M8 5l11 7-11 7z'],
    status: ['M3 12h4l2.5-6 5 12 2.5-6h4'],
    boat: ['M3 15h18l-3 5H6z', 'M12 3v12', 'M12 4l6 9h-6'],
    ruler: ['M3 17L17 3l4 4L7 21z', 'M7 13l2 2', 'M10 10l2 2', 'M13 7l2 2'],
    vrm: [
      'M12 4a8 8 0 1 0 0 16a8 8 0 1 0 0-16',
      'M12 12L20 4',
      'M12 11a1 1 0 1 0 0 2a1 1 0 1 0 0-2',
    ],
    mark: ['M6 21V4', 'M6 4h11l-2 4 2 4H6'],
    search: ['M10 4a6 6 0 1 0 0 12a6 6 0 1 0 0-12', 'M15 15l5 5'],
    eye: ['M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12z', 'M12 9a3 3 0 1 0 0 6a3 3 0 1 0 0-6'],
    eyeOff: [
      'M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12z',
      'M12 9a3 3 0 1 0 0 6a3 3 0 1 0 0-6',
      'M4 4l16 16',
    ],
    log: [
      'M5 4h11a2 2 0 0 1 2 2v14H7a2 2 0 0 1-2-2z',
      'M5 18a2 2 0 0 1 2-2h11',
      'M9 8h6',
      'M9 11h4',
    ],
    sun: [
      'M12 8a4 4 0 1 0 0 8a4 4 0 1 0 0-8',
      'M12 2v2',
      'M12 20v2',
      'M2 12h2',
      'M20 12h2',
      'M4.9 4.9l1.4 1.4',
      'M17.7 17.7l1.4 1.4',
      'M4.9 19.1l1.4-1.4',
      'M17.7 6.3l1.4-1.4',
    ],
    moon: ['M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z'],
    expand: ['M4 9V4h5', 'M20 9V4h-5', 'M4 15v5h5', 'M20 15v5h-5'],
    shrink: ['M9 4v5H4', 'M15 4v5h5', 'M9 20v-5H4', 'M15 20v-5h5'],
    map: ['M3 6l6-3 6 3 6-3v15l-6 3-6-3-6 3z', 'M9 3v15', 'M15 6v15'],
    settings: [
      'M4 6h9',
      'M17 6h3',
      'M15 4v4',
      'M4 12h3',
      'M11 12h9',
      'M9 10v4',
      'M4 18h11',
      'M19 18h1',
      'M17 16v4',
    ],
    antenna: ['M5 11a7 7 0 0 1 14 0', 'M8.5 11a3.5 3.5 0 0 1 7 0', 'M12 11v10', 'M9 21h6'],
    ais: ['M12 3l6 17-6-4-6 4z'],
    bell: ['M6 16v-5a6 6 0 0 1 12 0v5l2 2H4z', 'M10 20a2 2 0 0 0 4 0'],
    chip: [
      'M7 7h10v10H7z',
      'M10 3v4',
      'M14 3v4',
      'M10 17v4',
      'M14 17v4',
      'M3 10h4',
      'M3 14h4',
      'M17 10h4',
      'M17 14h4',
    ],
    weather: [
      'M7 17h10a3.5 3.5 0 0 0 0-7 5 5 0 0 0-9.6 1.2A2.9 2.9 0 0 0 7 17z',
      'M4 21h9',
      'M15 21h5',
    ],
  } as const;

  export type IconName = keyof typeof PATHS;
</script>

<script lang="ts">
  interface Props {
    name: IconName;
  }

  let { name }: Props = $props();
</script>

<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
  {#each PATHS[name] as d (d)}
    <path {d} />
  {/each}
</svg>

<style>
  svg {
    width: 1.15rem;
    height: 1.15rem;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
    flex-shrink: 0;
  }
</style>
