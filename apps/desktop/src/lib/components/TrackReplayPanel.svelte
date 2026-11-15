<script lang="ts">
  // Relecture d'une trace : statistiques (moteur), curseur, lecture
  // accélérée et valeurs enregistrées au point courant. La trace est
  // colorée selon la vitesse sur la carte.
  import { formatDepth, formatDistance, formatDuration, formatSpeed } from '../format';
  import { REPLAY_RATES, type ReplayController } from '../replay.svelte';
  import { SPEED_STOPS_KN } from '../map/replay';

  interface Props {
    replay: ReplayController;
  }

  let { replay }: Props = $props();

  const stats = $derived(replay.profile?.stats ?? null);
  const points = $derived(replay.profile?.points ?? []);
  const current = $derived(replay.current);

  const timeFormat = new Intl.DateTimeFormat('fr-FR', {
    dateStyle: 'short',
    timeStyle: 'medium',
    timeZone: 'UTC',
  });

  const rateLabel = (rate: number) => (rate < 60 ? `×${rate}` : `×${rate / 60} min/s`);
</script>

<section class="replay" aria-label="Relecture de trace" data-testid="track-replay">
  <header>
    <h3>Relecture — {replay.trackName}</h3>
    <button
      type="button"
      class="close"
      aria-label="Fermer la relecture"
      onclick={() => replay.close()}>✕</button
    >
  </header>

  {#if replay.busy}
    <p class="muted">Chargement…</p>
  {:else if replay.error}
    <p class="error" role="alert">{replay.error}</p>
  {:else if stats}
    <dl class="stats" data-testid="replay-stats">
      <dt>Distance</dt>
      <dd>{formatDistance(stats.distanceM)}</dd>
      <dt>Durée</dt>
      <dd>{formatDuration(stats.durationS * 1000)}</dd>
      <dt>En route</dt>
      <dd>{formatDuration(stats.movingS * 1000)}</dd>
      <dt>Moyenne</dt>
      <dd>{formatSpeed(stats.movingAverageMps)}</dd>
      <dt>Max</dt>
      <dd>{formatSpeed(stats.maxMps)}</dd>
      {#if stats.minDepthM !== null}
        <dt>Prof. min</dt>
        <dd>{formatDepth(stats.minDepthM)}</dd>
      {/if}
    </dl>

    {#if points.length >= 2}
      <input
        type="range"
        min="0"
        max={points.length - 1}
        step="1"
        value={replay.index}
        aria-label="Position dans la trace"
        oninput={(e) => replay.setIndex(Number(e.currentTarget.value))}
      />
      <div class="controls">
        <button
          type="button"
          aria-label={replay.playing ? 'Pause' : 'Lecture'}
          onclick={() => replay.toggle()}>{replay.playing ? '❚❚' : '▶'}</button
        >
        <select
          aria-label="Vitesse de lecture"
          value={replay.rate}
          onchange={(e) => replay.setRate(Number(e.currentTarget.value))}
        >
          {#each REPLAY_RATES as rate (rate)}
            <option value={rate}>{rateLabel(rate)}</option>
          {/each}
        </select>
      </div>
    {:else}
      <p class="muted">Trace trop courte pour être rejouée.</p>
    {/if}

    {#if current}
      <dl class="now" data-testid="replay-now">
        <dt>Heure</dt>
        <dd>{timeFormat.format(current.at)} UTC</dd>
        <dt>Vitesse</dt>
        <dd>{formatSpeed(current.speedMps)}</dd>
        <dt>Parcouru</dt>
        <dd>{formatDistance(current.distanceM)}</dd>
        {#if current.depthM !== null}
          <dt>Profondeur</dt>
          <dd>{formatDepth(current.depthM)}</dd>
        {/if}
      </dl>
    {/if}

    <div class="legend" aria-label="Couleurs de vitesse">
      {#each SPEED_STOPS_KN as [kn, color] (kn)}
        <span><i style:background={color}></i>{kn} nd</span>
      {/each}
      <span><i class="unknown"></i>inconnue</span>
    </div>
  {/if}
</section>

<style>
  .replay {
    min-width: 14rem;
    max-width: 19rem;
    padding: 0.5rem 0.6rem;
    border: 1px solid var(--border);
    border-left: 3px solid #43a047;
    border-radius: 0.4rem;
    background: var(--panel-bg);
    color: var(--fg);
    font-size: 0.85rem;
    box-shadow: 0 2px 8px rgb(0 0 0 / 25%);
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.4rem;
  }

  h3 {
    overflow: hidden;
    margin: 0;
    color: var(--muted);
    font-size: 0.78rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .close {
    border: 0;
    background: none;
    color: var(--fg);
    cursor: pointer;
  }

  dl {
    display: grid;
    grid-template-columns: auto auto;
    gap: 0.1rem 0.6rem;
    margin: 0.35rem 0;
  }

  dt {
    color: var(--muted);
  }

  dd {
    margin: 0;
    font-weight: 600;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  input[type='range'] {
    width: 100%;
  }

  .controls {
    display: flex;
    gap: 0.4rem;
  }

  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 0.15rem 0.5rem;
    color: var(--muted);
    font-size: 0.72rem;
  }

  .legend i {
    display: inline-block;
    width: 0.9rem;
    height: 0.3rem;
    margin-right: 0.2rem;
    vertical-align: middle;
  }

  .legend i.unknown {
    background: #9e9e9e;
  }

  .muted {
    color: var(--muted);
  }

  .error {
    color: var(--danger, #d32f2f);
  }

  p {
    margin: 0.35rem 0;
  }
</style>
