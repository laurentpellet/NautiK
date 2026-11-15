<script lang="ts">
  // Paramètres → Cartes : dossier des fichiers MBTiles (choisi par le
  // dialogue du système côté moteur), relecture et retour au dossier par
  // défaut. Un dossier absent est signalé, jamais remplacé en silence.
  import type { ChartCatalog, ChartsApi, ChartsDirSource } from '../services/charts';
  import {
    CHART_SORTS,
    MAX_CHART_NAME_CHARS,
    chartNames,
    chartSort,
    setChartName,
    setChartSort,
    sortCharts,
  } from '../chartNames.svelte';

  interface Props {
    api: ChartsApi;
    catalog: ChartCatalog | null;
    onchange: (catalog: ChartCatalog) => void;
  }

  let { api, catalog, onchange }: Props = $props();

  let busy = $state(false);
  let error = $state<string | null>(null);

  const SOURCE_LABELS: Record<ChartsDirSource, string> = {
    default: "dossier par défaut de l'application",
    settings: 'dossier choisi',
    environment: 'imposé par la variable NAUTIK_CHARTS_DIR',
  };

  const locked = $derived(catalog?.chartsDirSource === 'environment');

  const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

  async function run(action: () => Promise<ChartCatalog | null>): Promise<void> {
    busy = true;
    try {
      const updated = await action();
      if (updated) onchange(updated);
      error = null;
    } catch (e) {
      error = message(e);
    } finally {
      busy = false;
    }
  }
</script>

<section class="panel" aria-label="Cartes" data-testid="charts-folder">
  <h2>CARTES</h2>
  {#if catalog}
    <p>
      Dossier : <code class="path" data-testid="charts-dir">{catalog.chartsDir}</code>
      <span class="muted">({SOURCE_LABELS[catalog.chartsDirSource]})</span>
    </p>
    {#if catalog.error}
      <p class="error" role="alert">Cartes indisponibles : {catalog.error}</p>
    {:else}
      <p class="muted" data-testid="charts-count">
        {catalog.charts.length === 0
          ? 'Aucune carte .mbtiles dans ce dossier.'
          : `${catalog.charts.length} carte${catalog.charts.length > 1 ? 's' : ''} chargée${catalog.charts.length > 1 ? 's' : ''}.`}
        {#if catalog.failures.length > 0}
          {catalog.failures.length} fichier{catalog.failures.length > 1 ? 's' : ''} ignoré{catalog
            .failures.length > 1
            ? 's'
            : ''}.
        {/if}
      </p>
    {/if}
    {#if !catalog.error && catalog.charts.length > 0}
      <!-- Cartes chargées, triées au choix : nom affiché dans l'application (calques, volet
           de droite), modifiable ; vide : nom des métadonnées. -->
      <label class="sort">
        Trier par
        <select value={chartSort.value} onchange={(e) => setChartSort(e.currentTarget.value)}>
          {#each CHART_SORTS as s (s.id)}
            <option value={s.id}>{s.label}</option>
          {/each}
        </select>
      </label>
      <table class="charts" aria-label="Cartes chargées">
        <thead>
          <tr>
            <th scope="col">Nom dans l'application</th>
            <th scope="col">Fichier</th>
            <th scope="col">Zooms</th>
          </tr>
        </thead>
        <tbody>
          {#each sortCharts(catalog.charts, chartSort.value, chartNames.names) as chart (chart.id)}
            <tr data-chart={chart.id}>
              <td>
                <div class="name">
                  <input
                    type="text"
                    maxlength={MAX_CHART_NAME_CHARS}
                    aria-label="Nom de la carte {chart.fileName}"
                    placeholder={chart.name}
                    value={chartNames.names[chart.id] ?? ''}
                    onchange={(e) => setChartName(chart.id, e.currentTarget.value)}
                    onkeydown={(e) => {
                      if (e.key === 'Enter') e.currentTarget.blur();
                    }}
                  />
                  <button
                    type="button"
                    disabled={!chartNames.names[chart.id]}
                    aria-label="Nom d'origine pour {chart.fileName}"
                    title="Revenir au nom d'origine : {chart.name}"
                    onclick={() => setChartName(chart.id, '')}>↺</button
                  >
                </div>
                {#if chartNames.names[chart.id]}
                  <span class="muted">Nom d'origine : {chart.name}</span>
                {/if}
              </td>
              <td><code class="path">{chart.fileName}</code></td>
              <td class="zooms">{chart.minZoom}–{chart.maxZoom}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}

    <div class="row">
      <button type="button" disabled={busy || locked} onclick={() => run(() => api.chooseDir())}>
        Choisir un dossier…
      </button>
      <button type="button" disabled={busy} onclick={() => run(() => api.reload())}>
        Relire le dossier
      </button>
      <button
        type="button"
        disabled={busy || locked || catalog.chartsDirSource === 'default'}
        onclick={() => run(() => api.resetDir())}
      >
        Dossier par défaut
      </button>
    </div>
    <p class="hint">
      Par exemple Documents\Charts, ou une clé USB. Tous les fichiers <code>.mbtiles</code> du dossier
      sont chargés ; après y avoir copié des cartes, utilisez « Relire le dossier ».
    </p>
  {:else}
    <p class="muted">Chargement des cartes…</p>
  {/if}
  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}
</section>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  h2 {
    margin: 0;
    color: var(--muted);
    font-size: 0.9rem;
    letter-spacing: 0.12em;
  }

  p {
    margin: 0;
  }

  .path {
    word-break: break-all;
  }

  .row {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .muted,
  .hint {
    color: var(--muted);
    font-size: 0.85rem;
  }

  .error {
    color: var(--error-fg);
    font-weight: 600;
    font-size: 0.85rem;
  }

  button {
    padding: 0.3rem 0.6rem;
    border: 1px solid var(--border);
    border-radius: 0.3rem;
    background: var(--button-bg);
    color: var(--fg);
    font: inherit;
    cursor: pointer;
  }

  .sort {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.85rem;
  }

  .charts {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.85rem;
  }

  .charts th {
    color: var(--muted);
    font-weight: 600;
    text-align: left;
  }

  .charts th,
  .charts td {
    padding: 0.25rem 0.4rem;
    border-bottom: 1px solid var(--border);
    vertical-align: top;
  }

  .charts .name {
    display: flex;
    gap: 0.3rem;
  }

  .charts input {
    flex: 1;
    min-width: 8rem;
  }

  .charts .zooms {
    white-space: nowrap;
  }

  button:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
