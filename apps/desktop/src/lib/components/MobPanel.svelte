<script lang="ts">
  // Homme à la mer : grand affichage du relèvement et de la distance au
  // point MOB, calculés par le moteur, lisible de loin. Visible dans tous
  // les onglets tant que l'alerte n'est pas terminée explicitement. Une
  // position obsolète ou un point approximatif sont toujours signalés.
  import { formatBearing, formatDistance, formatDuration } from '../format';
  import type { AlarmState, MobView } from '../services/alarms';

  interface Props {
    mob: MobView;
    /** État de l'alarme MOB (acquittement possible si active). */
    alarmState: AlarmState;
    now: number;
    /** Position du bateau obsolète ou flux muet. */
    stale: boolean;
    onacknowledge: () => void;
    /** Fin de l'alerte (après confirmation). */
    onend: () => Promise<void>;
  }

  let { mob, alarmState, now, stale, onacknowledge, onend }: Props = $props();

  /** Fin demandée : un second appui confirme. */
  let confirming = $state(false);
  let error = $state<string | null>(null);

  /** Temps écoulé à la seconde (« 3 min 07 s »), en heures au-delà. */
  const elapsed = $derived.by(() => {
    const ms = Math.max(0, now - mob.markedAt);
    if (ms >= 3_600_000) return formatDuration(ms);
    const s = Math.floor(ms / 1000);
    return `${Math.floor(s / 60)} min ${String(s % 60).padStart(2, '0')} s`;
  });
  const markedAt = $derived(
    new Date(mob.markedAt).toLocaleTimeString('fr-FR', {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    }),
  );
  const unknown = $derived(stale || mob.distanceM === null);

  async function end(): Promise<void> {
    if (!confirming) {
      confirming = true;
      return;
    }
    try {
      await onend();
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      confirming = false;
    }
  }
</script>

<section class="mob" role="alert" aria-label="Homme à la mer" data-testid="mob-panel">
  <header>
    <h2>HOMME À LA MER</h2>
    <span class="since" title={`Déclenché à ${markedAt}`}>depuis {elapsed}</span>
  </header>
  <div class="values" class:unknown>
    <div class="value">
      <span class="number" data-testid="mob-bearing"
        >{unknown ? '—' : formatBearing(mob.bearingRad)}</span
      >
      <span class="label">Relèvement</span>
    </div>
    <div class="value">
      <span class="number" data-testid="mob-distance"
        >{unknown || mob.distanceM === null ? '—' : formatDistance(mob.distanceM)}</span
      >
      <span class="label">Distance</span>
    </div>
  </div>
  {#if unknown}
    <p class="warn" data-testid="mob-stale">
      Position du bateau obsolète ou absente : relèvement et distance inconnus.
    </p>
  {/if}
  {#if mob.approximate}
    <p class="warn">Point MOB approximatif : la position était obsolète au déclenchement.</p>
  {/if}
  <p class="hint">Point posé à {markedAt}. Noter l'heure, le cap et la vitesse.</p>
  <div class="actions">
    {#if alarmState === 'active'}
      <button type="button" onclick={onacknowledge}>Couper l'alarme</button>
    {/if}
    <button
      type="button"
      class="end"
      class:confirming
      onclick={end}
      onblur={() => (confirming = false)}
    >
      {confirming ? 'Confirmer la fin du MOB' : 'Fin du MOB'}
    </button>
  </div>
  {#if error}
    <p class="warn" role="alert">{error}</p>
  {/if}
</section>

<style>
  .mob {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    width: min(26rem, calc(100vw - 2rem));
    padding: 0.6rem 0.9rem 0.8rem;
    border: 3px solid #e53935;
    border-radius: 0.5rem;
    background: var(--panel-bg);
    color: var(--fg);
    box-shadow: 0 6px 24px rgba(0, 0, 0, 0.45);
  }

  header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 0.5rem;
  }

  h2 {
    margin: 0;
    color: #e53935;
    font-size: 1.2rem;
    letter-spacing: 0.06em;
  }

  .since {
    font-variant-numeric: tabular-nums;
    font-weight: 600;
  }

  .values {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.5rem;
  }

  .value {
    display: flex;
    flex-direction: column;
    align-items: center;
  }

  .number {
    font-size: clamp(2.2rem, 7vw, 3.4rem);
    font-weight: 800;
    font-variant-numeric: tabular-nums;
    line-height: 1.05;
  }

  .unknown .number {
    color: var(--muted);
  }

  .label {
    color: var(--muted);
    font-size: 0.8rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  p {
    margin: 0;
    font-size: 0.85rem;
  }

  .warn {
    color: var(--warn-fg);
    font-weight: 700;
  }

  .hint {
    color: var(--muted);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    justify-content: flex-end;
  }

  button {
    padding: 0.35rem 0.7rem;
    border: 1px solid var(--border);
    border-radius: 0.3rem;
    background: var(--button-bg);
    color: var(--fg);
    font: inherit;
    font-size: 0.9rem;
    cursor: pointer;
  }

  .end.confirming {
    border-color: #e53935;
    color: #e53935;
    font-weight: 700;
  }
</style>
