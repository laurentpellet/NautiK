<script lang="ts">
  import { formatDistance } from '../format';
  import type { NavigationController } from '../navigation.svelte';

  interface Props {
    nav: NavigationController;
  }

  let { nav }: Props = $props();

  let name = $state('');

  async function save(event: SubmitEvent) {
    event.preventDefault();
    if (await nav.finishDrawing(name)) name = '';
  }

  function confirmDelete(id: string, routeName: string) {
    if (confirm(`Supprimer la route « ${routeName} » ?`)) void nav.deleteRoute(id);
  }
</script>

<section class="routes" aria-label="Routes">
  <h2>Routes</h2>

  {#if nav.drawing}
    <form class="draft" onsubmit={save}>
      <p class="hint" role="status">
        Cliquez sur la carte pour ajouter des points ({nav.draft.length} point{nav.draft.length > 1
          ? 's'
          : ''}).
      </p>
      <input
        type="text"
        placeholder="Nom de la route"
        maxlength="100"
        required
        bind:value={name}
        aria-label="Nom de la route"
      />
      <div class="actions">
        <button type="submit" disabled={nav.draft.length < 2 || nav.busy}>Enregistrer</button>
        <button type="button" disabled={nav.draft.length === 0} onclick={() => nav.undoPoint()}>
          Annuler le dernier point
        </button>
        <button type="button" onclick={() => nav.cancelDrawing()}>Abandonner</button>
      </div>
    </form>
  {:else}
    <div class="actions">
      <button type="button" onclick={() => nav.startDrawing()}>Nouvelle route</button>
      <button type="button" disabled={nav.busy} onclick={() => nav.importGpx()}>
        Importer GPX
      </button>
    </div>
  {/if}

  {#if nav.routes.length === 0}
    <p class="muted">Aucune route.</p>
  {:else}
    <ul>
      {#each nav.routes as route (route.id)}
        <li class:active={route.active}>
          <div class="title">
            <span>{route.name}</span>
            <span class="muted">
              {formatDistance(route.totalDistanceM)} — {route.waypoints.length} WP
            </span>
          </div>
          <div class="actions">
            {#if route.active}
              <button type="button" onclick={() => nav.deactivate()}>Arrêter</button>
            {:else}
              <button
                type="button"
                disabled={nav.busy || nav.activeRouteId !== null}
                onclick={() => nav.activate(route.id)}>Naviguer</button
              >
              <button type="button" disabled={nav.busy} onclick={() => nav.reverse(route.id)}>
                Inverser
              </button>
            {/if}
            <button type="button" disabled={nav.busy} onclick={() => nav.exportRoute(route.id)}>
              GPX
            </button>
            {#if !route.active}
              <button
                type="button"
                class="danger"
                disabled={nav.busy}
                aria-label={`Supprimer ${route.name}`}
                onclick={() => confirmDelete(route.id, route.name)}>✕</button
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
  }

  li.active .title span:first-child {
    color: #d81b60;
    font-weight: 700;
  }

  .title {
    display: flex;
    justify-content: space-between;
    gap: 0.5rem;
    font-size: 0.9rem;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
    margin: 0.25rem 0;
  }

  .draft input {
    width: 100%;
    box-sizing: border-box;
    padding: 0.3rem;
    border: 1px solid var(--border);
    border-radius: 0.3rem;
    background: var(--button-bg);
    color: var(--fg);
    font: inherit;
  }

  .hint {
    margin: 0.25rem 0;
    font-size: 0.85rem;
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

  .danger {
    color: var(--error-fg);
  }
</style>
