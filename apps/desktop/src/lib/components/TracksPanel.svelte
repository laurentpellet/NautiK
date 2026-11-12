<script lang="ts">
  import type { NavigationController } from '../navigation.svelte';

  interface Props {
    nav: NavigationController;
  }

  let { nav }: Props = $props();

  const dateFormat = new Intl.DateTimeFormat('fr-FR', {
    dateStyle: 'short',
    timeStyle: 'short',
    timeZone: 'UTC',
  });

  function confirmDelete(id: string, name: string) {
    if (confirm(`Supprimer la trace « ${name} » ?`)) void nav.deleteTrack(id);
  }
</script>

<section class="tracks" aria-label="Traces">
  <h2>Traces</h2>
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
  </div>

  {#if nav.tracks.length === 0}
    <p class="muted">Aucune trace.</p>
  {:else}
    <ul>
      {#each nav.tracks as track (track.id)}
        <li>
          <label>
            <input
              type="checkbox"
              checked={track.id in nav.shownTracks}
              onchange={() => nav.toggleTrack(track.id)}
            />
            <span class:rec={track.isRecording}>{track.name}</span>
          </label>
          <div class="meta">
            <span class="muted">
              {dateFormat.format(track.startedAt)} UTC — {track.pointCount} pts
              {#if track.isRecording}(en cours){/if}
            </span>
            {#if !track.isRecording}
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
  h2 {
    margin: 0.75rem 0 0.25rem;
    font-size: 1rem;
  }

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

  .rec {
    color: #2e7d32;
    font-weight: 700;
  }

  .actions {
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
