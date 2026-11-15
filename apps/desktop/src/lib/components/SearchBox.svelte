<script lang="ts">
  // Recherche d'un lieu (moteur) : coordonnées saisies, marques, waypoints,
  // îles et amers. Un choix centre la carte sur le lieu.
  import { formatLatitude, formatLongitude } from '../format';
  import { MAX_QUERY_CHARS, PLACE_KIND_LABELS, searchPlaces, type Place } from '../services/search';
  import type { InvokeFn } from '../services/engine';

  interface Props {
    onselect: (place: Place) => void;
    /** Commande du moteur (remplaçable dans les tests). */
    call?: InvokeFn;
  }

  let { onselect, call }: Props = $props();

  let query = $state('');
  let results = $state.raw<Place[]>([]);
  let error = $state<string | null>(null);
  let searched = $state(false);
  let seq = 0;
  let input: HTMLInputElement;

  $effect(() => {
    input.focus();
  });

  async function search(event: SubmitEvent) {
    event.preventDefault();
    const mine = ++seq;
    error = null;
    try {
      const found = await (call ? searchPlaces(query, call) : searchPlaces(query));
      if (mine !== seq) return;
      results = found;
      searched = true;
    } catch (e) {
      if (mine === seq) error = e instanceof Error ? e.message : String(e);
    }
  }
</script>

<div class="search" data-testid="search-box">
  <form onsubmit={search}>
    <input
      bind:this={input}
      type="search"
      maxlength={MAX_QUERY_CHARS}
      placeholder="Nom, ou 17°32,1'S 149°34,2'W"
      aria-label="Rechercher un lieu"
      bind:value={query}
    />
    <button type="submit">Chercher</button>
  </form>
  {#if error}
    <p class="error" role="alert">{error}</p>
  {:else if searched && results.length === 0}
    <p class="muted">Aucun résultat.</p>
  {/if}
  {#if results.length > 0}
    <ul role="listbox" aria-label="Résultats">
      {#each results as place, i (i)}
        <li>
          <button type="button" role="option" aria-selected="false" onclick={() => onselect(place)}>
            <span class="name">{place.name}</span>
            <span class="meta">
              {PLACE_KIND_LABELS[place.kind]}{place.detail ? ` — ${place.detail}` : ''}
            </span>
            <span class="meta coords">
              {formatLatitude(place.position.latitudeDeg)}
              {formatLongitude(place.position.longitudeDeg)}
            </span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .search {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    width: min(22rem, 80vw);
    font-size: 0.85rem;
  }

  form {
    display: flex;
    gap: 0.3rem;
  }

  input {
    flex: 1;
    min-width: 0;
  }

  ul {
    max-height: 18rem;
    margin: 0;
    padding: 0;
    overflow-y: auto;
    list-style: none;
  }

  li button {
    display: flex;
    flex-direction: column;
    width: 100%;
    padding: 0.3rem 0.4rem;
    border: 0;
    border-radius: 0.3rem;
    background: transparent;
    color: var(--fg);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  li button:hover,
  li button:focus-visible {
    background: var(--button-bg);
  }

  .name {
    font-weight: 600;
  }

  .meta {
    color: var(--muted);
    font-size: 0.75rem;
  }

  .coords {
    font-variant-numeric: tabular-nums;
  }

  .muted {
    margin: 0;
    color: var(--muted);
  }

  .error {
    margin: 0;
    color: var(--danger, #d32f2f);
  }
</style>
