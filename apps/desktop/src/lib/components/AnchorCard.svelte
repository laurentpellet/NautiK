<script lang="ts">
  // Carte « Mouillage » (onglet Carte) : poser l'ancre à la position
  // courante, suivre la distance, lever l'ancre (avec confirmation).
  // La surveillance est faite par le moteur, même onglet fermé.
  import {
    ANCHOR_DELAYS_S,
    DEFAULT_ANCHOR_DELAY_S,
    MAX_ANCHOR_RADIUS_M,
    MIN_ANCHOR_RADIUS_M,
    type AlarmsSnapshot,
  } from '../services/alarms';

  interface Props {
    alarms: AlarmsSnapshot | null;
    /** Une position fraîche est disponible. */
    canDrop: boolean;
    ondrop: (radiusM: number, delayS: number) => Promise<void>;
    onraise: () => Promise<void>;
  }

  let { alarms, canDrop, ondrop, onraise }: Props = $props();

  let radiusText = $state('');
  let delay = $state<number>(DEFAULT_ANCHOR_DELAY_S);
  let error = $state<string | null>(null);
  let busy = $state(false);
  let confirmRaise = $state(false);

  const anchor = $derived(alarms?.anchor ?? null);
  const view = $derived(alarms?.alarms.find((a) => a.kind === 'anchor') ?? null);
  const alerting = $derived(view?.state === 'active' || view?.state === 'acknowledged');

  // Rayon proposé : le dernier utilisé, tant que l'utilisateur n'a rien saisi.
  $effect(() => {
    if (radiusText === '' && alarms) radiusText = String(Math.round(alarms.anchorRadiusM));
  });

  const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

  async function submit(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    const radius = Number(radiusText.replace(',', '.'));
    if (!Number.isFinite(radius) || radius < MIN_ANCHOR_RADIUS_M || radius > MAX_ANCHOR_RADIUS_M) {
      error = `Rayon invalide : ${MIN_ANCHOR_RADIUS_M} à ${MAX_ANCHOR_RADIUS_M} m.`;
      return;
    }
    busy = true;
    try {
      await ondrop(radius, delay);
      error = null;
    } catch (e) {
      error = message(e);
    } finally {
      busy = false;
    }
  }

  async function raise(): Promise<void> {
    confirmRaise = false;
    busy = true;
    try {
      await onraise();
      error = null;
    } catch (e) {
      error = message(e);
    } finally {
      busy = false;
    }
  }
</script>

<section class="card" class:alerting data-testid="anchor-card" aria-label="Mouillage">
  <h2>MOUILLAGE</h2>
  {#if anchor}
    <p class="distance" data-testid="anchor-distance">
      {#if anchor.distanceM !== null}
        <strong>{Math.round(anchor.distanceM)} m</strong> / {Math.round(anchor.radiusM)} m
      {:else}
        <strong>— m</strong> / {Math.round(anchor.radiusM)} m
        <span class="warn">position inconnue</span>
      {/if}
    </p>
    {#if anchor.distanceM !== null}
      <meter
        min="0"
        max={anchor.radiusM}
        high={anchor.radiusM * 0.8}
        optimum="0"
        value={Math.min(anchor.distanceM, anchor.radiusM)}
        aria-label="Distance à l'ancre"
      ></meter>
    {/if}
    <p class="muted">{view?.message}</p>
    <p class="muted">
      Temporisation {anchor.delayS} s · retour sous {Math.round(
        anchor.radiusM - anchor.hysteresisM,
      )}
      m
    </p>
    {#if confirmRaise}
      <div class="row">
        <button type="button" class="danger" disabled={busy} onclick={raise}>
          Confirmer : lever l'ancre
        </button>
        <button type="button" onclick={() => (confirmRaise = false)}>Annuler</button>
      </div>
    {:else}
      <button type="button" disabled={busy} onclick={() => (confirmRaise = true)}>
        Lever l'ancre
      </button>
    {/if}
  {:else}
    <form onsubmit={submit}>
      <label>
        Rayon (m)
        <input
          type="text"
          inputmode="numeric"
          size="5"
          bind:value={radiusText}
          aria-label="Rayon d'évitage (m)"
        />
      </label>
      <label>
        Tempo.
        <select bind:value={delay} aria-label="Temporisation">
          {#each ANCHOR_DELAYS_S as d (d)}
            <option value={d}>{d} s</option>
          {/each}
        </select>
      </label>
      <button type="submit" class="primary" disabled={busy || !canDrop}>Mouiller ici</button>
    </form>
    {#if !canDrop}
      <p class="muted">Position récente requise pour poser l'ancre.</p>
    {/if}
  {/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
</section>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    width: 17rem;
    padding: 0.7rem 0.9rem;
    border: 1px solid var(--border);
    border-radius: 0.7rem;
    background: var(--card-bg);
    color: var(--fg);
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.3);
    font-size: 0.85rem;
  }

  .card.alerting {
    border: 2px solid var(--alert-bg);
  }

  h2 {
    margin: 0;
    color: var(--muted);
    font-size: 0.8rem;
    letter-spacing: 0.12em;
  }

  p {
    margin: 0;
  }

  .distance strong {
    font-size: 1.4rem;
    font-variant-numeric: tabular-nums;
  }

  meter {
    width: 100%;
  }

  .muted {
    color: var(--muted);
  }

  .warn {
    margin-left: 0.3rem;
    color: var(--warn-fg);
    font-weight: 600;
  }

  form,
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem;
  }

  label {
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }

  input,
  select,
  button {
    padding: 0.25rem 0.5rem;
    border: 1px solid var(--border);
    border-radius: 0.35rem;
    background: var(--button-bg);
    color: var(--fg);
    font: inherit;
  }

  button {
    cursor: pointer;
  }

  button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .primary {
    background: var(--accent-bg);
    color: var(--accent-fg);
  }

  .danger {
    background: var(--alert-bg);
    color: var(--alert-fg);
  }

  .error {
    color: var(--error-fg);
    font-weight: 600;
  }
</style>
