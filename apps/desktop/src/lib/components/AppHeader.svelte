<script lang="ts">
  // Barre d'en-tête : onglets, état des chaînes de données, alertes,
  // heure locale (UTC en infobulle), thème et plein écran. Affichage seul : les états sont
  // calculés par `shell.ts` à partir des données reçues.
  import { activeAlerts, localClock, utcClock, type StatusPill } from '../shell';
  import type { Theme } from '../map/style';

  interface Props {
    tabs: readonly { id: string; label: string }[];
    tab: string;
    onselect: (id: string) => void;
    pills: StatusPill[];
    now: number;
    theme: Theme;
    ontheme: () => void;
    fullscreen: boolean;
    onfullscreen: () => void;
  }

  let { tabs, tab, onselect, pills, now, theme, ontheme, fullscreen, onfullscreen }: Props =
    $props();

  const alerts = $derived(activeAlerts(pills));
  const clock = $derived(localClock(now));
  const utc = $derived(utcClock(now));
</script>

<header class="bar" data-testid="app-header">
  <div class="brand">
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M12 2 L12 17 L4 17 Z" />
      <path d="M13.5 5 L20 17 L13.5 17 Z" />
      <path d="M3 19 H21 L18.5 22 H5.5 Z" />
    </svg>
    <span>NautiK</span>
  </div>

  <div role="tablist" aria-label="Affichage">
    {#each tabs as t (t.id)}
      <button type="button" role="tab" aria-selected={tab === t.id} onclick={() => onselect(t.id)}>
        {t.label}
      </button>
    {/each}
  </div>

  <ul class="pills" aria-label="État des données">
    {#each pills as p (p.id)}
      <li class={p.level} title={p.title} data-testid="pill-{p.id}">
        <span class="dot" aria-hidden="true"></span>
        <span class="text">
          <span class="label">{p.label}</span>
          <span class="state">{p.state}</span>
        </span>
      </li>
    {/each}
  </ul>

  <div class="end">
    {#if alerts.length > 0}
      <span class="alerts" role="alert" title={alerts.join('\n')} data-testid="header-alerts">
        ⚠ {alerts.length} alerte{alerts.length > 1 ? 's' : ''} active{alerts.length > 1 ? 's' : ''}
      </span>
    {/if}
    <span
      class="clock"
      aria-label="Heure locale"
      title="{utc.time} — {utc.date}"
      data-testid="clock"
    >
      <span class="date">{clock.date}</span>
      <span class="time">{clock.time} <small>{clock.zone}</small></span>
    </span>
    <button
      type="button"
      class="icon"
      title={theme === 'day' ? 'Passer en mode nuit' : 'Passer en mode jour'}
      onclick={ontheme}
    >
      {theme === 'day' ? 'Mode nuit' : 'Mode jour'}
    </button>
    <button
      type="button"
      class="icon"
      aria-pressed={fullscreen}
      title={fullscreen ? 'Quitter le plein écran (Échap)' : 'Plein écran (F11)'}
      onclick={onfullscreen}
    >
      {fullscreen ? 'Quitter le plein écran' : 'Plein écran'}
    </button>
  </div>
</header>

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 1rem;
    min-height: 3.4rem;
    padding: 0.3rem 1rem;
    border-bottom: 1px solid var(--border);
    background: var(--header-bg);
    color: var(--fg);
    flex-wrap: wrap;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 1.25rem;
    font-weight: 800;
    letter-spacing: 0.04em;
  }

  .brand svg {
    width: 1.6rem;
    height: 1.6rem;
    fill: currentColor;
  }

  [role='tablist'] {
    display: flex;
    gap: 0.25rem;
  }

  [role='tab'] {
    padding: 0.35rem 0.9rem;
    border: 1px solid transparent;
    border-radius: 0.4rem;
    background: transparent;
    color: var(--muted);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }

  [role='tab']:hover {
    color: var(--fg);
  }

  [role='tab'][aria-selected='true'] {
    border-color: var(--border);
    background: var(--accent-bg);
    color: var(--accent-fg);
  }

  .pills {
    display: flex;
    gap: 1.1rem;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .pills li {
    display: flex;
    align-items: flex-start;
    gap: 0.4rem;
    cursor: default;
  }

  .dot {
    width: 0.6rem;
    height: 0.6rem;
    margin-top: 0.35rem;
    border-radius: 50%;
    background: var(--muted);
  }

  .text {
    display: flex;
    flex-direction: column;
    line-height: 1.15;
  }

  .label {
    font-size: 0.85rem;
    font-weight: 600;
  }

  .state {
    font-size: 0.75rem;
    color: var(--muted);
  }

  .ok .dot {
    background: var(--ok-fg);
  }

  .ok .state {
    color: var(--ok-fg);
  }

  .warn .dot {
    background: var(--sim-fg);
  }

  .warn .state {
    color: var(--sim-fg);
  }

  .alert .dot {
    background: var(--alert-fg);
  }

  .alert .state {
    color: var(--alert-fg);
    font-weight: 700;
  }

  .end {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    margin-left: auto;
  }

  .alerts {
    padding: 0.35rem 0.8rem;
    border-radius: 0.4rem;
    background: var(--alert-bg);
    color: var(--alert-fg);
    font-size: 0.85rem;
    font-weight: 700;
  }

  .clock {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    line-height: 1.15;
  }

  .date {
    color: var(--muted);
    font-size: 0.75rem;
  }

  .time {
    font-size: 1.05rem;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }

  .time small {
    color: var(--muted);
    font-size: 0.7rem;
    font-weight: 600;
  }

  .icon {
    padding: 0.3rem 0.6rem;
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    background: var(--button-bg);
    color: var(--fg);
    font: inherit;
    font-size: 0.85rem;
    cursor: pointer;
  }
</style>
