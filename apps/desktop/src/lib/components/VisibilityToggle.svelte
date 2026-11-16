<script lang="ts">
  // Œil de visibilité d'une ligne de liste (trace, route) : ouvert quand
  // l'élément est affiché sur la carte, barré quand il est masqué.
  import Icon from './Icon.svelte';

  interface Props {
    /** Élément affiché sur la carte. */
    visible: boolean;
    /** Nom de l'élément, repris dans le libellé accessible. */
    name: string;
    disabled?: boolean;
    /** Infobulle ; par défaut « Afficher / Masquer sur la carte ». */
    title?: string;
    ontoggle: () => void;
  }

  let { visible, name, disabled = false, title, ontoggle }: Props = $props();
</script>

<button
  type="button"
  class="eye"
  class:off={!visible}
  aria-pressed={visible}
  aria-label={`${visible ? 'Masquer' : 'Afficher'} ${name}`}
  title={title ?? (visible ? 'Masquer sur la carte' : 'Afficher sur la carte')}
  {disabled}
  onclick={ontoggle}
>
  <Icon name={visible ? 'eye' : 'eyeOff'} />
</button>

<style>
  .eye {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0.15rem;
    border: 0;
    border-radius: 0.3rem;
    background: none;
    color: var(--fg);
    cursor: pointer;
    flex-shrink: 0;
  }

  .eye:hover:not(:disabled),
  .eye:focus-visible {
    background: var(--button-bg);
    outline: none;
  }

  .eye.off {
    color: var(--muted);
  }

  .eye:disabled {
    cursor: default;
    opacity: 0.6;
  }
</style>
