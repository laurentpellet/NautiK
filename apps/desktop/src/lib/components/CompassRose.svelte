<script lang="ts">
  // Compas nord en haut : repère sur le cap (ou la route) reçu, valeur
  // au centre. Sans donnée, aucun repère n'est dessiné.
  import { DIAL_CENTER, DIAL_RADIUS, compassTicks, headingMarker } from '../dashboard';

  interface Props {
    /** Angle vrai (rad) ; `null` si inconnu. */
    angleRad: number | null;
    /** Nature de l'angle affiché (Cap, COG…). */
    label: string;
    stale: boolean;
  }

  let { angleRad, label, stale }: Props = $props();

  const ticks = compassTicks();
  const c = DIAL_CENTER;
  const degrees = $derived(
    angleRad === null ? null : Math.round((((angleRad * 180) / Math.PI) % 360) + 360) % 360,
  );
</script>

<svg
  class="compass"
  viewBox="-4 -4 208 208"
  role="img"
  aria-label={degrees === null ? `${label} inconnu` : `${label} ${degrees}°`}
  data-testid="compass"
>
  <circle cx={c} cy={c} r={DIAL_RADIUS} class="rim" />
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
      <text x={t.labelAt.x} y={t.labelAt.y} class="cardinal">{t.label}</text>
    {/if}
  {/each}
  {#if angleRad !== null}
    <path d={headingMarker(angleRad)} class="marker" class:stale />
  {/if}
  <text x={c} y={c - 2} class="value" class:stale>{degrees === null ? '—' : `${degrees}°`}</text>
  <text x={c} y={c + 22} class="kind">{label}</text>
</svg>

<style>
  .compass {
    width: 100%;
    max-width: 12rem;
    aspect-ratio: 1;
  }

  .rim {
    fill: none;
    stroke: var(--border);
    stroke-width: 1.5;
  }

  .tick {
    stroke: var(--muted);
    stroke-width: 1;
  }

  .tick.major {
    stroke: var(--fg);
    stroke-width: 2;
  }

  .cardinal {
    fill: var(--fg);
    font:
      700 14px system-ui,
      sans-serif;
    text-anchor: middle;
    dominant-baseline: middle;
  }

  .marker {
    fill: var(--accent-bg);
    stroke: var(--fg);
    stroke-width: 1;
  }

  .value {
    fill: var(--fg);
    font:
      800 34px system-ui,
      sans-serif;
    text-anchor: middle;
    dominant-baseline: middle;
  }

  .kind {
    fill: var(--muted);
    font:
      600 12px system-ui,
      sans-serif;
    text-anchor: middle;
  }

  .stale {
    opacity: 0.4;
  }
</style>
