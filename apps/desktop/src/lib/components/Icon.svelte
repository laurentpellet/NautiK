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
