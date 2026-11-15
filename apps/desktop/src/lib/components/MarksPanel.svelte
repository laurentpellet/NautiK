<script lang="ts">
  // Marques personnelles : liste, création (à la position du bateau ou par
  // clic droit sur la carte), modification, import et export KML.
  import { formatLatitude, formatLongitude } from '../format';
  import type { MarksController } from '../marks.svelte';
  import {
    MARK_ICON_COLORS,
    MARK_ICON_LABELS,
    MARK_ICON_SYMBOLS,
    MARK_ICONS,
    type Mark,
  } from '../services/marks';
  import type { LatLon } from '../services/validate';

  interface Props {
    marks: MarksController;
    /** Position fraîche du bateau (`null` : inconnue ou obsolète). */
    boat: LatLon | null;
    onzoom: (position: LatLon) => void;
  }

  let { marks, boat, onzoom }: Props = $props();

  let filter = $state('');
  const shown = $derived.by((): Mark[] => {
    const q = filter.trim().toLowerCase();
    return q === ''
      ? marks.marks
      : marks.marks.filter(
          (m) => m.name.toLowerCase().includes(q) || (m.note ?? '').toLowerCase().includes(q),
        );
  });

  function save(event: SubmitEvent) {
    event.preventDefault();
    void marks.save();
  }

  function confirmDelete(mark: Mark) {
    if (confirm(`Supprimer la marque « ${mark.name} » ?`)) void marks.remove(mark.id);
  }
</script>

<section class="marks" aria-label="Marques">
  {#if marks.draft}
    <form class="editor" onsubmit={save} aria-label="Édition de la marque">
      <p class="where">
        {formatLatitude(marks.draft.position.latitudeDeg)}
        {formatLongitude(marks.draft.position.longitudeDeg)}
      </p>
      <label>
        Nom
        <input type="text" maxlength="100" required bind:value={marks.draft.name} />
      </label>
      <label>
        Type
        <select bind:value={marks.draft.icon}>
          {#each MARK_ICONS as icon (icon)}
            <option value={icon}>{MARK_ICON_SYMBOLS[icon]} {MARK_ICON_LABELS[icon]}</option>
          {/each}
        </select>
      </label>
      <label>
        Note
        <textarea rows="3" maxlength="2000" bind:value={marks.draft.note}></textarea>
      </label>
      <div class="actions">
        <button type="submit" disabled={marks.busy}>Enregistrer</button>
        <button type="button" onclick={() => marks.cancel()}>Annuler</button>
      </div>
    </form>
  {:else}
    <div class="actions">
      <button
        type="button"
        disabled={boat === null}
        title={boat === null
          ? 'Position du bateau inconnue ou obsolète'
          : 'Poser une marque à la position du bateau'}
        onclick={() => boat && marks.startNew(boat)}>+ Au bateau</button
      >
      <button type="button" disabled={marks.busy} onclick={() => marks.importKml()}>
        Importer KML
      </button>
      <button
        type="button"
        disabled={marks.busy || marks.marks.length === 0}
        onclick={() => marks.exportKml()}>Exporter KML</button
      >
    </div>
    <p class="muted hint">Clic droit sur la carte → « Marque ici ».</p>
    {#if marks.marks.length > 8}
      <input
        type="search"
        class="filter"
        placeholder="Filtrer les marques"
        aria-label="Filtrer les marques"
        bind:value={filter}
      />
    {/if}
  {/if}

  {#if marks.error}
    <p class="error" role="alert">{marks.error}</p>
  {/if}
  {#if marks.notice}
    <p class="notice" role="status">{marks.notice}</p>
  {/if}

  {#if marks.marks.length === 0}
    <p class="muted">Aucune marque.</p>
  {:else}
    <ul>
      {#each shown as mark (mark.id)}
        <li>
          <span
            class="icon"
            style:background={MARK_ICON_COLORS[mark.icon]}
            title={MARK_ICON_LABELS[mark.icon]}>{MARK_ICON_SYMBOLS[mark.icon]}</span
          >
          <div class="body">
            <button type="button" class="name" onclick={() => onzoom(mark.position)}>
              {mark.name}
            </button>
            {#if mark.note}<span class="note">{mark.note}</span>{/if}
          </div>
          <div class="row-actions">
            <button
              type="button"
              disabled={marks.busy}
              aria-label={`Modifier ${mark.name}`}
              onclick={() => marks.edit(mark.id)}>✎</button
            >
            <button
              type="button"
              class="danger"
              disabled={marks.busy}
              aria-label={`Supprimer ${mark.name}`}
              onclick={() => confirmDelete(mark)}>✕</button
            >
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .marks {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    font-size: 0.85rem;
  }

  .actions,
  .row-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
  }

  .editor {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    padding: 0.4rem;
    border: 1px solid var(--border);
    border-radius: 0.35rem;
  }

  .editor label {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    color: var(--muted);
    font-size: 0.78rem;
  }

  .where {
    margin: 0;
    color: var(--muted);
    font-size: 0.78rem;
    font-variant-numeric: tabular-nums;
  }

  .hint {
    margin: 0;
    font-size: 0.75rem;
  }

  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  li {
    display: flex;
    align-items: flex-start;
    gap: 0.4rem;
    padding: 0.25rem 0;
    border-bottom: 1px solid var(--border);
  }

  .icon {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: 1.4rem;
    height: 1.4rem;
    border-radius: 50%;
    color: #fff;
    font-size: 0.8rem;
  }

  .body {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
  }

  .name {
    padding: 0;
    border: 0;
    background: none;
    color: var(--fg);
    font: inherit;
    font-weight: 600;
    text-align: left;
    cursor: pointer;
  }

  .name:hover {
    text-decoration: underline;
  }

  .note {
    overflow: hidden;
    color: var(--muted);
    font-size: 0.78rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .muted {
    color: var(--muted);
  }

  .error {
    margin: 0;
    color: var(--danger, #d32f2f);
  }

  .notice {
    margin: 0;
    color: var(--muted);
  }

  .danger {
    color: var(--danger, #d32f2f);
  }
</style>
