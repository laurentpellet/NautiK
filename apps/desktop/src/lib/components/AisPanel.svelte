<script lang="ts">
  import { formatAge, formatBearing, formatSpeed } from '../format';
  import {
    navigationStatusLabel,
    targetLabel,
    type AisSnapshot,
    type AisTarget,
    type TargetState,
  } from '../services/ais';

  interface Props {
    snapshot: AisSnapshot | null;
    /** Le moteur ne diffuse plus : les états affichés sont douteux. */
    silent: boolean;
    /** Cible sélectionnée (clic sur la carte). */
    selected?: string | null;
    onselect?: (mmsi: string) => void;
  }

  let { snapshot, silent, selected = null, onselect }: Props = $props();

  /** Nombre maximal de lignes affichées (les autres sont comptées). */
  const MAX_ROWS = 50;

  const STATE_ORDER: Record<TargetState, number> = { active: 0, stale: 1, lost: 2 };
  const STATE_LABEL: Record<TargetState, string> = {
    active: '',
    stale: 'OBSOLÈTE',
    lost: 'PERDUE',
  };

  const targets = $derived(
    [...(snapshot?.targets ?? [])].sort(
      (a, b) =>
        STATE_ORDER[a.state] - STATE_ORDER[b.state] || targetLabel(a).localeCompare(targetLabel(b)),
    ),
  );
  const shown = $derived(targets.slice(0, MAX_ROWS));
  const lost = $derived(targets.filter((t) => t.state === 'lost').length);

  function details(t: AisTarget): string {
    const parts = [`MMSI ${t.mmsi}`, `classe ${t.class}`];
    if (t.callsign) parts.push(t.callsign);
    const status = navigationStatusLabel(t.navigationStatus);
    if (status) parts.push(status);
    parts.push(
      t.positionAgeMs === null
        ? 'aucune position valide'
        : `position il y a ${formatAge(t.positionAgeMs)}`,
    );
    return parts.join(' — ');
  }
</script>

<section class="ais" aria-label="Cibles AIS">
  <h2>
    AIS
    <span class="count" data-testid="ais-count">
      {targets.length} cible{targets.length > 1 ? 's' : ''}{#if lost > 0}, {lost} perdue{lost > 1
          ? 's'
          : ''}{/if}
    </span>
  </h2>
  {#if snapshot?.simulated && targets.length > 0}
    <p class="badge" role="status" data-testid="ais-simulated">
      SIMULATION — cibles fictives ou rejouées
    </p>
  {/if}
  {#if silent && targets.length > 0}
    <p class="badge" role="alert">Moteur muet : état des cibles inconnu</p>
  {/if}
  {#if targets.length === 0}
    <p class="muted">Aucune cible AIS reçue.</p>
  {:else}
    <ul data-testid="ais-targets">
      {#each shown as t (t.mmsi)}
        <li
          class={t.state}
          class:selected={t.mmsi === selected}
          data-mmsi={t.mmsi}
          title={details(t)}
        >
          <button type="button" onclick={() => onselect?.(t.mmsi)}>
            <span class="name">
              <span class="class class-{t.class}">{t.class}</span>
              {targetLabel(t)}
            </span>
            <span class="kinematics">
              {formatSpeed(t.sogMps)} · {formatBearing(t.cogRad)}
            </span>
            {#if STATE_LABEL[t.state]}
              <span class="state" data-testid="ais-state">{STATE_LABEL[t.state]}</span>
            {/if}
          </button>
        </li>
      {/each}
    </ul>
    {#if targets.length > shown.length}
      <p class="muted">… et {targets.length - shown.length} autres cibles.</p>
    {/if}
  {/if}
</section>

<style>
  h2 {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    margin: 0.75rem 0 0.25rem;
    font-size: 1rem;
  }

  .count {
    color: var(--muted);
    font-size: 0.75rem;
    font-weight: 400;
  }

  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    max-height: 14rem;
    overflow-y: auto;
  }

  li button {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 0 0.5rem;
    width: 100%;
    padding: 0.2rem 0.3rem;
    border: 0;
    border-radius: 0.25rem;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  li.selected button {
    background: var(--accent-bg, rgba(0, 137, 123, 0.18));
  }

  li.stale button {
    opacity: 0.6;
  }

  li.lost button {
    opacity: 0.45;
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.85rem;
  }

  .class {
    display: inline-block;
    min-width: 1.1rem;
    margin-right: 0.2rem;
    border-radius: 0.2rem;
    color: #000;
    font-size: 0.7rem;
    font-weight: 700;
    text-align: center;
  }

  .class-A {
    background: #00897b;
    color: #fff;
  }

  .class-B {
    background: #f9a825;
  }

  .kinematics {
    font:
      600 0.8rem ui-monospace,
      'Cascadia Mono',
      Consolas,
      monospace;
    white-space: nowrap;
  }

  .state {
    grid-column: 1 / -1;
    color: var(--alert-fg);
    font-size: 0.7rem;
    font-weight: 700;
    letter-spacing: 0.05em;
  }

  .badge {
    margin: 0.25rem 0;
    padding: 0.15rem 0.4rem;
    border-radius: 0.25rem;
    background: #ff9800;
    color: #000;
    font-size: 0.75rem;
    font-weight: 700;
  }

  .muted {
    color: var(--muted);
    font-size: 0.8rem;
  }
</style>
