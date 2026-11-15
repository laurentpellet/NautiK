<script lang="ts">
  // Routes : liste, édition sur la carte (départ, arrivée, points
  // intermédiaires, noms, déplacement), plan à la vitesse prévue
  // (longueur, durée, heures de passage, calculés par le moteur) et
  // cadrage de la carte sur la route.
  import { formatBearing, formatDistance, formatDuration, formatUtcClock } from '../format';
  import { draftPointName } from '../map/overlays';
  import type { InsertMode, NavigationController } from '../navigation.svelte';
  import { SPEED_UNITS, speedIn, speedToMps, units } from '../units.svelte';
  import type { PlannedLeg } from '../services/routing';

  interface Props {
    nav: NavigationController;
  }

  let { nav }: Props = $props();

  const editor = $derived(nav.editor);
  const plan = $derived(nav.plan);
  /** Noms des points du parcours planifié (édition ou route sélectionnée). */
  const pointNames = $derived(
    editor
      ? editor.points.map((p, i) => draftPointName(p.name, i))
      : (nav.selectedRoute?.waypoints.map((w) => w.name) ?? []),
  );

  // Plan recalculé par le moteur à chaque changement (glisser compris),
  // après une courte pause.
  $effect(() => {
    void nav.planPositions;
    void nav.speedMps;
    void nav.departure;
    const timer = setTimeout(() => void nav.refreshPlan(), 150);
    return () => clearTimeout(timer);
  });

  const speedValue = $derived(Math.round(speedIn(nav.speedMps) * 10) / 10);

  function onSpeed(event: Event) {
    const value = Number((event.currentTarget as HTMLInputElement).value);
    if (Number.isFinite(value) && value > 0) nav.setSpeed(speedToMps(value));
  }

  /** Départ saisi en UTC (`AAAA-MM-JJTHH:MM`). */
  const departureValue = $derived(
    nav.departure === null ? '' : new Date(nav.departure).toISOString().slice(0, 16),
  );

  function onDeparture(event: Event) {
    const value = (event.currentTarget as HTMLInputElement).value;
    nav.setDeparture(value === '' ? null : Date.parse(`${value}:00Z`));
  }

  function departNow() {
    nav.setDeparture(Math.ceil(Date.now() / 60_000) * 60_000);
  }

  async function save(event: SubmitEvent) {
    event.preventDefault();
    await nav.saveEdit();
  }

  function confirmDelete(id: string, routeName: string) {
    if (confirm(`Supprimer la route « ${routeName} » ?`)) void nav.deleteRoute(id);
  }

  function hint(mode: InsertMode) {
    if (mode === 'start') return 'Cliquez sur la carte pour placer le nouveau départ.';
    if (mode === 'arrival') return 'Cliquez sur la carte pour placer la nouvelle arrivée.';
    if (typeof mode === 'number') return `Cliquez sur la carte pour insérer le point ${mode + 1}.`;
    return 'Cliquez sur la carte pour ajouter un point en fin de route ; glissez un point pour le déplacer.';
  }

  /**
   * Segments arrivant à chaque point de l'éditeur, indexés par point
   * d'arrivée (valeurs du moteur). Absents tant que le plan ne correspond
   * pas aux points édités (recalcul en cours, plan indisponible).
   */
  const editorLegs: ReadonlyMap<number, PlannedLeg> = $derived(
    editor && plan && plan.legs.length === editor.points.length - 1
      ? new Map(plan.legs.map((leg) => [leg.fromIndex + 1, leg]))
      : new Map(),
  );

  const roleLabel = (index: number, count: number) =>
    index === 0 ? 'D' : index === count - 1 && count > 1 ? 'A' : String(index + 1);
</script>

