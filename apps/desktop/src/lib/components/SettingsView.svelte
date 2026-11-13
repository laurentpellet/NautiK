<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { SimulatorApi } from '../services/simulator';
  import {
    DEPTH_UNITS,
    DISTANCE_UNITS,
    SPEED_UNITS,
    setUnits,
    speedIn,
    speedToMps,
    units,
    type DepthUnit,
    type DistanceUnit,
    type SpeedUnit,
  } from '../units.svelte';

  interface Props {
    simulator: SimulatorApi;
    /** Alertes de sûreté, toujours visibles. */
    status: Snippet;
    /** Réglages des sources (panneau existant). */
    sources: Snippet;
  }

  let { simulator, status, sources }: Props = $props();

  let speedText = $state('');
  let speedError = $state<string | null>(null);
  let speedSaved = $state(false);
  let busy = $state(false);

  const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

  function showSpeed(mps: number): void {
    speedText = speedIn(mps).toFixed(1);
  }

  $effect(() => {
    simulator
      .get()
      .then((s) => showSpeed(s.speedMps))
      .catch((e: unknown) => (speedError = message(e)));
  });

  async function saveSpeed(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    speedSaved = false;
    const value = Number(speedText.replace(',', '.'));
    if (speedText.trim() === '' || !Number.isFinite(value) || value < 0) {
      speedError = 'Vitesse invalide.';
      return;
    }
    busy = true;
    try {
      const saved = await simulator.setSpeed(speedToMps(value));
      showSpeed(saved.speedMps);
      speedError = null;
      speedSaved = true;
    } catch (e) {
      speedError = message(e);
    } finally {
      busy = false;
    }
  }

  function changeSpeedUnit(unit: SpeedUnit): void {
    const value = Number(speedText.replace(',', '.'));
    const mps = Number.isFinite(value) ? speedToMps(value) : null;
    setUnits({ speed: unit });
    if (mps !== null) showSpeed(mps);
  }

  const options = <T extends string>(table: Record<T, { label: string }>) =>
    (Object.keys(table) as T[]).map((key) => ({ key, label: table[key].label }));
</script>

<section class="settings-view" aria-label="Paramètres">
  {@render status()}

  <div class="columns">
    <section class="card" aria-labelledby="units-title">
      <h2 id="units-title">Unités</h2>
      <label>
        Vitesse
        <select
          value={units.speed}
          onchange={(e) => changeSpeedUnit(e.currentTarget.value as SpeedUnit)}
        >
          {#each options(SPEED_UNITS) as o (o.key)}<option value={o.key}>{o.label}</option>{/each}
        </select>
      </label>
      <label>
        Distance
        <select
          value={units.distance}
          onchange={(e) => setUnits({ distance: e.currentTarget.value as DistanceUnit })}
        >
          {#each options(DISTANCE_UNITS) as o (o.key)}<option value={o.key}>{o.label}</option
            >{/each}
        </select>
      </label>
      <label>
        Profondeur
        <select
          value={units.depth}
          onchange={(e) => setUnits({ depth: e.currentTarget.value as DepthUnit })}
        >
          {#each options(DEPTH_UNITS) as o (o.key)}<option value={o.key}>{o.label}</option>{/each}
        </select>
      </label>
      <p class="hint">Le moteur calcule toujours en unités SI ; seul l'affichage change.</p>
    </section>

    <section class="card" aria-labelledby="sim-title">
      <h2 id="sim-title">Simulateur</h2>
      <form onsubmit={saveSpeed}>
        <label>
          Vitesse du bateau simulé ({SPEED_UNITS[units.speed].label})
          <input
            type="text"
            inputmode="decimal"
            bind:value={speedText}
            oninput={() => (speedSaved = false)}
          />
        </label>
        <button type="submit" disabled={busy}>Enregistrer</button>
      </form>
      <p class="hint">Vitesse suivie sur la route active en mode simulateur (0 à 30 nœuds).</p>
      {#if speedError}
        <p class="error" role="alert">{speedError}</p>
      {:else if speedSaved}
        <p class="ok" role="status">Vitesse enregistrée.</p>
      {/if}
    </section>

    <section class="card sources" aria-label="Sources">
      {@render sources()}
    </section>
  </div>
</section>

<style>
  .settings-view {
    position: absolute;
    inset: 0;
    z-index: 3;
    overflow: auto;
    padding: 1rem 1rem 2.5rem;
    background: var(--bg);
    color: var(--fg);
  }

  .columns {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(20rem, 1fr));
    gap: 0.75rem;
    margin-top: 0.75rem;
    align-items: start;
  }

  .card {
    padding: 0.75rem 1rem;
    border: 1px solid var(--border);
    border-radius: 0.5rem;
    background: var(--panel-bg);
  }

  h2 {
    margin: 0 0 0.5rem;
    font-size: 1.1rem;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    margin-bottom: 0.6rem;
    font-size: 0.9rem;
  }

  select,
  input,
  button {
    padding: 0.3rem 0.5rem;
    border: 1px solid var(--border);
    border-radius: 0.3rem;
    background: var(--button-bg);
    color: var(--fg);
    font: inherit;
  }

  button {
    cursor: pointer;
  }

  .hint {
    color: var(--muted);
    font-size: 0.8rem;
  }

  .error {
    color: var(--error-fg);
    font-weight: 600;
    font-size: 0.85rem;
  }

  .ok {
    font-size: 0.85rem;
  }
</style>
