<script lang="ts">
  import { describeConnection } from '../format';
  import {
    BAUD_RATES,
    type ConnectionState,
    type Rejections,
    type SourcesApi,
    type SourcesView,
  } from '../services/instruments';
  import { describeSource, formFromView, requestFromForm, type SourceForm } from '../sourceForm';

  interface Props {
    api: SourcesApi;
    /** Dernier état reçu par événement (`undefined` : aucun encore). */
    connection: ConnectionState | null | undefined;
    /** Derniers rejets reçus par événement (`undefined` : aucun encore). */
    rejections: Rejections | undefined;
  }

  let { api, connection, rejections }: Props = $props();

  let view = $state<SourcesView | null>(null);
  let form = $state<SourceForm | null>(null);
  let error = $state<string | null>(null);
  let busy = $state(false);
  let ports = $state<string[]>([]);

  const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

  function show(v: SourcesView) {
    view = v;
    form = null;
  }

  $effect(() => {
    api
      .get()
      .then(show)
      .catch((e: unknown) => (error = message(e)));
  });

  const live = $derived(connection === undefined ? (view?.connection ?? null) : connection);
  const status = $derived(describeConnection(live));
  const rejected = $derived(rejections ?? view?.rejections ?? { count: 0, lastReason: null });

  function edit() {
    if (!view) return;
    form = formFromView(view);
    void refreshPorts();
  }

  async function refreshPorts() {
    try {
      ports = await api.serialPorts();
    } catch (e) {
      error = message(e);
    }
  }

  async function chooseRecording() {
    if (!form) return;
    try {
      const path = await api.chooseRecording();
      if (path !== null && form) form.replayPath = path;
    } catch (e) {
      error = message(e);
    }
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (!form || !view) return;
    busy = true;
    try {
      show(await api.set(requestFromForm(form, view)));
      error = null;
    } catch (e) {
      error = message(e);
      // Le jeton saisi n'est jamais conservé dans l'interface.
      if (form) form.token = '';
    } finally {
      busy = false;
    }
  }

  function cancel() {
    form = null;
    error = null;
  }
</script>

