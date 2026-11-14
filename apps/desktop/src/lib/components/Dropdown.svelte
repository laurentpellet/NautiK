<script lang="ts">
  // Bouton à menu déroulant. Le menu s'ouvre vers le haut, ou vers le bas
  // quand le bouton est dans la moitié haute de l'écran. Fermeture par un
  // clic en dehors ou par Échap ; Échap est arrêté ici pour ne pas faire
  // quitter le plein écran en même temps.
  import type { Snippet } from 'svelte';

  interface Props {
    /** Nom accessible du bouton. */
    label: string;
    title?: string;
    /** Marque d'attention sur le bouton (élément masqué, par exemple). */
    marked?: boolean;
    /** `icon` : bouton carré de barre d'outils ; `chip` : étiquette sur la carte. */
    variant?: 'icon' | 'chip';
    testid?: string;
    /** Contenu du bouton. */
    button: Snippet;
    /** Contenu du menu. */
    children: Snippet;
  }

  let {
    label,
    title,
    marked = false,
    variant = 'icon',
    testid,
    button,
    children,
  }: Props = $props();

  let open = $state(false);
  let below = $state(false);
  let anchor: HTMLElement;

  function toggle(): void {
    if (!open) below = anchor.getBoundingClientRect().top < window.innerHeight / 2;
    open = !open;
  }

  function outside(event: PointerEvent): void {
    if (open && !anchor.contains(event.target as Node)) open = false;
  }

  function escape(event: KeyboardEvent): void {
    if (open && event.key === 'Escape') {
      open = false;
      event.stopPropagation();
    }
  }
</script>

<svelte:window onpointerdown={outside} />

<div class="anchor" role="presentation" bind:this={anchor} onkeydown={escape}>
  <button
    type="button"
    class={variant}
    class:marked
    aria-label={label}
    aria-haspopup="true"
    aria-expanded={open}
    title={title ?? label}
    onclick={toggle}
  >
    {@render button()}
    <span class="caret" aria-hidden="true">▾</span>
  </button>
  {#if open}
    <div class="menu" class:below data-testid={testid}>
      {@render children()}
    </div>
  {/if}
</div>

<style>
  .anchor {
    position: relative;
  }

  button {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 0.15rem;
    border: 1px solid transparent;
    color: var(--fg);
    font: inherit;
    cursor: pointer;
  }

  .icon {
    height: 2.1rem;
    padding: 0 0.3rem;
    border-radius: 0.35rem;
    background: transparent;
  }

  .icon:hover,
  .icon:focus-visible,
  [aria-expanded='true'] {
    border-color: var(--border);
    background: var(--button-bg);
  }

  .chip {
    padding: 0.2rem 0.5rem;
    border-color: var(--border);
    border-radius: 0.35rem;
    background: var(--panel-bg);
    font-size: 0.8rem;
    font-weight: 600;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.25);
  }

  .caret {
    color: var(--muted);
    font-size: 0.65rem;
  }

  /* Petite marque d'attention ; le détail reste affiché ailleurs. */
  .marked::after {
    content: '';
    position: absolute;
    top: 0.25rem;
    right: 0.2rem;
    width: 0.4rem;
    height: 0.4rem;
    border-radius: 50%;
    background: var(--warn-fg);
  }

  .menu {
    position: absolute;
    bottom: calc(100% + 0.5rem);
    left: 0;
    z-index: 5;
    min-width: 12rem;
    max-height: 60vh;
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    background: var(--card-bg);
    color: var(--fg);
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.3);
  }

  .menu.below {
    top: calc(100% + 0.5rem);
    bottom: auto;
  }
</style>
