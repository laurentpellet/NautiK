<script lang="ts">
  // Paramètres → Alarmes : surveillance de la position (désactivation
  // explicite et confirmée) et historique des événements.
  import {
    EVENT_LABELS,
    type AlarmEvent,
    type AlarmsApi,
    type AlarmsSnapshot,
  } from '../services/alarms';
  import { formatUtc } from '../services/weather';

  interface Props {
    api: AlarmsApi;
    alarms: AlarmsSnapshot | null;
    onchange: (snapshot: AlarmsSnapshot) => void;
  }

  let { api, alarms, onchange }: Props = $props();

  /** Nombre d'événements affichés. */
  const EVENTS_SHOWN = 50;

  let events = $state.raw<AlarmEvent[]>([]);
  let error = $state<string | null>(null);
  let confirmDisable = $state(false);

  const position = $derived(alarms?.alarms.find((a) => a.kind === 'positionLost') ?? null);
  const enabled = $derived(position !== null && position.state !== 'off');

  const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

  async function refresh(): Promise<void> {
    try {
      events = await api.events(EVENTS_SHOWN);
      error = null;
    } catch (e) {
      error = `Historique illisible : ${message(e)}`;
    }
  }

  $effect(() => {
    void refresh();
  });

  async function setEnabled(on: boolean): Promise<void> {
    confirmDisable = false;
    try {
      onchange(await api.setEnabled('positionLost', on));
      await refresh();
    } catch (e) {
      error = message(e);
    }
  }
</script>

<section class="panel" aria-label="Alarmes" data-testid="alarms-panel">
  <h2>ALARMES</h2>
  <div class="row">
    <div>
      <strong>Perte de position</strong>
      <p class="muted">{position?.message ?? '—'}</p>
    </div>
    {#if enabled}
      {#if confirmDisable}
        <button type="button" class="danger" onclick={() => setEnabled(false)}>
          Confirmer la désactivation
        </button>
        <button type="button" onclick={() => (confirmDisable = false)}>Annuler</button>
      {:else}
        <button type="button" onclick={() => (confirmDisable = true)}>Désactiver</button>
      {/if}
    {:else}
      <span class="badge" role="status">DÉSACTIVÉE</span>
      <button type="button" class="primary" onclick={() => setEnabled(true)}>Activer</button>
    {/if}
  </div>
  <p class="muted">
    Le mouillage se surveille depuis l'onglet Carte (« Mouiller ici »). Les alarmes sont évaluées
    par le moteur, même lorsque l'interface est fermée sur un autre onglet.
  </p>

  <div class="history-head">
    <h3>Historique</h3>
    <button type="button" onclick={refresh}>Actualiser</button>
  </div>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if events.length === 0}
    <p class="muted" data-testid="no-alarm-events">Aucun événement.</p>
  {:else}
    <ul data-testid="alarm-events">
      {#each events as e (e.id)}
        <li class={e.kind}>
          <span class="time">{formatUtc(e.at)}</span>
          <span class="kind">{e.alarm === 'anchor' ? 'Mouillage' : 'Position'}</span>
          <span class="what">{EVENT_LABELS[e.kind]}</span>
          <span class="msg">{e.message}{e.simulated ? ' (simulation)' : ''}</span>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  h2,
  h3 {
    margin: 0;
    color: var(--muted);
    font-size: 0.9rem;
    letter-spacing: 0.12em;
  }

  h3 {
    font-size: 0.8rem;
  }

  p {
    margin: 0;
  }

  .row,
  .history-head {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    flex-wrap: wrap;
  }

  .row > div {
    flex: 1;
  }

  .muted {
    color: var(--muted);
    font-size: 0.85rem;
  }

  button {
    padding: 0.3rem 0.8rem;
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    background: var(--button-bg);
    color: var(--fg);
    font: inherit;
    font-size: 0.9rem;
    cursor: pointer;
  }

  .primary {
    background: var(--accent-bg);
    color: var(--accent-fg);
  }

  .danger {
    background: var(--alert-bg);
    color: var(--alert-fg);
  }

  .badge {
    padding: 0 0.4rem;
    border-radius: 0.3rem;
    background: var(--warn-bg);
    color: var(--warn-fg);
    font-size: 0.75rem;
    font-weight: 700;
  }

  ul {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    max-height: 18rem;
    margin: 0;
    padding: 0;
    overflow: auto;
    list-style: none;
    font-size: 0.85rem;
  }

  li {
    display: grid;
    grid-template-columns: 11rem 6rem 6rem 1fr;
    gap: 0.5rem;
    padding: 0.25rem 0.5rem;
    border-radius: 0.3rem;
    background: var(--inset-bg);
  }

  li.activated .what {
    color: var(--error-fg);
    font-weight: 700;
  }

  .time {
    font-variant-numeric: tabular-nums;
  }

  .error {
    color: var(--error-fg);
    font-weight: 600;
  }
</style>
