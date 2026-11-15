<script lang="ts">
  // Barre d'en-tête : onglets, état des chaînes de données, alertes,
  // heure locale (UTC en infobulle), thème et plein écran. Affichage seul : les états sont
  // calculés par `shell.ts` à partir des données reçues.
  import { activeAlerts, localClock, utcClock, worstLevel, type StatusPill } from '../shell';
  import { loadStatusPanelOpen, saveStatusPanelOpen } from '../panels';
  import type { Snippet } from 'svelte';
  import type { Theme } from '../map/style';
  import Icon, { type IconName } from './Icon.svelte';

  interface Props {
    /** Onglets, affichés en icônes (nom en infobulle et pour l'accessibilité). */
    tabs: readonly { id: string; label: string; icon: IconName }[];
    tab: string;
    onselect: (id: string) => void;
    pills: StatusPill[];
    now: number;
    theme: Theme;
    ontheme: () => void;
    fullscreen: boolean;
    onfullscreen: () => void;
    /** Outils de la carte (onglet Carte), entre les onglets et l'état. */
    tools?: Snippet;
  }

  let { tabs, tab, onselect, pills, now, theme, ontheme, fullscreen, onfullscreen, tools }: Props =
    $props();

  const alerts = $derived(activeAlerts(pills));
  const clock = $derived(localClock(now));
  const utc = $derived(utcClock(now));
  const level = $derived(worstLevel(pills));
  const summary = $derived(pills.map((p) => `${p.label} : ${p.state}`).join('\n'));

  /** Panneau d'état ouvert (mémorisé). Les alertes restent visibles fermé. */
  let statusOpen = $state(loadStatusPanelOpen());

  function toggleStatus(): void {
    statusOpen = !statusOpen;
    saveStatusPanelOpen(statusOpen);
  }

  /** Icône de chaque pastille d'état. */
  const PILL_ICONS: Record<StatusPill['id'], IconName> = {
    position: 'center',
    link: 'antenna',
    ais: 'ais',
    alarm: 'bell',
    engine: 'chip',
  };
</script>

<header class="bar" data-testid="app-header">
  <div role="tablist" aria-label="Affichage">
    {#each tabs as t (t.id)}
      <button
        type="button"
        role="tab"
        aria-selected={tab === t.id}
        aria-label={t.label}
        title={t.label}
        onclick={() => onselect(t.id)}
      >
        <Icon name={t.icon} />
      </button>
    {/each}
  </div>

  <span class="sep" aria-hidden="true"></span>
  {#if tools}
    <div class="tools">{@render tools()}</div>
    <span class="sep" aria-hidden="true"></span>
  {/if}

  <div class="info">
    <div class="status">
      <!-- Icône colorée selon l'état le plus grave ; détail dans le panneau. -->
      <button
        type="button"
        class="icon status-toggle {level}"
        aria-label="État des données"
        aria-expanded={statusOpen}
        aria-controls="status-panel"
        title={summary}
        data-testid="status-toggle"
        onclick={toggleStatus}
      >
        <Icon name="status" />
      </button>
      {#if statusOpen}
        <ul
          class="pills"
          id="status-panel"
          aria-label="État des données"
          data-testid="status-panel"
        >
          {#each pills as p (p.id)}
            <li class={p.level} title={p.title} data-testid="pill-{p.id}">
              <Icon name={PILL_ICONS[p.id]} />
              <span class="label">{p.label}</span>
              <span class="state">{p.state}</span>
            </li>
          {/each}
        </ul>
      {/if}
    </div>

    <div class="end">
      {#if alerts.length > 0}
        <span class="alerts" role="alert" title={alerts.join('\n')} data-testid="header-alerts">
          ⚠ {alerts.length} alerte{alerts.length > 1 ? 's' : ''} active{alerts.length > 1
            ? 's'
            : ''}
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
        aria-label={theme === 'day' ? 'Mode nuit' : 'Mode jour'}
        title={theme === 'day' ? 'Passer en mode nuit' : 'Passer en mode jour'}
        onclick={ontheme}
      >
        <Icon name={theme === 'day' ? 'moon' : 'sun'} />
      </button>
      <button
        type="button"
        class="icon"
        aria-pressed={fullscreen}
        aria-label={fullscreen ? 'Quitter le plein écran' : 'Plein écran'}
        title={fullscreen ? 'Quitter le plein écran (Échap)' : 'Plein écran (F11)'}
        onclick={onfullscreen}
      >
        <Icon name={fullscreen ? 'shrink' : 'expand'} />
      </button>
    </div>
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

  [role='tablist'] {
    display: flex;
    gap: 0.25rem;
  }

  [role='tab'] {
    display: inline-flex;
    align-items: center;
    padding: 0.4rem 0.55rem;
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

  .status {
    position: relative;
  }

  /* Couleur de l'état le plus grave : vert, orange, rouge sur fond rouge. */
  .status-toggle.ok {
    color: var(--ok-fg);
  }

  .status-toggle.warn {
    color: var(--sim-fg);
  }

  .status-toggle.alert {
    background: var(--alert-bg);
    color: var(--alert-fg);
  }

  .status-toggle.off {
    color: var(--muted);
  }

  /* Panneau sous l'icône, au-dessus de la carte et du bandeau d'alarme
     (qu'il ne recouvre que tant qu'il est ouvert). */
  .pills {
    position: absolute;
    top: calc(100% + 0.4rem);
    right: 0;
    z-index: 20;
    display: grid;
    grid-template-columns: auto auto 1fr;
    gap: 0.15rem 0.5rem;
    min-width: 14rem;
    padding: 0.5rem;
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    background: var(--card-bg);
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.3);
    margin: 0;
    list-style: none;
  }

  .pills li {
    display: grid;
    grid-column: 1 / -1;
    grid-template-columns: subgrid;
    align-items: center;
    padding: 0.3rem;
    border-radius: 0.35rem;
    color: var(--muted);
    cursor: default;
  }

  .pills li.ok {
    color: var(--ok-fg);
  }

  .pills li.warn {
    color: var(--sim-fg);
  }

  /* Alerte : icône rouge sur fond d'alerte, impossible à confondre. */
  .pills li.alert {
    background: var(--alert-bg);
    color: var(--alert-fg);
  }

  .label {
    color: var(--fg);
    font-weight: 600;
  }

  .pills li.alert .label {
    color: inherit;
  }

  .state {
    font-size: 0.85rem;
  }

  /* Séparateurs : onglets | outils de la carte | état et réglages. */
  .sep {
    align-self: stretch;
    width: 1px;
    margin: 0.3rem 0;
    background: var(--border);
  }

  /* Ne se tasse pas : sur un écran étroit, passe sur sa propre ligne. */
  .tools {
    flex: 0 1 auto;
    max-width: 100%;
  }

  /* État des données, alertes, heure et réglages, poussés à droite. */
  .info {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.75rem 1rem;
    margin-left: auto;
  }

  .end {
    display: flex;
    align-items: center;
    gap: 0.75rem;
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
    display: inline-flex;
    align-items: center;
    padding: 0.35rem;
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    background: var(--button-bg);
    color: var(--fg);
    font: inherit;
    font-size: 0.85rem;
    cursor: pointer;
  }
</style>
