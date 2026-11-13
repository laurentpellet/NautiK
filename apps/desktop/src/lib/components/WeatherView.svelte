<script lang="ts">
  // Onglet Météo : bibliothèque de fichiers GRIB (hors ligne), import,
  // téléchargement GFS optionnel et choix du fichier affiché sur la carte.
  import type { Snippet } from 'svelte';
  import type { ListenFn } from '../services/position';
  import {
    HORIZONS_H,
    OLD_FORECAST_MS,
    WeatherApi,
    downloadArea,
    formatArea,
    formatUtc,
    subscribeDownload,
    type Bounds,
    type DownloadProgress,
    type WeatherFile,
  } from '../services/weather';
  import { holdSubscription } from '../subscription';

  interface Props {
    api: WeatherApi;
    /** Zone affichée sur la carte. */
    view: Bounds | null;
    /** Fichier affiché sur la carte. */
    active: string | null;
    onshow: (file: WeatherFile | null) => void;
    now: number;
    status: Snippet;
    listen?: ListenFn;
  }

  let { api, view, active, onshow, now, status, listen }: Props = $props();

  let files = $state.raw<WeatherFile[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let notice = $state<string | null>(null);
  let horizon = $state<number>(48);
  let downloading = $state(false);
  let progress = $state<DownloadProgress | null>(null);
  let confirmDelete = $state<string | null>(null);

  const area = $derived(view ? downloadArea(view) : null);
  const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

  async function refresh(): Promise<void> {
    try {
      files = await api.list();
      error = null;
    } catch (e) {
      error = `Bibliothèque illisible : ${message(e)}`;
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    void refresh();
  });

  $effect(() =>
    holdSubscription(
      () => subscribeDownload((p) => (progress = p), listen),
      () => {},
    ),
  );

  async function download(): Promise<void> {
    if (!area) return;
    downloading = true;
    progress = null;
    error = null;
    notice = null;
    try {
      const file = await api.download(area, horizon);
      notice = `Téléchargé : ${file.id} (${file.validTimes.length} échéances).`;
      await refresh();
      onshow(file);
    } catch (e) {
      error = `Téléchargement impossible : ${message(e)}`;
    } finally {
      downloading = false;
    }
  }

  async function importFile(): Promise<void> {
    error = null;
    notice = null;
    try {
      const file = await api.importFile();
      if (!file) return;
      notice = `Importé : ${file.id} (${file.validTimes.length} échéances).`;
      await refresh();
      onshow(file);
    } catch (e) {
      error = `Import impossible : ${message(e)}`;
    }
  }

  async function remove(file: WeatherFile): Promise<void> {
    confirmDelete = null;
    try {
      await api.remove(file.id);
      if (active === file.id) onshow(null);
      await refresh();
    } catch (e) {
      error = `Suppression impossible : ${message(e)}`;
    }
  }

  const sizeLabel = (bytes: number) =>
    bytes >= 1_048_576 ? `${(bytes / 1_048_576).toFixed(1)} Mo` : `${Math.ceil(bytes / 1024)} ko`;
</script>

<section class="weather" aria-label="Météo">
  <div class="status">{@render status()}</div>

  <p class="safety" role="note">
    Prévisions de modèle numérique : une aide à la décision, qui ne remplace pas les bulletins
    météorologiques officiels. Vent à 10 m, sans rafales.
  </p>

  <div class="grid">
    <article class="card">
      <h2>TÉLÉCHARGER — GFS 0,25° (NOAA)</h2>
      <p class="muted">
        Internet requis. Le fichier est ensuite conservé pour un usage hors ligne.
      </p>
      <p data-testid="download-area">
        Zone : {area ? formatArea(area) : 'carte non disponible'}
        {#if area && view && area !== view}
          <span class="muted">(réduite autour du centre de la carte)</span>
        {/if}
      </p>
      <label>
        Horizon
        <select bind:value={horizon} disabled={downloading}>
          {#each HORIZONS_H as h (h)}
            <option value={h}>{h} h</option>
          {/each}
        </select>
      </label>
      <button type="button" class="primary" disabled={!area || downloading} onclick={download}>
        {downloading ? 'Téléchargement…' : 'Télécharger la zone affichée'}
      </button>
      {#if downloading && progress}
        <progress max={progress.total} value={progress.done} aria-label="Progression"></progress>
        <span class="muted">{progress.done} / {progress.total} échéances</span>
      {/if}
    </article>

    <article class="card">
      <h2>IMPORTER</h2>
      <p class="muted">
        Fichier GRIB2 (vent à 10 m, grille régulière) obtenu par e-mail, satellite ou un autre
        service.
      </p>
      <button type="button" onclick={importFile}>Importer un fichier GRIB…</button>
    </article>
  </div>

  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if notice}<p class="notice" role="status">{notice}</p>{/if}

  <article class="card">
    <h2>FICHIERS DISPONIBLES</h2>
    {#if loading}
      <p class="muted">Chargement…</p>
    {:else if files.length === 0}
      <p class="muted" data-testid="no-weather">Aucun fichier météo.</p>
    {:else}
      <ul data-testid="weather-files">
        {#each files as f (f.id)}
          {@const old = f.referenceTime !== null && now - f.referenceTime > OLD_FORECAST_MS}
          {@const last = f.validTimes.at(-1)}
          <li class:active={active === f.id}>
            <div class="info">
              <strong>{f.id}</strong>
              {#if f.error}
                <span class="error">Inexploitable : {f.error}</span>
              {:else}
                <span>
                  Run {f.referenceTime !== null ? formatUtc(f.referenceTime) : '—'}
                  {#if old}<span class="badge" role="status">ANCIENNE</span>{/if}
                </span>
                <span class="muted">
                  {f.validTimes.length} échéances{#if last !== undefined}, jusqu'au {formatUtc(
                      last,
                    )}{/if}
                  · {f.bounds ? formatArea(f.bounds) : ''} · {sizeLabel(f.sizeBytes)}
                </span>
                {#if f.skipped.length > 0}
                  <span class="muted" title={f.skipped.join('\n')}>
                    {f.skipped.length} champ(s) ignoré(s)
                  </span>
                {/if}
              {/if}
            </div>
            <div class="actions">
              {#if !f.error}
                <button
                  type="button"
                  aria-pressed={active === f.id}
                  onclick={() => onshow(active === f.id ? null : f)}
                >
                  {active === f.id ? 'Masquer' : 'Afficher sur la carte'}
                </button>
              {/if}
              {#if confirmDelete === f.id}
                <button type="button" class="danger" onclick={() => remove(f)}>
                  Confirmer la suppression
                </button>
                <button type="button" onclick={() => (confirmDelete = null)}>Annuler</button>
              {:else}
                <button type="button" onclick={() => (confirmDelete = f.id)}>Supprimer</button>
              {/if}
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  </article>
</section>

<style>
  .weather {
    position: absolute;
    inset: 0;
    z-index: 3;
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
    overflow: auto;
    padding: 1rem 1rem 2.5rem;
    background: var(--bg);
    color: var(--fg);
  }

  .safety {
    margin: 0;
    padding: 0.4rem 0.7rem;
    border-radius: 0.4rem;
    background: var(--warn-bg);
    color: var(--warn-fg);
    font-size: 0.85rem;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(20rem, 1fr));
    gap: 0.9rem;
  }

  .card {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.5rem;
    padding: 1rem 1.2rem;
    border: 1px solid var(--border);
    border-radius: 0.8rem;
    background: var(--card-bg);
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

  .muted {
    color: var(--muted);
    font-size: 0.85rem;
  }

  label {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  button,
  select {
    padding: 0.35rem 0.8rem;
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    background: var(--button-bg);
    color: var(--fg);
    font: inherit;
    font-size: 0.9rem;
    cursor: pointer;
  }

  button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .primary {
    background: var(--accent-bg);
    color: var(--accent-fg);
  }

  .danger {
    background: var(--alert-bg);
    color: var(--alert-fg);
  }

  progress {
    width: 100%;
  }

  ul {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    width: 100%;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  li {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
    padding: 0.6rem 0.8rem;
    border: 1px solid var(--border);
    border-radius: 0.5rem;
    background: var(--inset-bg);
  }

  li.active {
    border-color: var(--accent-bg);
  }

  .info {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .actions {
    display: flex;
    gap: 0.4rem;
    flex-wrap: wrap;
    justify-content: flex-end;
  }

  .badge {
    margin-left: 0.4rem;
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

  .notice {
    color: var(--ok-fg);
  }
</style>
