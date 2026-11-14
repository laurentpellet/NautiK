<script lang="ts">
  // Contenu du menu Calques (ouvert depuis la barre d'outils) : cartes
  // MBTiles (quand plusieurs sont chargées) et cartes vectorielles (ENC,
  // CM93), puis objets. Le rappel des calques masqués est affiché sur la
  // carte par l'application.
  import {
    LAYER_LABELS,
    OBJECT_KEYS,
    toggleChart,
    vectorKeys,
    type MapLayers,
    type VectorCharts,
  } from '../layers';
  import type { ChartInfo } from '../services/charts';

  interface Props {
    layers: MapLayers;
    onchange: (layers: MapLayers) => void;
    /** Cartes du catalogue, de la moins à la plus détaillée. */
    charts?: ChartInfo[];
    /** Identifiants des cartes masquées. */
    hiddenCharts?: string[];
    onchartschange?: (hidden: string[]) => void;
    /** Cartes vectorielles du volet ; par défaut celles de `layers`. */
    vector?: VectorCharts;
    onvectorchange?: (vector: VectorCharts) => void;
    /** Une base CM93 est trouvée : son choix est proposé. */
    cm93?: boolean;
    /** Afficher aussi les objets (routes, traces, AIS). */
    objects?: boolean;
  }

  let {
    layers,
    onchange,
    charts = [],
    hiddenCharts = [],
    onchartschange,
    vector,
    onvectorchange,
    cm93 = false,
    objects = true,
  }: Props = $props();

  // Plus détaillée en tête, comme dans le panneau Cartes.
  const chartList = $derived([...charts].reverse());
  const shownVector = $derived(vector ?? { enc: layers.enc, cm93: layers.cm93 });

  function setVector(key: keyof VectorCharts, checked: boolean): void {
    const next = { ...shownVector, [key]: checked };
    if (onvectorchange) onvectorchange(next);
    else onchange({ ...layers, ...next });
  }
</script>

<fieldset>
  <legend class="sr">Calques affichés</legend>
  <p class="group">Cartes</p>
  {#if chartList.length > 1}
    {#each chartList as chart (chart.id)}
      <label title={chart.description ?? chart.fileName}>
        <input
          type="checkbox"
          checked={!hiddenCharts.includes(chart.id)}
          onchange={(e) =>
            onchartschange?.(toggleChart(hiddenCharts, chart.id, e.currentTarget.checked))}
        />
        {chart.name}
        <span class="zoom">z{chart.minZoom}–{chart.maxZoom}</span>
      </label>
    {/each}
  {/if}
  {#each vectorKeys(cm93) as key (key)}
    <label>
      <input
        type="checkbox"
        checked={shownVector[key]}
        onchange={(e) => setVector(key, e.currentTarget.checked)}
      />
      {LAYER_LABELS[key]}
    </label>
  {/each}
  {#if objects}
    <hr />
    <p class="group">Objets</p>
    {#each OBJECT_KEYS as key (key)}
      <label>
        <input
          type="checkbox"
          checked={layers[key]}
          onchange={(e) => onchange({ ...layers, [key]: e.currentTarget.checked })}
        />
        {LAYER_LABELS[key]}
      </label>
    {/each}
  {/if}
</fieldset>

<style>
  fieldset {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    margin: 0;
    padding: 0.5rem 0.8rem;
    border: 0;
    font-size: 0.85rem;
  }

  .group {
    margin: 0;
    color: var(--muted);
    font-size: 0.7rem;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }

  hr {
    width: 100%;
    margin: 0.3rem 0;
    border: 0;
    border-top: 1px solid var(--border);
  }

  .zoom {
    color: var(--muted);
    font-size: 0.75rem;
  }

  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
  }
</style>
