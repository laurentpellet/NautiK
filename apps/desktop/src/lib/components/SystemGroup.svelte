<script lang="ts">
  // Carte d'un groupe de grandeurs (moteur, énergie, réservoirs,
  // environnement). Une donnée absente reste « — » ; une jauge n'est
  // dessinée que pour un niveau effectivement reçu.
  import type { Tile } from '../instrumentTiles';

  interface Props {
    title: string;
    tiles: Tile[];
    /** Couleur de la valeur (variable de thème). */
    tone?: string;
  }

  let { title, tiles, tone = 'var(--tone-main)' }: Props = $props();
</script>

<article class="card" aria-label={title} data-testid="group-{title}">
  <h2>{title}</h2>
  <div class="tiles">
    {#each tiles as t (t.id)}
      <div class="tile" class:stale={t.stale} class:missing={t.missing} data-testid="tile-{t.id}">
        <span class="label">
          {t.label}
          {#if t.stale}<span class="flag" aria-label="donnée obsolète">⚠</span>{/if}
        </span>
        <span class="reading">
          <span class="value" style:color={t.missing ? undefined : tone}>{t.value}</span>
          <span class="unit">{t.unit}</span>
        </span>
        {#if t.ratio !== undefined}
          <span
            class="gauge"
            class:low={t.ratio < 0.2}
            role="meter"
            aria-label={t.label}
            aria-valuemin="0"
            aria-valuemax="100"
            aria-valuenow={Math.round(t.ratio * 100)}
          >
            <span style:width="{t.ratio * 100}%"></span>
          </span>
        {/if}
        {#if t.detail}<span class="detail">{t.detail}</span>{/if}
      </div>
    {/each}
  </div>
</article>

<style>
  .card {
    padding: 1rem 1.2rem;
    border: 1px solid var(--border);
    border-radius: 0.8rem;
    background: var(--card-bg);
  }

  h2 {
    margin: 0 0 0.6rem;
    color: var(--muted);
    font-size: 0.9rem;
    letter-spacing: 0.12em;
  }

  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(7.5rem, 1fr));
    gap: 0.8rem 1rem;
  }

  .tile {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .label {
    color: var(--muted);
    font-size: 0.75rem;
    font-weight: 600;
    letter-spacing: 0.08em;
  }

  .reading {
    display: flex;
    align-items: baseline;
    gap: 0.3rem;
  }

  .value {
    font-size: 1.8rem;
    font-weight: 800;
    line-height: 1.1;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .unit {
    color: var(--muted);
    font-size: 0.9rem;
  }

  .missing .value {
    color: var(--muted);
  }

  .stale .value,
  .stale .unit {
    opacity: 0.4;
  }

  .flag,
  .stale .detail {
    color: var(--warn-fg);
  }

  .detail {
    color: var(--muted);
    font-size: 0.75rem;
  }

  .gauge {
    display: block;
    height: 0.45rem;
    margin-top: 0.25rem;
    border-radius: 0.3rem;
    background: var(--inset-bg);
    overflow: hidden;
  }

  .gauge span {
    display: block;
    height: 100%;
    background: var(--tone-good);
  }

  .gauge.low span {
    background: var(--tone-bad);
  }
</style>
