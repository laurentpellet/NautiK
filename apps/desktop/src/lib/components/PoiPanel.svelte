<script lang="ts">
  // Paramètres → Cartes : îles et amers nautiques (OpenStreetMap). Une
  // région n'est téléchargée qu'à la demande ; elle s'affiche ensuite sur
  // la carte, hors ligne. Les données ne sont pas officielles.
  import { POI_ATTRIBUTION, type PoiApi, type PoiRegion } from '../services/poi';

  interface Props {
    api: PoiApi;
    /** Points enregistrés modifiés (téléchargement, suppression). */
    onchange?: () => void;
  }

  let { api, onchange }: Props = $props();

  let regions = $state<PoiRegion[]>([]);
  /** Région en cours de téléchargement ou de suppression. */
  let working = $state<string | null>(null);
  let error = $state<string | null>(null);
  let notice = $state<string | null>(null);

  const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

  const dateFormat = new Intl.DateTimeFormat('fr-FR', {
    dateStyle: 'short',
    timeStyle: 'short',
    timeZone: 'UTC',
  });

  $effect(() => {
    api.listRegions().then(
      (r) => (regions = r),
      (e: unknown) => (error = message(e)),
    );
  });

  async function act(region: PoiRegion, action: 'download' | 'remove'): Promise<void> {
    working = region.id;
    error = null;
    notice = null;
    try {
      regions = action === 'download' ? await api.download(region.id) : await api.remove(region.id);
      const updated = regions.find((r) => r.id === region.id);
      notice =
        action === 'download'
          ? `${region.name} : ${updated?.poiCount ?? 0} îles et amers enregistrés`
          : `${region.name} : données supprimées`;
      onchange?.();
    } catch (e) {
      // Les données déjà enregistrées restent intactes.
      error = `${region.name} : ${message(e)}`;
    } finally {
      working = null;
    }
  }

  function confirmRemove(region: PoiRegion) {
    if (confirm(`Supprimer les îles et amers de « ${region.name} » ?`)) void act(region, 'remove');
  }
</script>

<section class="panel" aria-label="Îles et amers">
  <h3>ÎLES ET AMERS (OPENSTREETMAP)</h3>
  <p class="hint">
    Noms des îles, passes, balises, bouées, feux, mouillages et ports du Pacifique, téléchargés à la
    demande depuis OpenStreetMap (service Overpass) puis utilisés hors ligne. Données
    collaboratives, non officielles : elles complètent les cartes marines sans les remplacer.
  </p>
  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}
  {#if notice}
    <p class="notice" role="status">{notice}</p>
  {/if}
  {#if working}
    <p class="hint" role="status">Téléchargement en cours (jusqu'à quelques minutes)…</p>
  {/if}
  <ul>
    {#each regions as region (region.id)}
      <li data-region={region.id}>
        <span class="name">{region.name}</span>
        <span class="muted">
          {#if region.fetchedAt === null}
            non téléchargée
          {:else}
            {region.poiCount} éléments — {dateFormat.format(region.fetchedAt)} UTC
          {/if}
        </span>
        <span class="actions">
          <button
            type="button"
            disabled={working !== null}
            aria-label={`${region.fetchedAt === null ? 'Télécharger' : 'Mettre à jour'} ${region.name}`}
            onclick={() => act(region, 'download')}
            >{region.fetchedAt === null ? 'Télécharger' : 'Mettre à jour'}</button
          >
          {#if region.fetchedAt !== null}
            <button
              type="button"
              class="danger"
              disabled={working !== null}
              aria-label={`Supprimer ${region.name}`}
              onclick={() => confirmRemove(region)}>✕</button
            >
          {/if}
        </span>
      </li>
    {/each}
  </ul>
  <p class="muted">{POI_ATTRIBUTION}.</p>
</section>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    margin-top: 0.75rem;
    padding-top: 0.75rem;
    border-top: 1px solid var(--border);
  }

  h3 {
    margin: 0;
    color: var(--muted);
    font-size: 0.85rem;
    letter-spacing: 0.12em;
  }

  p {
    margin: 0;
  }

  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.3rem 0.6rem;
    padding: 0.3rem 0;
    border-top: 1px solid var(--border);
    font-size: 0.9rem;
  }

  .name {
    min-width: 11rem;
    font-weight: 600;
  }

  .actions {
    display: flex;
    gap: 0.3rem;
    margin-left: auto;
  }

  .muted,
  .hint {
    color: var(--muted);
    font-size: 0.85rem;
  }

  .notice {
    font-size: 0.85rem;
  }

  .error {
    color: var(--error-fg);
    font-weight: 600;
    font-size: 0.85rem;
  }

  button {
    padding: 0.25rem 0.6rem;
    border: 1px solid var(--border);
    border-radius: 0.3rem;
    background: var(--button-bg);
    color: var(--fg);
    font: inherit;
    font-size: 0.85rem;
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
