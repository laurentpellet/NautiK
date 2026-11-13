<script lang="ts">
  // Barre de prévision affichée sur la carte : choix de l'échéance,
  // run du modèle, légende des couleurs. La prévision n'est jamais
  // présentée comme une mesure.
  import { WIND_SCALE } from '../map/wind';
  import {
    OLD_FORECAST_MS,
    formatUtc,
    type WeatherFile,
    type WindField,
  } from '../services/weather';

  interface Props {
    file: WeatherFile;
    step: number;
    field: WindField | null;
    error: string | null;
    now: number;
    onstep: (step: number) => void;
    onclose: () => void;
  }

  let { file, step, field, error, now, onstep, onclose }: Props = $props();

  const valid = $derived(file.validTimes[step] ?? null);
  const lead = $derived(
    valid !== null && file.referenceTime !== null
      ? Math.round((valid - file.referenceTime) / 3_600_000)
      : null,
  );
  const old = $derived(file.referenceTime !== null && now - file.referenceTime > OLD_FORECAST_MS);
  const past = $derived(valid !== null && valid < now - 3 * 3_600_000);
</script>

<div class="bar" data-testid="wind-bar">
  <div class="head">
    <strong>Vent prévu (10 m)</strong>
    <span class="muted">
      {file.id} · run {file.referenceTime !== null ? formatUtc(file.referenceTime) : '—'}
    </span>
    {#if old}<span class="badge" role="status">PRÉVISION ANCIENNE</span>{/if}
    <button type="button" class="close" aria-label="Masquer le vent" onclick={onclose}>✕</button>
  </div>
  <div class="time">
    <button
      type="button"
      aria-label="Échéance précédente"
      disabled={step <= 0}
      onclick={() => onstep(step - 1)}>◀</button
    >
    <input
      type="range"
      min="0"
      max={file.validTimes.length - 1}
      value={step}
      aria-label="Échéance"
      oninput={(e) => onstep(Number(e.currentTarget.value))}
    />
    <button
      type="button"
      aria-label="Échéance suivante"
      disabled={step >= file.validTimes.length - 1}
      onclick={() => onstep(step + 1)}>▶</button
    >
    <span class="valid" data-testid="wind-valid">
      {valid !== null ? formatUtc(valid) : '—'}{#if lead !== null}
        <span class="muted"> (+{lead} h)</span>{/if}
    </span>
    {#if past}<span class="badge" role="status">ÉCHÉANCE PASSÉE</span>{/if}
  </div>
  <div class="legend" aria-label="Légende (nœuds)">
    {#each WIND_SCALE as [kn, color] (kn)}
      <span><i style:background={color}></i>{kn}</span>
    {/each}
    <span class="muted">nœuds</span>
    {#if error}
      <span class="error" role="alert">{error}</span>
    {:else if field && field.points.length === 0}
      <span class="muted" role="status">Aucun vent dans la zone affichée.</span>
    {/if}
  </div>
</div>

<style>
  .bar {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    width: min(40rem, calc(100vw - 2rem));
    padding: 0.5rem 0.8rem;
    border: 1px solid var(--border);
    border-radius: 0.6rem;
    background: var(--card-bg);
    color: var(--fg);
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.3);
    font-size: 0.85rem;
  }

  .head,
  .time,
  .legend {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .muted {
    color: var(--muted);
  }

  .close {
    margin-left: auto;
  }

  button {
    padding: 0.1rem 0.5rem;
    border: 1px solid var(--border);
    border-radius: 0.3rem;
    background: var(--button-bg);
    color: var(--fg);
    font: inherit;
    cursor: pointer;
  }

  button:disabled {
    opacity: 0.4;
    cursor: default;
  }

  input[type='range'] {
    flex: 1;
    min-width: 8rem;
  }

  .valid {
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }

  .legend i {
    display: inline-block;
    width: 0.8rem;
    height: 0.5rem;
    margin-right: 0.2rem;
    border-radius: 0.15rem;
  }

  .badge {
    padding: 0 0.4rem;
    border-radius: 0.3rem;
    background: var(--warn-bg);
    color: var(--warn-fg);
    font-size: 0.75rem;
    font-weight: 700;
  }

  .error {
    color: var(--error-fg);
    font-weight: 600;
  }
</style>
