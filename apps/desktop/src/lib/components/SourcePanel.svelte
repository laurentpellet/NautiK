<script lang="ts">
  import { describeConnection } from '../format';
  import type {
    ConnectionState,
    SourceMode,
    SourcesApi,
    SourcesView,
  } from '../services/instruments';

  interface Props {
    api: SourcesApi;
    /** Dernier état reçu par événement (`undefined` : aucun encore). */
    connection: ConnectionState | null | undefined;
  }

  let { api, connection }: Props = $props();

  let view = $state<SourcesView | null>(null);
  let error = $state<string | null>(null);
  let busy = $state(false);
  let editing = $state(false);

  // Formulaire.
  let mode = $state<SourceMode>('simulator');
  let url = $state('');
  let token = $state('');
  let clearToken = $state(false);

  const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

  function fill(v: SourcesView) {
    view = v;
    mode = v.mode;
    url = v.signalkUrl ?? '';
    token = '';
    clearToken = false;
  }

  $effect(() => {
    api
      .get()
      .then(fill)
      .catch((e: unknown) => (error = message(e)));
  });

  const live = $derived(connection === undefined ? (view?.connection ?? null) : connection);
  const status = $derived(describeConnection(live));

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    busy = true;
    try {
      const trimmed = url.trim();
      fill(
        await api.set({
          mode,
          signalkUrl: trimmed === '' ? null : trimmed,
          token: token.trim() === '' ? null : token,
          clearToken,
        }),
      );
      error = null;
      editing = false;
    } catch (e) {
      error = message(e);
    } finally {
      // Le jeton saisi n'est jamais conservé dans l'interface.
      token = '';
      busy = false;
    }
  }

  function cancel() {
    if (view) fill(view);
    error = null;
    editing = false;
  }
</script>

<section class="source" aria-label="Source de données">
  <h2>Source</h2>
  {#if view}
    <p class="current" data-testid="source-mode">
      {view.mode === 'signalK' ? `Signal K — ${view.signalkUrl ?? '?'}` : 'Simulateur'}
    </p>
    {#if view.mode === 'signalK'}
      <p
        class="status {status.level}"
        role={status.level === 'alert' ? 'alert' : 'status'}
        data-testid="signalk-status"
      >
        {status.label}
      </p>
    {/if}
    {#if !editing}
      <button type="button" onclick={() => (editing = true)}>Modifier la source</button>
    {:else}
      <form onsubmit={submit}>
        <fieldset>
          <legend>Mode</legend>
          <label><input type="radio" bind:group={mode} value="simulator" /> Simulateur</label>
          <label><input type="radio" bind:group={mode} value="signalK" /> Signal K</label>
        </fieldset>
        <label class="field">
          Adresse du serveur
          <input
            type="url"
            bind:value={url}
            placeholder="http://192.168.1.10:3000"
            autocomplete="off"
            spellcheck="false"
          />
        </label>
        <label class="field">
          Jeton d'accès {view.tokenStored
            ? '(enregistré — laisser vide pour le conserver)'
            : '(facultatif)'}
          <input type="password" bind:value={token} autocomplete="off" disabled={clearToken} />
        </label>
        {#if view.tokenStored}
          <label
            ><input type="checkbox" bind:checked={clearToken} /> Supprimer le jeton enregistré</label
          >
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

  .current {
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

  form {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    font-size: 0.85rem;
  }

  fieldset {
    display: flex;
    gap: 0.75rem;
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
  input[type='password'] {
    padding: 0.25rem 0.4rem;
    border: 1px solid var(--border);
    border-radius: 0.25rem;
    background: var(--button-bg);
    color: var(--fg);
    font: inherit;
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
