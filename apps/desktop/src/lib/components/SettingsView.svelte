<script lang="ts" module>
  /** Sections de l'onglet Paramètres, dans l'ordre de la navigation. */
  export const SETTINGS_SECTIONS = [
    { id: 'general', label: 'Général', hint: 'Unités, carte, simulateur' },
    { id: 'instruments', label: 'Instruments', hint: 'Barre Navigation' },
    { id: 'routes', label: 'Routes', hint: 'Routes, traces et marques' },
    { id: 'sources', label: 'Source', hint: 'Données du bord' },
    { id: 'alarms', label: 'Alarmes', hint: 'Surveillance, historique' },
    { id: 'charts', label: 'Cartes', hint: 'MBTiles, ENC, îles et amers' },
    { id: 'weather', label: 'Météo', hint: 'Fichiers GRIB, vent' },
    { id: 'logbook', label: 'Journal', hint: 'Journal de bord' },
  ] as const;

  export type SettingsSection = (typeof SETTINGS_SECTIONS)[number]['id'];

  const SECTION_KEY = 'nautik.settingsSection';

  /** Section mémorisée ; inconnue ou absente : Général. */
  export function loadSection(
    storage: Pick<Storage, 'getItem'> | undefined = globalThis.localStorage,
  ): SettingsSection {
    try {
      const value = storage?.getItem(SECTION_KEY);
      return SETTINGS_SECTIONS.find((s) => s.id === value)?.id ?? 'general';
    } catch {
      return 'general';
    }
  }

  export function saveSection(section: SettingsSection): void {
    try {
      globalThis.localStorage?.setItem(SECTION_KEY, section);
    } catch {
      // Préférence non mémorisée : sans conséquence.
    }
  }
</script>

