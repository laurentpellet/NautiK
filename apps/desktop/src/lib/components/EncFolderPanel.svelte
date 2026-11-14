<script lang="ts">
  // Paramètres → Cartes : dossier des cartes ENC S-57 (choisi par le
  // dialogue du système côté moteur) et inventaire de ses cellules. Un
  // dossier disparu est signalé, jamais oublié en silence. Un changement
  // de dossier est signalé (`onchange`) pour redessiner les ENC de la carte.
  import {
    formatIssueDate,
    usageLabel,
    type EncApi,
    type EncCell,
    type EncCellSummary,
    type EncFolder,
  } from '../services/enc';

  interface Props {
    api: EncApi;
    /** Dossier choisi, relu ou retiré par l'utilisateur. */
    onchange?: () => void;
  }

  let { api, onchange }: Props = $props();

  let folder = $state<EncFolder | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);

  /** Nombre maximal de cellules listées (l'inventaire complet reste compté). */
  const MAX_LISTED = 200;

  const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

  async function run(action: () => Promise<EncFolder | null>, notify = true): Promise<void> {
    busy = true;
    try {
      const updated = await action();
      if (updated) {
        folder = updated;
        reading = null;
        if (notify) onchange?.();
      }
      error = null;
    } catch (e) {
      error = message(e);
    } finally {
      busy = false;
    }
  }

  $effect(() => {
    void run(() => api.get(), false);
  });

  /** Cellule lue (décodée par le moteur) et son résumé. */
  let reading = $state<EncCell | null>(null);
  let summary = $state<EncCellSummary | null>(null);
  let readError = $state<string | null>(null);

  async function inspect(cell: EncCell): Promise<void> {
    reading = cell;
    summary = null;
    readError = null;
    busy = true;
    try {
      const result = await api.inspectCell(cell.path);
      if (reading?.path === cell.path) summary = result;
    } catch (e) {
      if (reading?.path === cell.path) readError = message(e);
    } finally {
      busy = false;
    }
  }

  /** Classes les plus nombreuses, pour le résumé. */
  const topClasses = (s: EncCellSummary) =>
    Object.entries(s.classes)
      .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
      .slice(0, 8);

  const scale = (n: number) => `1:${n.toLocaleString('fr-FR')}`;
  const deg = (n: number) => n.toFixed(3);

  const inventory = $derived(folder?.inventory ?? null);
  const withUpdates = $derived(inventory?.cells.filter((c) => c.updates > 0).length ?? 0);
  const plural = (n: number, word: string) => `${n} ${word}${n > 1 ? 's' : ''}`;
</script>

