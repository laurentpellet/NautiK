<script lang="ts">
  // Alertes de sûreté toujours visibles lorsque le panneau latéral est
  // masqué (onglet Instruments, plein écran) : moteur muet, position
  // absente ou obsolète, simulation, liaison en défaut.
  import { describeConnection, formatAge } from '../format';
  import type { ConnectionState, PositionStatus } from '../services/instruments';

  interface Props {
    position: PositionStatus | null;
    silent: boolean;
    /** `undefined` : état inconnu ; `null` : pas de liaison (simulateur). */
    connection?: ConnectionState | null;
    compact?: boolean;
  }

  let { position, silent, connection = null, compact = false }: Props = $props();

  const link = $derived(connection ? describeConnection(connection) : null);
  const source = $derived(position?.fix.source ?? null);
</script>

<div class="status" class:compact data-testid="instruments-status">
  {#if silent}
    <span class="badge alert" role="alert">MOTEUR MUET — VALEURS NON ACTUALISÉES</span>
  {/if}
  {#if link && link.level !== 'ok'}
    <span class="badge {link.level === 'alert' ? 'alert' : 'info'}" role="alert">{link.label}</span>
  {/if}
  {#if source === 'simulator'}
    <span class="badge sim" role="status">SIMULATION</span>
  {:else if source === 'replay'}
    <span class="badge sim" role="status">REJEU — DONNÉES NON ACTUELLES</span>
  {/if}
  {#if position?.stale}
    <span class="badge alert" role="alert">POSITION OBSOLÈTE ({formatAge(position.ageMs)})</span>
  {/if}
  {#if !position}
    <span class="badge alert" role="alert">AUCUNE POSITION</span>
  {/if}
</div>

<style>
  .status {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  .badge {
    padding: 0.2rem 0.6rem;
    border-radius: 0.3rem;
    font-size: 0.95rem;
    font-weight: 700;
    letter-spacing: 0.04em;
  }

  .compact .badge {
    font-size: 0.8rem;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.3);
  }

  .alert {
    background: var(--alert-bg);
    color: var(--alert-fg);
  }

  .info {
    background: var(--warn-bg);
    color: var(--warn-fg);
  }

  .sim {
    background: var(--sim-bg);
    color: var(--sim-fg);
  }
</style>
