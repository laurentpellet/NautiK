<script lang="ts" generics="T extends string">
  // Menu contextuel (clic droit ou appui long) pour choisir où placer une
  // barre. Fermeture par un choix, un clic en dehors ou Échap (arrêté ici
  // pour ne pas quitter le plein écran en même temps).
  interface Props {
    /** Titre du menu (nom accessible). */
    title: string;
    /** Position du pointeur dans la fenêtre (px). */
    x: number;
    y: number;
    options: readonly { id: T; label: string }[];
    current: T;
    onselect: (id: T) => void;
    onclose: () => void;
  }

  let { title, x, y, options, current, onselect, onclose }: Props = $props();

  let menu: HTMLElement;
  let left = $state(0);
  let top = $state(0);

  // Reste dans la fenêtre, même ouvert près d'un bord.
  $effect(() => {
    const { width, height } = menu.getBoundingClientRect();
    left = Math.max(0, Math.min(x, window.innerWidth - width - 4));
    top = Math.max(0, Math.min(y, window.innerHeight - height - 4));
    menu.querySelector<HTMLElement>('[aria-checked="true"]')?.focus();
  });

  function outside(event: PointerEvent): void {
    if (!menu.contains(event.target as Node)) onclose();
  }

  function keydown(event: KeyboardEvent): void {
    if (event.key === 'Escape') {
      event.stopPropagation();
      onclose();
    }
  }
</script>

<svelte:window onpointerdown={outside} />

<div
  class="menu"
  role="menu"
  tabindex="-1"
  aria-label={title}
  style:left="{left}px"
  style:top="{top}px"
  bind:this={menu}
  onkeydown={keydown}
  data-testid="placement-menu"
>
  <p class="title">{title}</p>
  {#each options as o (o.id)}
    <button
      type="button"
      role="menuitemradio"
      aria-checked={o.id === current}
      onclick={() => {
        onselect(o.id);
        onclose();
      }}
    >
      <span class="check" aria-hidden="true">{o.id === current ? '✓' : ''}</span>
      {o.label}
    </button>
  {/each}
</div>

<style>
  .menu {
    position: fixed;
    z-index: 30;
    display: flex;
    flex-direction: column;
    min-width: 11rem;
    padding: 0.3rem;
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    background: var(--card-bg);
    color: var(--fg);
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.35);
  }

  .title {
    margin: 0.1rem 0.4rem 0.3rem;
    color: var(--muted);
    font-size: 0.75rem;
    font-weight: 700;
  }

  button {
    display: flex;
    gap: 0.4rem;
    align-items: center;
    padding: 0.35rem 0.5rem;
    border: 0;
    border-radius: 0.3rem;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  button:hover,
  button:focus-visible {
    background: var(--button-bg);
  }

  .check {
    width: 1rem;
  }
</style>
