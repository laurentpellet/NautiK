<script lang="ts">
  // Onglet Journal : entrées manuelles, relevés automatiques du moteur
  // (intervalle réglable) et export CSV. Une position obsolète au moment
  // de l'entrée reste signalée ; une donnée absente s'affiche « — ».
  import type { Snippet } from 'svelte';
  import {
    formatBearing,
    formatDepth,
    formatLatitude,
    formatLongitude,
    formatSpeed,
  } from '../format';
  import type { LogbookController } from '../logbook.svelte';
  import { AUTO_INTERVALS_MIN } from '../services/logbook';

  interface Props {
    logbook: LogbookController;
    /** Alertes de sûreté ; absentes une fois intégrée aux Paramètres. */
    status?: Snippet;
    /** Intégrée à une section des Paramètres (pas en surimpression). */
    embedded?: boolean;
  }

  let { logbook, status, embedded = false }: Props = $props();

  let text = $state('');

  $effect(() => {
    void logbook.refresh();
  });

  const timeFormat = new Intl.DateTimeFormat('fr-FR', {
    dateStyle: 'short',
    timeStyle: 'short',
    timeZone: 'UTC',
  });

  const intervalLabel = (m: number) =>
    m === 0 ? 'Désactivés' : m < 60 ? `Toutes les ${m} min` : `Toutes les ${m / 60} h`;

  async function add(event: SubmitEvent) {
    event.preventDefault();
    if (await logbook.add(text)) text = '';
  }

  function confirmDelete(id: string) {
    if (confirm('Supprimer cette entrée du journal ?')) void logbook.remove(id);
  }
</script>

<section class="logbook" class:embedded aria-label="Journal de bord">
  {#if status}{@render status()}{/if}
  <div class="toolbar">
    <form class="add" onsubmit={add}>
      <input
        type="text"
        maxlength="2000"
        placeholder="Nouvelle entrée (ris pris, moteur, mouillage…)"
        aria-label="Texte de l'entrée"
        bind:value={text}
      />
      <button type="submit" disabled={logbook.busy || text.trim() === ''}>Ajouter</button>
    </form>
    <label>
      Relevés automatiques
      <select
        value={logbook.autoIntervalMin ?? ''}
        disabled={logbook.autoIntervalMin === null}
        onchange={(e) => logbook.setInterval(Number(e.currentTarget.value))}
      >
        {#each AUTO_INTERVALS_MIN as m (m)}
          <option value={m}>{intervalLabel(m)}</option>
        {/each}
      </select>
    </label>
    <button
      type="button"
      disabled={logbook.busy || logbook.entries.length === 0}
      onclick={() => logbook.exportCsv()}>Exporter CSV</button
    >
  </div>

  {#if logbook.error}
    <p class="error" role="alert">{logbook.error}</p>
  {/if}
  {#if logbook.notice}
    <p class="notice" role="status">{logbook.notice}</p>
  {/if}

  {#if logbook.entries.length === 0}
    <p class="muted">Journal vide.</p>
  {:else}
    <div class="table">
      <table>
        <thead>
          <tr>
            <th>Heure (UTC)</th>
            <th>Position</th>
            <th>Fond</th>
            <th>Cap</th>
            <th>Vent</th>
            <th>Baro</th>
            <th>Sonde</th>
            <th>Entrée</th>
            <th><span class="sr">Actions</span></th>
          </tr>
        </thead>
        <tbody>
          {#each logbook.entries as e (e.id)}
            <tr class:auto={e.kind === 'auto'}>
              <td class="num">{timeFormat.format(e.at)}</td>
              <td class="num" class:stale={e.positionStale}>
                {#if e.position}
                  {formatLatitude(e.position.latitudeDeg)}<br />{formatLongitude(
                    e.position.longitudeDeg,
                  )}
                  {#if e.positionStale}<span class="tag">obsolète</span>{/if}
                {:else}
                  <span class="tag">aucune</span>
                {/if}
              </td>
              <td class="num">{formatSpeed(e.sogMps)}<br />{formatBearing(e.cogRad)}</td>
              <td class="num">{formatBearing(e.headingRad)}</td>
              <td class="num">{formatSpeed(e.twsMps)}<br />{formatBearing(e.twdRad)}</td>
              <td class="num"
                >{e.pressurePa === null ? '—' : `${(e.pressurePa / 100).toFixed(1)} hPa`}</td
              >
              <td class="num">{formatDepth(e.depthM)}</td>
              <td class="text">
                {#if e.kind === 'auto'}<span class="tag">auto</span>{/if}
                {#if e.simulated}<span class="tag sim">simulation</span>{/if}
                {e.text ?? ''}
              </td>
              <td>
                <button
                  type="button"
                  class="danger"
                  aria-label="Supprimer l'entrée"
                  disabled={logbook.busy}
                  onclick={() => confirmDelete(e.id)}>✕</button
                >
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    {#if logbook.more}
      <button type="button" disabled={logbook.busy} onclick={() => logbook.loadMore()}>
        Entrées plus anciennes
      </button>
    {/if}
  {/if}
</section>

<style>
  /* Au-dessus de la carte, qui reste montée sous les autres onglets. */
  .logbook {
    position: absolute;
    inset: 0;
    z-index: 3;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    overflow: auto;
    padding: 1rem 1rem 2.5rem;
    background: var(--bg);
    color: var(--fg);
    font-size: 0.85rem;
  }

  .toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem 1rem;
  }

  .add {
    display: flex;
    flex: 1 1 22rem;
    gap: 0.3rem;
  }

  .add input {
    flex: 1;
    min-width: 0;
  }

  label {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    color: var(--muted);
  }

  .table {
    overflow-x: auto;
    /* Contient le libellé masqué « Actions » (sinon il élargit la page). */
    position: relative;
  }

  table {
    width: 100%;
    border-collapse: collapse;
  }

  th,
  td {
    padding: 0.3rem 0.45rem;
    border-bottom: 1px solid var(--border);
    text-align: left;
    vertical-align: top;
  }

  th {
    color: var(--muted);
    font-size: 0.75rem;
    font-weight: 600;
  }

  .num {
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  tr.auto td {
    color: var(--muted);
  }

  .stale {
    color: var(--warning, #ef6c00);
  }

  .tag {
    display: inline-block;
    margin-right: 0.3rem;
    padding: 0 0.3rem;
    border: 1px solid currentcolor;
    border-radius: 0.25rem;
    font-size: 0.7rem;
  }

  .tag.sim {
    color: var(--warning, #ef6c00);
  }

  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
  }

  .muted,
  .notice {
    margin: 0;
    color: var(--muted);
  }

  .error {
    margin: 0;
    color: var(--danger, #d32f2f);
  }

  .danger {
    color: var(--danger, #d32f2f);
  }

  /* Section des Paramètres : dans le flux, sans fond propre. */
  .logbook.embedded {
    position: static;
    inset: auto;
    z-index: auto;
    overflow: visible;
    padding: 0;
    background: none;
  }
</style>