<script lang="ts">
  // Onglet Paramètres : une navigation par sections (une seule affichée à
  // la fois) et des réglages compacts. Les sections restent montées : un
  // formulaire en cours (source, alarmes) n'est pas perdu en changeant de
  // section. Les alertes de sûreté restent visibles au-dessus.
  import type { Snippet } from 'svelte';
  import type { SimulatorApi } from '../services/simulator';
  import { DEFAULT_ACTIVE_ROUTE_COLOR } from '../map/overlays';
  import { routeStyle, setActiveRouteColor } from '../routeColor.svelte';
  import { followBoat, setFollowBoat } from '../followBoat.svelte';
  import {
    moveNavField,
    navBar,
    navFieldLabel,
    resetNavFields,
    setNavFieldShown,
  } from '../navFields.svelte';
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
    /** Alarmes (surveillance et historique). */
    alarms?: Snippet;
    /** Dossiers des cartes, ENC, îles et amers. */
    charts?: Snippet;
    /** Gestionnaire des routes, traces et marques. */
    routesView?: Snippet;
    /** Fichiers météo (GRIB). */
    weatherView?: Snippet;
    /** Journal de bord. */
    logbookView?: Snippet;
    /** Section affichée (choisie aussi depuis la barre de la carte). */
    section?: SettingsSection;
  }

  let {
    simulator,
    status,
    sources,
    alarms,
    charts,
    routesView,
    weatherView,
    logbookView,
    section = $bindable(loadSection()),
  }: Props = $props();

  /** Sections disponibles (Alarmes, Cartes, Météo et Journal sont facultatives). */
  const available = $derived(
    SETTINGS_SECTIONS.filter((s) => {
      const snippets: Partial<Record<SettingsSection, Snippet | undefined>> = {
        alarms,
        charts,
        routes: routesView,
        weather: weatherView,
        logbook: logbookView,
      };
      return !(s.id in snippets) || snippets[s.id] !== undefined;
    }),
  );

  function select(id: SettingsSection): void {
    section = id;
    saveSection(id);
  }

  /** Flèches haut/bas (ou gauche/droite) : section voisine. */
  function onNavKeydown(event: KeyboardEvent): void {
    const step = { ArrowDown: 1, ArrowRight: 1, ArrowUp: -1, ArrowLeft: -1 }[event.key];
    if (step === undefined) return;
    event.preventDefault();
    const ids = available.map((s) => s.id);
    const next = ids[(ids.indexOf(section) + step + ids.length) % ids.length];
    if (next) {
      select(next);
      document.getElementById(`settings-tab-${next}`)?.focus();
    }
  }

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

  <div class="layout">
    <nav class="sections" aria-label="Sections des paramètres">
      <div role="tablist" aria-orientation="vertical" tabindex="-1" onkeydown={onNavKeydown}>
        {#each available as s (s.id)}
          <button
            type="button"
            role="tab"
            id={`settings-tab-${s.id}`}
            aria-selected={section === s.id}
            aria-controls={`settings-panel-${s.id}`}
            tabindex={section === s.id ? 0 : -1}
            onclick={() => select(s.id)}
          >
            <span class="label">{s.label}</span>
            <span class="hint">{s.hint}</span>
          </button>
        {/each}
      </div>
    </nav>

    <div class="content">
      <div
        role="tabpanel"
        id="settings-panel-general"
        aria-labelledby="settings-tab-general"
        hidden={section !== 'general'}
      >
        <section class="card" aria-labelledby="units-title">
          <h2 id="units-title">Unités</h2>
          <div class="fields">
            <label>
              <span>Vitesse</span>
              <select
                value={units.speed}
                onchange={(e) => changeSpeedUnit(e.currentTarget.value as SpeedUnit)}
              >
                {#each options(SPEED_UNITS) as o (o.key)}<option value={o.key}>{o.label}</option
                  >{/each}
              </select>
            </label>
            <label>
              <span>Distance</span>
              <select
                value={units.distance}
                onchange={(e) => setUnits({ distance: e.currentTarget.value as DistanceUnit })}
              >
                {#each options(DISTANCE_UNITS) as o (o.key)}<option value={o.key}>{o.label}</option
                  >{/each}
              </select>
            </label>
            <label>
              <span>Profondeur</span>
              <select
                value={units.depth}
                onchange={(e) => setUnits({ depth: e.currentTarget.value as DepthUnit })}
              >
                {#each options(DEPTH_UNITS) as o (o.key)}<option value={o.key}>{o.label}</option
                  >{/each}
              </select>
            </label>
          </div>
          <p class="note">Le moteur calcule toujours en unités SI ; seul l'affichage change.</p>
        </section>

        <section class="card" aria-labelledby="route-color-title">
          <h2 id="route-color-title">Carte</h2>
          <div class="row">
            <label class="inline">
              <input
                type="color"
                value={routeStyle.activeColor}
                oninput={(e) => setActiveRouteColor(e.currentTarget.value)}
              />
              <span>Couleur de la route active</span>
            </label>
            <button
              type="button"
              disabled={routeStyle.activeColor === DEFAULT_ACTIVE_ROUTE_COLOR}
              onclick={() => setActiveRouteColor(DEFAULT_ACTIVE_ROUTE_COLOR)}>Jaune (défaut)</button
            >
          </div>
          <p class="note">
            Bordée d'un liseré sombre et toujours affichée ; les autres routes restent violettes.
          </p>
          <label class="inline follow">
            <input
              type="checkbox"
              checked={followBoat.enabled}
              onchange={(e) => setFollowBoat(e.currentTarget.checked)}
            />
            <span>Faire défiler la carte avec le bateau</span>
          </label>
          <p class="note">
            Le bouton Centrer de la barre devient un interrupteur : actif, la carte suit chaque
            nouvelle position ; la déplacer à la main arrête le suivi. Désactivée : le bouton centre
            une seule fois.
          </p>
        </section>

        <section class="card" aria-labelledby="sim-title">
          <h2 id="sim-title">Simulateur</h2>
          <form class="row" onsubmit={saveSpeed}>
            <label class="inline">
              <span>Vitesse du bateau simulé ({SPEED_UNITS[units.speed].label})</span>
              <input
                class="number"
                type="text"
                inputmode="decimal"
                bind:value={speedText}
                oninput={() => (speedSaved = false)}
              />
            </label>
            <button type="submit" disabled={busy}>Enregistrer</button>
          </form>
          {#if speedError}
            <p class="error" role="alert">{speedError}</p>
          {:else if speedSaved}
            <p class="ok" role="status">Vitesse enregistrée.</p>
          {/if}
          <p class="note">
            Vitesse suivie sur la route active en mode simulateur (0 à 30 nœuds). Le bateau se place
            aussi sur la carte (barre d'outils ou clic droit).
          </p>
        </section>
      </div>

      <div
        role="tabpanel"
        id="settings-panel-instruments"
        aria-labelledby="settings-tab-instruments"
        hidden={section !== 'instruments'}
      >
        <section class="card" aria-labelledby="nav-fields-title">
          <h2 id="nav-fields-title">Barre Navigation</h2>
          <p class="note">
            Cochez les valeurs à afficher ; les flèches changent leur ordre (de gauche à droite en
            bas, de haut en bas sur le côté). Au moins une valeur reste affichée.
          </p>
          <ol class="nav-fields" aria-label="Valeurs de la barre Navigation">
            {#each navBar.fields as f, i (f.id)}
              {@const label = navFieldLabel(f.id)}
              {@const last = f.shown && navBar.fields.filter((x) => x.shown).length === 1}
              <li data-field={f.id}>
                <label class="inline">
                  <input
                    type="checkbox"
                    checked={f.shown}
                    disabled={last}
                    title={last ? 'Au moins une valeur reste affichée' : undefined}
                    onchange={(e) => setNavFieldShown(f.id, e.currentTarget.checked)}
                  />
                  <span>{label}</span>
                </label>
                <span class="order">
                  <button
                    type="button"
                    aria-label="Monter {label}"
                    disabled={i === 0}
                    onclick={() => moveNavField(f.id, -1)}>↑</button
                  >
                  <button
                    type="button"
                    aria-label="Descendre {label}"
                    disabled={i === navBar.fields.length - 1}
                    onclick={() => moveNavField(f.id, 1)}>↓</button
                  >
                </span>
              </li>
            {/each}
          </ol>
          <button type="button" onclick={() => resetNavFields()}
            >Tout afficher (ordre par défaut)</button
          >
        </section>
      </div>

      {#if routesView}
        <div
          role="tabpanel"
          id="settings-panel-routes"
          aria-labelledby="settings-tab-routes"
          hidden={section !== 'routes'}
        >
          <section class="card" aria-label="Routes, traces et marques">
            {@render routesView()}
          </section>
        </div>
      {/if}

      <div
        role="tabpanel"
        id="settings-panel-sources"
        aria-labelledby="settings-tab-sources"
        hidden={section !== 'sources'}
      >
        <section class="card" aria-label="Sources">
          {@render sources()}
        </section>
      </div>

      {#if alarms}
        <div
          role="tabpanel"
          id="settings-panel-alarms"
          aria-labelledby="settings-tab-alarms"
          hidden={section !== 'alarms'}
        >
          <section class="card">
            {@render alarms()}
          </section>
        </div>
      {/if}

      {#if charts}
        <div
          role="tabpanel"
          id="settings-panel-charts"
          aria-labelledby="settings-tab-charts"
          hidden={section !== 'charts'}
        >
          <section class="card">
            {@render charts()}
          </section>
        </div>
      {/if}
      {#if weatherView}
        <div
          role="tabpanel"
          id="settings-panel-weather"
          aria-labelledby="settings-tab-weather"
          hidden={section !== 'weather'}
        >
          <section class="card">
            {@render weatherView()}
          </section>
        </div>
      {/if}
      {#if logbookView}
        <div
          role="tabpanel"
          id="settings-panel-logbook"
          aria-labelledby="settings-tab-logbook"
          hidden={section !== 'logbook'}
        >
          <section class="card">
            {@render logbookView()}
          </section>
        </div>
      {/if}
    </div>
  </div>
</section>

<style>
  .settings-view {
    position: absolute;
    inset: 0;
    z-index: 3;
    display: flex;
    flex-direction: column;
    overflow: auto;
    padding: 0.75rem 1rem 2.5rem;
    background: var(--bg);
    color: var(--fg);
  }

  .layout {
    display: grid;
    grid-template-columns: 13rem minmax(0, 46rem);
    gap: 1rem;
    align-items: start;
    margin-top: 0.75rem;
  }

  /* Navigation collée en haut pendant le défilement d'une longue section. */
  .sections {
    position: sticky;
    top: 0;
  }

  [role='tablist'] {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }

  [role='tab'] {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.05rem;
    padding: 0.45rem 0.7rem;
    border: 1px solid transparent;
    border-left: 3px solid transparent;
    border-radius: 0.35rem;
    background: transparent;
    color: var(--fg);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  [role='tab']:hover {
    background: var(--button-bg);
  }

  [role='tab'][aria-selected='true'] {
    border-color: var(--border);
    border-left-color: var(--fg);
    background: var(--panel-bg);
  }

  [role='tab'] .label {
    font-weight: 600;
  }

  [role='tab'] .hint {
    color: var(--muted);
    font-size: 0.75rem;
  }

  .content {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    /* Un contenu large (tableau du journal) défile dans sa carte. */
    min-width: 0;
  }

  [role='tabpanel'] {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    min-width: 0;
  }

  [role='tabpanel'][hidden] {
    display: none;
  }

  .card {
    padding: 0.7rem 0.9rem;
    min-width: 0;
    border: 1px solid var(--border);
    border-radius: 0.5rem;
    background: var(--panel-bg);
  }

  h2 {
    margin: 0 0 0.5rem;
    color: var(--muted);
    font-size: 0.8rem;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }

  /* Trois unités sur une ligne, repliées si la place manque. */
  .fields {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(8rem, 1fr));
    gap: 0.5rem 0.75rem;
  }

  .fields label {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    font-size: 0.85rem;
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }

  label.inline {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem;
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

  input.number {
    width: 5.5rem;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  input[type='color'] {
    width: 2.5rem;
    height: 2rem;
    padding: 0.1rem;
    cursor: pointer;
  }

  button {
    cursor: pointer;
  }

  button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .note {
    margin: 0.5rem 0 0;
    color: var(--muted);
    font-size: 0.8rem;
  }

  .nav-fields {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    margin: 0.6rem 0;
    padding: 0;
    list-style: none;
  }

  .nav-fields li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    padding: 0.2rem 0.3rem;
    border-bottom: 1px solid var(--border);
  }

  .order {
    display: flex;
    gap: 0.2rem;
  }

  .error {
    margin: 0.4rem 0 0;
    color: var(--error-fg);
    font-weight: 600;
    font-size: 0.85rem;
  }

  .ok {
    margin: 0.4rem 0 0;
    font-size: 0.85rem;
  }

  /* Fenêtre étroite : sections en onglets horizontaux au-dessus. */
  @media (max-width: 760px) {
    .layout {
      grid-template-columns: minmax(0, 1fr);
      gap: 0.6rem;
    }

    .sections {
      position: static;
    }

    [role='tablist'] {
      flex-direction: row;
      overflow-x: auto;
    }

    [role='tab'] {
      flex: none;
      border-left-width: 1px;
    }

    [role='tab'][aria-selected='true'] {
      border-bottom: 3px solid var(--fg);
    }

    [role='tab'] .hint {
      display: none;
    }
  }

  label.follow {
    margin-top: 0.75rem;
  }
</style>
