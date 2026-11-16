<script lang="ts">
  // Contenu du menu Calques (ouvert depuis la barre d'outils) : cartes
  // MBTiles (quand plusieurs sont chargées) et cartes vectorielles (ENC,
  // CM93), puis objets. Le rappel des calques masqués est affiché sur la
  // carte par l'application. Choix unique (volet de droite d'une vue
  // double) : boutons radio, parmi les cartes proposées seulement.
  import {
    LAYER_LABELS,
    layerLabel,
    OBJECT_KEYS,
    toggleChart,
    vectorKeys,
    type MapLayers,
    type VectorCharts,
  } from '../layers';
  import type { ChartInfo } from '../services/charts';
  import { sortCharts, type ChartSort } from '../chartNames.svelte';
  import { chartChoice } from '../split';

  /** Choix d'une seule carte parmi `options` (voir `rightChartOptions`). */
  interface SingleChoice {
    /** Carte affichée ; `null` : aucune proposée. */
    value: string | null;
    /** Cartes proposées (`chart:<id>`, `enc`, `cm93`). */
    options: string[];
    onselect: (value: string) => void;
  }

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
    /** Nom choisi pour la CM93 (Paramètres → Cartes), sinon libellé par défaut. */
    cm93Name?: string | null;
    /** Tri de la liste des cartes (Paramètres → Cartes). */
    sort?: ChartSort;
    /** Le dossier ENC contient des cellules S-57 : leur choix est proposé. */
    enc?: boolean;
    /** Afficher aussi les objets (routes, traces, AIS). */
    objects?: boolean;
    /** Une seule carte à choisir (boutons radio) au lieu de cases. */
    single?: SingleChoice;
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
    cm93Name = null,
    sort = 'detail',
    enc = true,
    objects = true,
    single,
  }: Props = $props();

  // Ordre choisi dans Paramètres → Cartes (par défaut : plus détaillée en tête).
  const chartList = $derived(sortCharts(charts, sort));
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
  {#if single}
    {@const name = `single-chart-${single.options.length}`}
    {#each chartList.filter((c) => single.options.includes(chartChoice(c.id))) as chart (chart.id)}
      <label title={chart.description ?? chart.fileName}>
        <input
          type="radio"
          {name}
          checked={single.value === chartChoice(chart.id)}
          onclick={() => single.onselect(chartChoice(chart.id))}
        />
        {chart.name}
        <span class="zoom">z{chart.minZoom}–{chart.maxZoom}</span>
      </label>
    {/each}
    {#each vectorKeys(cm93, enc).filter((k) => single.options.includes(k)) as key (key)}
      <label>
        <input
          type="radio"
          {name}
          checked={single.value === key}
          onclick={() => single.onselect(key)}
        />
        {layerLabel(key, cm93Name)}
      </label>
    {/each}
    {#if single.options.length === 0}
      <p class="note" data-testid="no-right-chart">
        Toutes les cartes sont déjà affichées à gauche : masquez-en une dans le menu Calques pour la
        comparer ici.
      </p>
    {/if}
  {:else if chartList.length > 1}
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
  {#if !single}
    {#each vectorKeys(cm93, enc) as key (key)}
      <label>
        <input
          type="checkbox"
          checked={shownVector[key]}
          onchange={(e) => setVector(key, e.currentTarget.checked)}
        />
        {layerLabel(key, cm93Name)}
      </label>
    {/each}
  {/if}
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

  .note {
    max-width: 16rem;
    margin: 0.2rem 0;
    color: var(--muted);
    font-size: 0.75rem;
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
