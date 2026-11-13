<script lang="ts">
  import {
    DIAL_CENTER,
    DIAL_RADIUS,
    DIAL_SECTORS,
    arcPath,
    dialTicks,
    windArrow,
  } from '../dashboard';

  interface Props {
    /** Angle du vent apparent (rad, positif à tribord) ; `null` si inconnu. */
    awaRad: number | null;
    /** Donnée obsolète : flèche estompée. */
    stale: boolean;
  }

  let { awaRad, stale }: Props = $props();

  const ticks = dialTicks();
  const c = DIAL_CENTER;
</script>

<svg
  class="dial"
  viewBox="0 0 200 200"
  role="img"
  aria-label={awaRad === null
    ? 'Vent apparent inconnu'
    : `Vent apparent ${Math.round((Math.abs(awaRad) * 180) / Math.PI)}° ${awaRad >= 0 ? 'tribord' : 'bâbord'}`}
  data-testid="wind-dial"
>
  <circle cx={c} cy={c} r={DIAL_RADIUS} class="rim" />
  <circle cx={c} cy={c} r={DIAL_RADIUS - 24} class="face" />
  {#each DIAL_SECTORS as s (s.fromDeg)}
    <path d={arcPath(s.fromDeg, s.toDeg, DIAL_RADIUS - 4)} class="sector {s.side}" />
  {/each}
  {#each ticks as t (t.angleDeg)}
    <line
      x1={t.inner.x}
      y1={t.inner.y}
      x2={t.outer.x}
      y2={t.outer.y}
      class="tick"
      class:major={t.major}
    />
    {#if t.label}
      <text x={t.labelAt.x} y={t.labelAt.y} class="label">{t.label}</text>
    {/if}
  {/each}
  <!-- Bateau vu de dessus, étrave en haut. -->
  <path d="M 100 74 C 109 84 111 104 108 124 L 92 124 C 89 104 91 84 100 74 Z" class="boat" />
  {#if awaRad !== null}
    <path d={windArrow(awaRad)} class="arrow" class:stale data-testid="wind-arrow" />
    <text
      x={c}
      y={c + 4}
      class="awa-letter"
      transform="rotate({(awaRad * 180) / Math.PI} {c} {c}) translate(0 {-(DIAL_RADIUS - 34)})"
      >A</text
    >
  {/if}
</svg>

<style>
  .dial {
    width: 100%;
    max-width: 22rem;
    aspect-ratio: 1;
  }

  .rim {
    fill: var(--inset-bg);
    stroke: var(--border);
    stroke-width: 1;
  }

  .face {
    fill: var(--card-bg);
    stroke: var(--border);
    stroke-width: 10;
  }

  .sector {
    fill: none;
    stroke-width: 6;
    stroke-linecap: round;
  }

  .port {
    stroke: var(--port);
  }

  .starboard {
    stroke: var(--starboard);
  }

  .tick {
    stroke: var(--muted);
    stroke-width: 0.8;
  }

  .tick.major {
    stroke-width: 1.6;
  }

  .label {
    fill: var(--fg);
    font:
      600 8px system-ui,
      sans-serif;
    text-anchor: middle;
    dominant-baseline: middle;
  }

  .boat {
    fill: none;
    stroke: var(--muted);
    stroke-width: 1.5;
  }

  .arrow {
    fill: var(--tone-wind);
    stroke: var(--fg);
    stroke-width: 0.8;
  }

  .arrow.stale {
    opacity: 0.35;
  }

  .awa-letter {
    fill: var(--inset-bg);
    font:
      700 7px system-ui,
      sans-serif;
    text-anchor: middle;
  }
</style>
