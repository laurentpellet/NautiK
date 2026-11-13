<script lang="ts">
  // Bandeau des alarmes déclenchées, visible dans tous les onglets et en
  // plein écran. Acquitter coupe le son ; l'alarme reste affichée tant
  // que le moteur ne l'a pas résolue.
  import type { AlarmKind, AlarmView } from '../services/alarms';

  interface Props {
    alarms: AlarmView[];
    simulated: boolean;
    /** Aucune sortie audio : l'alarme n'est que visuelle. */
    soundUnavailable: boolean;
    onacknowledge: (kind: AlarmKind) => void;
    error: string | null;
  }

  let { alarms, simulated, soundUnavailable, onacknowledge, error }: Props = $props();
</script>

{#if alarms.length > 0 || error}
  <div class="banner" data-testid="alarm-banner" role="alert">
    {#each alarms as a (a.kind)}
      <div class="alarm" class:active={a.state === 'active'} data-alarm={a.kind}>
        <strong>⚠ {a.label.toUpperCase()}</strong>
        <span>{a.message}</span>
        {#if simulated}<span class="tag">SIMULATION</span>{/if}
        {#if a.state === 'active'}
          <button type="button" onclick={() => onacknowledge(a.kind)}>Acquitter</button>
        {:else}
          <span class="tag">ACQUITTÉE</span>
        {/if}
      </div>
    {/each}
    {#if soundUnavailable && alarms.some((a) => a.state === 'active')}
      <p class="note">Son indisponible sur cet appareil : alarme visuelle seulement.</p>
    {/if}
    {#if error}<p class="note">{error}</p>{/if}
  </div>
{/if}

<style>
  .banner {
    position: absolute;
    top: var(--banner-top, 0.6rem);
    left: 50%;
    z-index: 6;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    width: min(44rem, calc(100% - 2rem));
    transform: translateX(-50%);
  }

  .alarm {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.6rem;
    padding: 0.55rem 0.9rem;
    border-radius: 0.6rem;
    background: var(--alert-bg);
    color: var(--alert-fg);
    box-shadow: 0 2px 12px rgba(0, 0, 0, 0.4);
    font-size: 0.95rem;
  }

  .alarm.active {
    animation: blink 1s steps(2, start) infinite;
  }

  @keyframes blink {
    50% {
      filter: brightness(1.35);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .alarm.active {
      animation: none;
    }
  }

  .alarm span:not(.tag) {
    flex: 1;
    min-width: 12rem;
  }

  .tag {
    padding: 0 0.4rem;
    border: 1px solid currentColor;
    border-radius: 0.3rem;
    font-size: 0.75rem;
    font-weight: 700;
  }

  button {
    padding: 0.3rem 0.9rem;
    border: 2px solid currentColor;
    border-radius: 0.4rem;
    background: transparent;
    color: inherit;
    font: inherit;
    font-weight: 700;
    cursor: pointer;
  }

  .note {
    margin: 0;
    padding: 0.3rem 0.8rem;
    border-radius: 0.4rem;
    background: var(--warn-bg);
    color: var(--warn-fg);
    font-size: 0.85rem;
  }
</style>
