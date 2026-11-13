<script lang="ts">
  // Menu des calques de la carte. Un calque masqué reste signalé sur la
  // carte : rien n'est caché silencieusement.
  import { LAYER_LABELS, hiddenLayers, type MapLayers } from '../layers';

  interface Props {
    layers: MapLayers;
    onchange: (layers: MapLayers) => void;
  }

  let { layers, onchange }: Props = $props();

  let open = $state(false);
  const keys = Object.keys(LAYER_LABELS) as (keyof MapLayers)[];
  const hidden = $derived(hiddenLayers(layers));
</script>

<div class="layers">
  <button type="button" aria-expanded={open} onclick={() => (open = !open)}>Calques ▾</button>
  {#if open}
    <fieldset>
      <legend class="sr">Calques affichés</legend>
      {#each keys as key (key)}
        <label>
          <input
            type="checkbox"
            checked={layers[key]}
            onchange={(e) => onchange({ ...layers, [key]: e.currentTarget.checked })}
          />
          {LAYER_LABELS[key]}
        </label>
      {/each}
    </fieldset>
  {/if}
  {#if hidden.length > 0}
    <p class="hidden-note" role="status" data-testid="hidden-layers">
      Masqué : {hidden.join(', ')}
    </p>
  {/if}
</div>

<style>
  .layers {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 0.3rem;
  }

  button {
    padding: 0.3rem 0.7rem;
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    background: var(--card-bg);
    color: var(--fg);
    font: inherit;
    font-size: 0.85rem;
    font-weight: 600;
    cursor: pointer;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.25);
  }

  fieldset {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    margin: 0;
    padding: 0.5rem 0.8rem;
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    background: var(--card-bg);
    color: var(--fg);
    font-size: 0.85rem;
  }

  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
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
</style>