<section class="source" aria-label="Source de données">
  <h2>Source</h2>
  {#if view}
    <p class="current" data-testid="source-mode">{describeSource(view)}</p>
    {#if view.mode !== 'simulator'}
      <p
        class="status {status.level}"
        role={status.level === 'alert' ? 'alert' : 'status'}
        data-testid="source-status"
      >
        {status.label}
      </p>
      {#if rejected.count > 0}
        <p class="rejections" data-testid="source-rejections" title={rejected.lastReason ?? ''}>
          {rejected.count} donnée{rejected.count > 1 ? 's' : ''} rejetée{rejected.count > 1
            ? 's'
            : ''}{rejected.lastReason ? ` — dernière : ${rejected.lastReason}` : ''}
        </p>
      {/if}
    {/if}
    {#if !form}
      <button type="button" onclick={edit}>Modifier la source</button>
    {:else}
      <form onsubmit={submit}>
        <fieldset>
          <legend>Mode</legend>
          <label><input type="radio" bind:group={form.mode} value="simulator" /> Simulateur</label>
          <label><input type="radio" bind:group={form.mode} value="signalK" /> Signal K</label>
          <label><input type="radio" bind:group={form.mode} value="nmea0183" /> NMEA 0183</label>
        </fieldset>

        {#if form.mode === 'signalK'}
          <label class="field">
            Adresse du serveur
            <input
              type="url"
              bind:value={form.signalkUrl}
              placeholder="http://192.168.1.10:3000"
              autocomplete="off"
              spellcheck="false"
            />
          </label>
          <label class="field">
            Jeton d'accès {view.tokenStored
              ? '(enregistré — laisser vide pour le conserver)'
              : '(facultatif)'}
            <input
              type="password"
              bind:value={form.token}
              autocomplete="off"
              disabled={form.clearToken}
            />
          </label>
          {#if view.tokenStored}
            <label
              ><input type="checkbox" bind:checked={form.clearToken} /> Supprimer le jeton enregistré</label
            >
          {/if}
        {:else if form.mode === 'nmea0183'}
          <label class="field">
            Liaison
            <select bind:value={form.nmeaKind}>
              <option value="tcp">TCP (multiplexeur, passerelle)</option>
              <option value="udp">UDP (diffusion réseau)</option>
              <option value="serial">Port série (USB)</option>
              <option value="replay">Rejeu d'un enregistrement</option>
            </select>
          </label>
          {#if form.nmeaKind === 'tcp'}
            <label class="field">
              Adresse de l'équipement
              <input
                type="text"
                bind:value={form.host}
                placeholder="192.168.1.10"
                autocomplete="off"
                spellcheck="false"
              />
            </label>
          {/if}
          {#if form.nmeaKind === 'tcp' || form.nmeaKind === 'udp'}
            <label class="field">
              Port
              <input type="text" inputmode="numeric" bind:value={form.port} autocomplete="off" />
            </label>
          {/if}
          {#if form.nmeaKind === 'serial'}
            <label class="field">
              Port série
              <input
                type="text"
                bind:value={form.serialPath}
                list="nmea-serial-ports"
                placeholder="COM3 ou /dev/ttyUSB0"
                autocomplete="off"
                spellcheck="false"
              />
              <datalist id="nmea-serial-ports">
                {#each ports as port (port)}
                  <option value={port}></option>
                {/each}
              </datalist>
            </label>
            <label class="field">
              Débit
              <select bind:value={form.baudRate}>
                {#each BAUD_RATES as rate (rate)}
                  <option value={rate}>{rate} Bd</option>
                {/each}
              </select>
            </label>
          {/if}
          {#if form.nmeaKind === 'replay'}
            <p class="hint">Données passées, jamais présentées comme la position actuelle.</p>
            <div class="actions">
              <button type="button" onclick={chooseRecording}>Choisir un fichier…</button>
            </div>
            {#if form.replayPath}
              <p class="path" data-testid="replay-path">{form.replayPath}</p>
            {/if}
          {/if}
        {/if}

        {#if form.mode !== 'simulator'}
          <label class="field">
            Priorités des sources (une par ligne, la première l'emporte)
            <textarea
              rows="2"
              bind:value={form.priorities}
              placeholder="nmea0183.GP"
              spellcheck="false"></textarea>
          </label>
        {/if}

        <div class="actions">
          <button type="submit" disabled={busy}>Appliquer</button>
          <button type="button" disabled={busy} onclick={cancel}>Annuler</button>
        </div>
      </form>
    {/if}
  {:else if !error}
    <p class="muted">Chargement…</p>
  {/if}
  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}
</section>

<style>
  h2 {
    margin: 0.75rem 0 0.25rem;
    font-size: 1rem;
  }

  p {
    margin: 0.25rem 0;
    font-size: 0.85rem;
  }

  .current,
  .path {
    word-break: break-all;
  }

  .status {
    padding: 0.15rem 0.4rem;
    border-radius: 0.25rem;
  }

  .status.ok {
    color: var(--fg);
  }

  .status.pending {
    background: var(--warn-bg);
    color: var(--warn-fg);
  }

  .status.alert {
    background: var(--alert-bg);
    color: var(--alert-fg);
    font-weight: 600;
  }

  .rejections,
  .hint,
  .path {
    color: var(--muted);
    font-size: 0.75rem;
  }

  form {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    font-size: 0.85rem;
  }

  fieldset {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem 0.75rem;
    margin: 0;
    padding: 0.2rem 0.5rem;
    border: 1px solid var(--border);
    border-radius: 0.3rem;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }

  input[type='url'],
  input[type='text'],
  input[type='password'],
  select,
  textarea {
    padding: 0.25rem 0.4rem;
    border: 1px solid var(--border);
    border-radius: 0.25rem;
    background: var(--button-bg);
    color: var(--fg);
    font: inherit;
  }

  textarea {
    resize: vertical;
  }

  .actions {
    display: flex;
    gap: 0.4rem;
  }

  button {
    padding: 0.3rem 0.6rem;
    border: 1px solid var(--border);
    border-radius: 0.3rem;
    background: var(--button-bg);
    color: var(--fg);
    font: inherit;
    font-size: 0.85rem;
    cursor: pointer;
  }

  button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .muted {
    color: var(--muted);
  }

  .error {
    color: var(--error-fg);
    font-weight: 600;
  }
</style>