<section class="routes" aria-label="Routes">
  {#if editor}
    <form class="editor" onsubmit={save} aria-label="Édition de route">
      <input
        type="text"
        placeholder="Nom de la route"
        maxlength="100"
        required
        bind:value={editor.name}
        aria-label="Nom de la route"
      />
      <p class="hint" role="status">{hint(editor.insert)}</p>
      <div class="actions">
        <button
          type="button"
          aria-pressed={editor.insert === 'start'}
          onclick={() => nav.setInsert(editor.insert === 'start' ? 'end' : 'start')}
          >+ Départ</button
        >
        <button
          type="button"
          aria-pressed={editor.insert === 'arrival'}
          onclick={() => nav.setInsert(editor.insert === 'arrival' ? 'end' : 'arrival')}
          >+ Arrivée</button
        >
        <button
          type="button"
          disabled={!plan}
          title="Cadrer la carte sur la route"
          onclick={() => nav.zoomTo(plan?.bounds ?? null)}>Zoomer</button
        >
      </div>

      {#if editor.points.length === 0}
        <p class="muted">Aucun point.</p>
      {:else}
        <ol class="points" aria-label="Waypoints">
          {#each editor.points as point, index (index)}
            <li>
              {#if index > 0}
                {@const leg = editorLegs.get(index)}
                <span class="leg" data-testid="editor-leg">
                  {#if leg}
                    ↓ {formatDistance(leg.distanceM)} · {formatBearing(leg.bearingRad)}
                  {:else}
                    ↓ …
                  {/if}
                </span>
              {/if}
              <span
                class="role"
                class:start={index === 0}
                class:end={index > 0 && index === editor.points.length - 1}
                >{roleLabel(index, editor.points.length)}</span
              >
              <input
                type="text"
                maxlength="100"
                placeholder={draftPointName('', index)}
                value={point.name}
                oninput={(e) => nav.renamePoint(index, e.currentTarget.value)}
                aria-label={`Nom du point ${index + 1}`}
              />
              <button
                type="button"
                class="icon"
                aria-pressed={editor.insert === index + 1}
                title="Insérer un point après celui-ci (cliquer ensuite sur la carte)"
                aria-label={`Insérer après le point ${index + 1}`}
                onclick={() => nav.setInsert(editor.insert === index + 1 ? 'end' : index + 1)}
                >+</button
              >
              <button
                type="button"
                class="icon danger"
                title="Retirer ce point"
                aria-label={`Retirer le point ${index + 1}`}
                onclick={() => nav.removePoint(index)}>✕</button
              >
            </li>
          {/each}
        </ol>
      {/if}

      {@render planView()}

      <div class="actions">
        <button
          type="submit"
          disabled={editor.points.length < 2 || editor.name.trim() === '' || nav.busy}
          >Enregistrer</button
        >
        <button type="button" disabled={editor.points.length === 0} onclick={() => nav.undoPoint()}
          >Retirer le dernier</button
        >
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

    {#if nav.routes.length === 0}
      <p class="muted">Aucune route.</p>
    {:else}
      <ul>
        {#each nav.routes as route (route.id)}
          <li class:active={route.active} class:selected={route.id === nav.selectedRouteId}>
            <button
              type="button"
              class="title"
              aria-expanded={route.id === nav.selectedRouteId}
              title="Afficher le plan et cadrer la carte sur la route"
              onclick={() => nav.selectRoute(route.id === nav.selectedRouteId ? null : route.id)}
            >
              <span class="name">{route.name}</span>
              <span class="muted">
                {formatDistance(route.totalDistanceM)} — {route.waypoints.length} WP
              </span>
            </button>
            <div class="actions">
              {#if route.active}
                <button type="button" onclick={() => nav.deactivate()}>Arrêter</button>
              {:else}
                <button
                  type="button"
                  disabled={nav.busy || nav.activeRouteId !== null}
                  onclick={() => nav.activate(route.id)}>Naviguer</button
                >
                <button type="button" disabled={nav.busy} onclick={() => nav.editRoute(route.id)}>
                  Modifier
                </button>
                <button type="button" disabled={nav.busy} onclick={() => nav.reverse(route.id)}>
                  Inverser
                </button>
              {/if}
              <button type="button" onclick={() => nav.zoomTo(route.bounds)}>Zoomer</button>
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
            {#if route.id === nav.selectedRouteId}
              {@render planView()}
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
</section>

{#snippet planView()}
  <div class="plan" data-testid="route-plan">
    <div class="plan-settings">
      <label>
        Vitesse prévue
        <input
          type="number"
          min="0.2"
          max="99"
          step="0.1"
          value={speedValue}
          onchange={onSpeed}
          aria-label="Vitesse prévue"
        />
        <span class="muted">{SPEED_UNITS[units.speed].label}</span>
      </label>
      <label>
        Départ (UTC)
        <input
          type="datetime-local"
          value={departureValue}
          onchange={onDeparture}
          aria-label="Heure de départ (UTC)"
        />
      </label>
      <div class="actions">
        <button type="button" onclick={departNow}>Maintenant</button>
        {#if nav.departure !== null}
          <button type="button" onclick={() => nav.setDeparture(null)}>Sans heure</button>
        {/if}
      </div>
    </div>
    {#if nav.planError}
      <p class="error" role="alert">Plan indisponible : {nav.planError}</p>
    {:else if plan}
      <dl class="totals">
        <div>
          <dt>Longueur</dt>
          <dd data-testid="plan-length">{formatDistance(plan.totalDistanceM)}</dd>
        </div>
        <div>
          <dt>Durée</dt>
          <dd data-testid="plan-duration">{formatDuration(plan.totalDurationS * 1000)}</dd>
        </div>
        <div>
          <dt>Arrivée</dt>
          <dd>{plan.arrival === null ? '—' : `${formatUtcClock(plan.arrival)} UTC`}</dd>
        </div>
      </dl>
      <table>
        <thead>
          <tr>
            <th scope="col">Vers</th>
            <th scope="col">Dist.</th>
            <th scope="col">Cap</th>
            <th scope="col">Cumul</th>
            <th scope="col">{plan.departure === null ? 'Temps' : 'Heure'}</th>
          </tr>
        </thead>
        <tbody>
          {#each plan.legs as leg (leg.fromIndex)}
            <tr>
              <td class="to">{pointNames[leg.fromIndex + 1] ?? '—'}</td>
              <td>{formatDistance(leg.distanceM)}</td>
              <td>{formatBearing(leg.bearingRad)}</td>
              <td>{formatDistance(leg.cumulativeDistanceM)}</td>
              <td>
                {leg.arrival === null
                  ? formatDuration(leg.cumulativeDurationS * 1000)
                  : formatUtcClock(leg.arrival)}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
      <p class="muted">Vitesse constante, sans courant ni vent : estimation indicative.</p>
    {:else}
      <p class="muted">Au moins deux points pour calculer la durée.</p>
    {/if}
  </div>
{/snippet}

<style>
  ul,
  ol {
    margin: 0.25rem 0;
    padding: 0;
    list-style: none;
  }

  li {
    padding: 0.3rem 0;
    border-top: 1px solid var(--border);
  }

  li.active .name {
    color: #d81b60;
    font-weight: 700;
  }

  li.selected .name {
    text-decoration: underline;
  }

  .title {
    display: flex;
    justify-content: space-between;
    gap: 0.5rem;
    width: 100%;
    padding: 0.1rem 0;
    border: 0;
    background: none;
    font-size: 0.9rem;
    text-align: left;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
    margin: 0.25rem 0;
  }

  input[type='text'],
  input[type='number'],
  input[type='datetime-local'] {
    box-sizing: border-box;
    padding: 0.25rem;
    border: 1px solid var(--border);
    border-radius: 0.3rem;
    background: var(--button-bg);
    color: var(--fg);
    font: inherit;
    font-size: 0.85rem;
  }

  .editor > input {
    width: 100%;
  }

  .points li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.3rem;
    padding: 0.2rem 0;
  }

  /* Segment depuis le point précédent, au-dessus du point. */
  .leg {
    flex-basis: 100%;
    padding-left: 0.45rem;
    color: var(--muted);
    font-size: 0.8rem;
    font-variant-numeric: tabular-nums;
  }

  .points input {
    flex: 1;
    min-width: 0;
  }

  .role {
    flex: none;
    width: 1.6rem;
    height: 1.6rem;
    border: 2px solid #ff6f00;
    border-radius: 50%;
    font-size: 0.7rem;
    font-weight: 700;
    line-height: 1.35rem;
    text-align: center;
  }

  .role.start {
    background: #2e7d32;
    color: #fff;
  }

  .role.end {
    background: #c62828;
    color: #fff;
  }

  .hint {
    margin: 0.25rem 0;
    font-size: 0.85rem;
  }

  .plan {
    margin-top: 0.4rem;
    padding: 0.4rem;
    border: 1px solid var(--border);
    border-radius: 0.3rem;
  }

  .plan-settings {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem 0.8rem;
    font-size: 0.8rem;
  }

  .plan-settings label {
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }

  .plan-settings input[type='number'] {
    width: 4.5rem;
  }

  .totals {
    display: flex;
    gap: 1rem;
    margin: 0.4rem 0;
  }

  .totals dt {
    color: var(--muted);
    font-size: 0.65rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .totals dd {
    margin: 0;
    font-size: 1rem;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.78rem;
    font-variant-numeric: tabular-nums;
  }

  th {
    color: var(--muted);
    font-weight: 600;
    text-align: left;
  }

  td,
  th {
    padding: 0.1rem 0.25rem;
  }

  td.to {
    overflow: hidden;
    max-width: 7rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .muted {
    color: var(--muted);
    font-size: 0.8rem;
  }

  .error {
    color: var(--error-fg);
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

  button.title {
    border: 0;
    background: none;
  }

  button[aria-pressed='true'] {
    border-color: #ff6f00;
    box-shadow: inset 0 0 0 1px #ff6f00;
  }

  button.icon {
    flex: none;
    width: 1.7rem;
    padding: 0.2rem 0;
  }

  button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .danger {
    color: var(--error-fg);
  }
</style>
