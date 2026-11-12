<script lang="ts">
  import { fetchEngineInfo, type EngineInfo, type InvokeFn } from './lib/services/engine';

  interface Props {
    /** Fonction IPC injectable (tests). Par défaut : Tauri `invoke`. */
    invoke?: InvokeFn;
  }

  let { invoke }: Props = $props();

  let engine = $state<EngineInfo | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    fetchEngineInfo(invoke)
      .then((info) => {
        engine = info;
      })
      .catch((e: unknown) => {
        error = e instanceof Error ? e.message : String(e);
      });
  });
</script>

<main>
  <h1>NautiK</h1>

  {#if engine}
    <p class="status" data-testid="engine-status">
      Moteur {engine.version} — protocole v{engine.protocolVersion}
    </p>
    <p class="safety" role="note">{engine.safetyNotice}</p>
  {:else if error}
    <p class="error" role="alert">Moteur indisponible : {error}</p>
  {:else}
    <p class="status">Connexion au moteur…</p>
  {/if}
</main>

<style>
  main {
    max-width: 48rem;
    margin: 0 auto;
    padding: 2rem 1.5rem;
  }

  h1 {
    margin: 0 0 1rem;
  }

  .status {
    color: var(--muted);
  }

  .safety {
    padding: 0.75rem 1rem;
    border-radius: 0.5rem;
    background: var(--warn-bg);
    color: var(--warn-fg);
  }

  .error {
    color: var(--error-fg);
    font-weight: 600;
  }
</style>
