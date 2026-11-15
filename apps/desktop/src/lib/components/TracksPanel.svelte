<script lang="ts">
  // Traces : enregistrement, affichage sur la carte (cadrée sur la trace),
  // longueur et durée (moteur), renommage, export et suppression.
  import { formatDistance, formatDuration } from '../format';
  import type { NavigationController } from '../navigation.svelte';

  interface Props {
    nav: NavigationController;
    /** Relecture d'une trace (statistiques, curseur, vitesse en couleur). */
    onreplay?: (id: string, name: string) => void;
  }

  let { nav, onreplay }: Props = $props();

  const dateFormat = new Intl.DateTimeFormat('fr-FR', {
    dateStyle: 'short',
    timeStyle: 'short',
    timeZone: 'UTC',
  });

  /** Trace en cours de renommage et nom saisi. */
  let renaming = $state<string | null>(null);
  let newName = $state('');

  function startRename(id: string, name: string) {
    renaming = id;
    newName = name;
  }

  async function rename(event: SubmitEvent, id: string) {
    event.preventDefault();
    if (await nav.renameTrack(id, newName)) renaming = null;
  }

  function confirmDelete(id: string, name: string) {
    if (confirm(`Supprimer la trace « ${name} » ?`)) void nav.deleteTrack(id);
  }
</script>

<section class="tracks" aria-label="Traces">
  <div class="actions">
    {#if nav.recordingId}
      <button type="button" class="recording" onclick={() => nav.stopRecording()}>
        ■ Arrêter l'enregistrement
      </button>
    {:else}
      <button type="button" disabled={nav.busy} onclick={() => nav.startRecording()}>
        ● Enregistrer la trace
      </button>
    {/if}
    <button type="button" disabled={nav.busy} onclick={() => nav.importGpx()}>Importer GPX</button>
  </div>

  {#if nav.tracks.length === 0}
    <p class="muted">Aucune trace.</p>
  {:else}
    <ul>
      {#each nav.tracks as track (track.id)}
        {@const info = nav.trackInfo[track.id]}
        <li>
          {#if renaming === track.id}
            <form class="rename" onsubmit={(e) => rename(e, track.id)}>
              <input
                type="text"
                maxlength="100"
                required
                bind:value={newName}
                aria-label="Nouveau nom de la trace"
              />
              <button type="submit" disabled={nav.busy}>OK</button>
              <button type="button" onclick={() => (renaming = null)}>Annuler</button>
            </form>
          {:else}
            <label>
              <input
                type="checkbox"
                checked={track.id in nav.shownTracks}
                onchange={() => nav.toggleTrack(track.id)}
              />
              <span class:rec={track.isRecording}>{track.name}</span>
            </label>
          {/if}
          <div class="meta">
            <span class="muted">
              {dateFormat.format(track.startedAt)} UTC — {track.pointCount} pts
              {#if track.isRecording}(en cours){/if}
            </span>
          </div>
          {#if info && track.id in nav.shownTracks}
            <dl class="info" data-testid="track-info">
              <div>
                <dt>Longueur</dt>
                <dd>{formatDistance(info.lengthM)}</dd>
              </div>
              <div>
                <dt>Durée</dt>
                <dd>{info.durationS === null ? '—' : formatDuration(info.durationS * 1000)}</dd>
              </div>
            </dl>
          {/if}
          <div class="actions">
            <button
              type="button"
              disabled={nav.busy || track.pointCount === 0}
              title="Afficher la trace et cadrer la carte dessus"
              onclick={() => nav.zoomTrack(track.id)}>Zoomer</button
            >
            {#if onreplay}
              <button
                type="button"
                disabled={nav.busy || track.pointCount < 2}
                title="Rejouer la trace : statistiques et vitesse en couleur"
                onclick={() => onreplay(track.id, track.name)}>Relire</button
              >
            {/if}
            {#if !track.isRecording}
              <button
                type="button"
                disabled={nav.busy}
                onclick={() => startRename(track.id, track.name)}>Renommer</button
              >
              <button type="button" disabled={nav.busy} onclick={() => nav.exportTrack(track.id)}>
                GPX
              </button>
              <button
                type="button"
                class="danger"
                disabled={nav.busy}
                aria-label={`Supprimer ${track.name}`}
                onclick={() => confirmDelete(track.id, track.name)}>✕</button
              >
            {/if}
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  ul {
    margin: 0.25rem 0;
    padding: 0;
    list-style: none;
  }

  li {
    padding: 0.3rem 0;
    border-top: 1px solid var(--border);
    font-size: 0.85rem;
  }

  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.3rem;
  }

  .info {
    display: flex;
    gap: 1rem;
    margin: 0.2rem 0;
  }

  .info dt {
    color: var(--muted);
    font-size: 0.65rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .info dd {
    margin: 0;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .rename {
    display: flex;
    gap: 0.3rem;
  }

  .rename input {
    flex: 1;
    min-width: 0;
    padding: 0.25rem;
    border: 1px solid var(--border);
    border-radius: 0.3rem;
    background: var(--button-bg);
    color: var(--fg);
    font: inherit;
  }

  .rec {
    color: #2e7d32;
    font-weight: 700;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
    margin: 0.25rem 0;
  }

  .muted {
    color: var(--muted);
    font-size: 0.8rem;
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

  button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .recording {
    color: var(--error-fg);
    font-weight: 700;
  }

  .danger {
    color: var(--error-fg);
  }
</style>
