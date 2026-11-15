<script lang="ts">
  // Outil VRM/EBL : cercle de distance et ligne de relèvement autour du
  // bateau. Un clic sur la carte les fait passer par le point visé
  // (distance et relèvement calculés par le moteur) ; ils se règlent aussi
  // à la main. Sans position fraîche, rien n'est tracé.
  import { formatBearing, formatDistance } from '../format';
  import type { MapToolsController } from '../mapTools.svelte';
  import { DISTANCE_UNITS, distanceIn, units } from '../units.svelte';

  interface Props {
    tools: MapToolsController;
    /** Position du bateau fraîche (centre du repère). */
    boatKnown: boolean;
  }

  let { tools, boatKnown }: Props = $props();

  const mark = $derived(tools.rangeBearing);
  const unit = $derived(DISTANCE_UNITS[units.distance]);

  let radiusText = $state('');
  let bearingText = $state('');

  // Champs recopiés depuis le repère (clic sur la carte).
  $effect(() => {
    const m = mark;
    radiusText = m.radiusM === null ? '' : distanceIn(m.radiusM).toFixed(2);
    bearingText = m.bearingRad === null ? '' : ((m.bearingRad * 180) / Math.PI).toFixed(0);
  });

  function parse(text: string): number | null | undefined {
    const t = text.trim().replace(',', '.');
    if (t === '') return null;
    const n = Number(t);
    return Number.isFinite(n) ? n : undefined;
  }

  function apply(event: SubmitEvent): void {
    event.preventDefault();
    const radius = parse(radiusText);
    const bearing = parse(bearingText);
    if (radius === undefined || bearing === undefined) {
      tools.error = 'Valeur non numérique.';
      return;
    }
    tools.setRangeBearing({
      radiusM: radius === null ? null : radius * unit.meters,
      bearingRad: bearing === null ? null : (bearing * Math.PI) / 180,
    });
  }
</script>

<section class="rb" aria-label="Distance et relèvement (VRM/EBL)" data-testid="range-bearing-panel">
  <header>
    <h3>VRM / EBL</h3>
    <button
      type="button"
      class="close"
      aria-label="Fermer VRM/EBL"
      title="Fermer (Échap)"
      onclick={() => tools.close()}>✕</button
    >
  </header>
  {#if !boatKnown}
    <p class="error" role="status">Position du bateau inconnue ou obsolète : rien n'est tracé.</p>
  {:else}
    <p class="hint">Cliquez sur la carte pour faire passer le cercle et la ligne par un point.</p>
  {/if}
  <dl>
    <dt>Distance (VRM)</dt>
    <dd data-testid="vrm-value">{mark.radiusM === null ? '—' : formatDistance(mark.radiusM)}</dd>
    <dt>Relèvement (EBL)</dt>
    <dd data-testid="ebl-value">{formatBearing(mark.bearingRad)}</dd>
  </dl>
  <form onsubmit={apply}>
    <label>
      Rayon ({unit.label})
      <input type="text" inputmode="decimal" bind:value={radiusText} size="6" />
    </label>
    <label>
      Relèvement (°)
      <input type="text" inputmode="decimal" bind:value={bearingText} size="4" />
    </label>
    <button type="submit">Appliquer</button>
  </form>
  {#if tools.error}
    <p class="error" role="alert">{tools.error}</p>
  {/if}
</section>

<style>
  .rb {
    min-width: 12rem;
    max-width: 17rem;
    padding: 0.5rem 0.6rem;
    border: 1px solid var(--border);
    border-left: 3px solid #ff6f00;
    border-radius: 0.4rem;
    background: var(--panel-bg);
    color: var(--fg);
    font-size: 0.85rem;
    box-shadow: 0 2px 8px rgb(0 0 0 / 25%);
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  h3 {
    margin: 0;
    color: var(--muted);
    font-size: 0.75rem;
    letter-spacing: 0.12em;
  }

  dl {
    display: grid;
    grid-template-columns: auto auto;
    gap: 0.15rem 0.6rem;
    margin: 0.35rem 0;
  }

  dt {
    color: var(--muted);
  }

  dd {
    margin: 0;
    font-weight: 600;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  p {
    margin: 0.35rem 0;
  }

  .hint {
    color: var(--muted);
  }

  .error {
    color: var(--error-fg);
    font-weight: 600;
  }

  form {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: 0.35rem;
  }

  label {
    display: flex;
    flex-direction: column;
    color: var(--muted);
    font-size: 0.75rem;
  }

  input {
    padding: 0.15rem 0.3rem;
    border: 1px solid var(--border);
    border-radius: 0.25rem;
    background: var(--bg);
    color: var(--fg);
    font: inherit;
    font-size: 0.85rem;
  }

  button {
    padding: 0.2rem 0.5rem;
    border: 1px solid var(--border);
    border-radius: 0.3rem;
    background: var(--button-bg);
    color: var(--fg);
    font: inherit;
    font-size: 0.8rem;
    cursor: pointer;
  }

  .close {
    padding: 0 0.35rem;
  }
</style>