<section class="panel" aria-label="Cartes ENC" data-testid="enc-folder">
  <h3>CARTES ENC (S-57)</h3>
  {#if folder === null}
    <p class="muted">{error ? '' : 'Lecture du dossier ENC…'}</p>
  {:else if folder.dir === null}
    <p class="muted" data-testid="enc-dir">Aucun dossier ENC choisi.</p>
  {:else}
    <p>
      Dossier : <code class="path" data-testid="enc-dir">{folder.dir}</code>
    </p>
    {#if folder.error}
      <p class="error" role="alert">Dossier ENC inaccessible : {folder.error}</p>
    {:else if inventory}
      <p class="muted" data-testid="enc-count">
        {inventory.cells.length === 0
          ? 'Aucune cellule ENC (.000) dans ce dossier.'
          : `${plural(inventory.cells.length, 'cellule')} trouvée${inventory.cells.length > 1 ? 's' : ''}` +
            (withUpdates > 0 ? `, dont ${withUpdates} avec mises à jour` : '') +
            '.'}
        {inventory.catalogs > 0
          ? 'Catalogue CATALOG.031 présent.'
          : 'Pas de catalogue CATALOG.031.'}
      </p>
      {#if inventory.cm93}
        <p class="warn" data-testid="enc-cm93">
          Base CM93 trouvée{inventory.cm93 === '.' ? '' : ` (${inventory.cm93})`} : affichée sous les
          ENC officielles. Cartes non officielles et anciennes, à recouper.
        </p>
      {/if}
      {#if inventory.truncated}
        <p class="warn" role="status">
          Inventaire incomplet : dossier trop vaste ou trop profond. Choisissez le dossier ENC_ROOT.
        </p>
      {/if}
      {#if inventory.unreadableDirs > 0}
        <p class="warn" role="status">
          {plural(inventory.unreadableDirs, 'sous-dossier')} illisible{inventory.unreadableDirs > 1
            ? 's'
            : ''}.
        </p>
      {/if}
      {#if inventory.cells.length > 0}
        <details>
          <summary>Cellules</summary>
          <ul data-testid="enc-cells">
            {#each inventory.cells.slice(0, MAX_LISTED) as cell (cell.path)}
              <li title={cell.path}>
                {cell.name}
                <span class="muted">
                  {(cell.sizeBytes / 1024).toFixed(0)} Kio{cell.updates > 0
                    ? ` — ${plural(cell.updates, 'mise')} à jour`
                    : ''}
                </span>
                <button
                  type="button"
                  class="small"
                  disabled={busy}
                  aria-label={`Lire ${cell.name}`}
                  onclick={() => inspect(cell)}>Lire</button
                >
              </li>
            {/each}
          </ul>
          {#if inventory.cells.length > MAX_LISTED}
            <p class="muted">… et {inventory.cells.length - MAX_LISTED} autres.</p>
          {/if}
        </details>
      {/if}
      {#if reading}
        <div class="summary" data-testid="enc-summary" aria-live="polite">
          {#if summary}
            {#if summary.cancelled}
              <p class="error" role="alert" data-testid="enc-cancelled">
                Cellule annulée par son producteur le {formatIssueDate(summary.cancelled)} : ne plus l'utiliser
                pour la navigation.
              </p>
            {/if}
            <p>
              <strong>{summary.datasetName}</strong> — édition {summary.edition}, mise à jour {summary.updateNumber},
              publiée le {formatIssueDate(summary.issueDate)}
            </p>
            <p class="muted">
              Usage : {usageLabel(summary.intendedUsage)} · échelle {scale(
                summary.compilationScale,
              )} ·
              {plural(summary.features, 'objet')} · {plural(summary.soundings, 'sonde')}
            </p>
            {#if summary.bounds}
              <p class="muted">
                Emprise : {deg(summary.bounds[1])}° à {deg(summary.bounds[3])}° de latitude,
                {deg(summary.bounds[0])}° à {deg(summary.bounds[2])}° de longitude
              </p>
            {/if}
            <p class="muted">
              {topClasses(summary)
                .map(([name, n]) => `${name} ${n}`)
                .join(' · ')}
            </p>
            {#if summary.updatesApplied > 0}
              <p class="muted" data-testid="enc-updates">
                {plural(summary.updatesApplied, 'mise')} à jour appliquée{summary.updatesApplied > 1
                  ? 's'
                  : ''}{summary.updateIssueDate
                  ? `, la dernière publiée le ${formatIssueDate(summary.updateIssueDate)}`
                  : ''}.
              </p>
            {/if}
            {#if summary.updateError}
              <p class="warn" role="status">
                Cellule pas à jour : {plural(summary.updatesPending, 'mise')} à jour non appliquée{summary.updatesPending >
                1
                  ? 's'
                  : ''} ({summary.updateError}).
              </p>
            {/if}
            {#if summary.skippedFeatures > 0}
              <p class="warn" role="status">
                {plural(summary.skippedFeatures, 'objet')} écarté{summary.skippedFeatures > 1
                  ? 's'
                  : ''} (géométrie incohérente).
              </p>
              {#if summary.problems.length > 0}
                <details>
                  <summary>Détail</summary>
                  <ul>
                    {#each summary.problems as problem, i (i)}
                      <li>{problem}</li>
                    {/each}
                  </ul>
                </details>
              {/if}
            {/if}
          {:else if readError}
            <p class="error" role="alert">{reading.name} illisible : {readError}</p>
          {:else}
            <p class="muted">Lecture de {reading.name}…</p>
          {/if}
        </div>
      {/if}
    {/if}
  {/if}
  <div class="row">
    <button type="button" disabled={busy} onclick={() => run(() => api.chooseDir())}>
      Choisir le dossier ENC…
    </button>
    {#if folder?.dir}
      <button type="button" disabled={busy} onclick={() => run(() => api.get())}>
        Relire le dossier
      </button>
      <button type="button" disabled={busy} onclick={() => run(() => api.clearDir())}>
        Retirer
      </button>
    {/if}
  </div>
  <p class="hint">
    Dossier d'un jeu d'échange S-57 (en général <code>ENC_ROOT</code>, avec
    <code>CATALOG.031</code> et les cellules <code>.000</code>). Pour l'instant, NautiK repère et
    lit les cellules (WGS 84 uniquement) avec leurs mises à jour ; leur affichage sur la carte
    viendra ensuite. Les fichiers ne sont jamais modifiés.
  </p>
  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}
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

  .path {
    word-break: break-all;
  }

  .row {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  ul {
    max-height: 14rem;
    margin: 0.3rem 0 0;
    padding-left: 1.1rem;
    overflow: auto;
    font-size: 0.85rem;
  }

  summary {
    cursor: pointer;
    font-size: 0.85rem;
  }

  .muted,
  .hint {
    color: var(--muted);
    font-size: 0.85rem;
  }

  .warn {
    color: var(--warn-fg);
    font-size: 0.85rem;
    font-weight: 600;
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

  button.small {
    margin-left: 0.4rem;
    padding: 0 0.4rem;
    font-size: 0.8rem;
  }

  .summary {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    padding: 0.5rem;
    border: 1px solid var(--border);
    border-radius: 0.3rem;
  }

  button:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
