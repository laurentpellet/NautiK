<script lang="ts">
  // Panneau de l'onglet Carte, déplaçable par sa poignée (souris, doigt ou
  // flèches du clavier) et masquable. Double-clic sur la poignée : retour à
  // la place par défaut. La poignée reste toujours dans la zone de la carte.
  import { onMount, type Snippet } from 'svelte';
  import { clampPosition, type PanelState, type Rect } from '../panels';

  interface Props {
    title: string;
    panel: PanelState;
    /** Zone de la carte (repère de la fenêtre), pour borner le déplacement. */
    area: () => Rect | null;
    onmove: (x: number, y: number) => void;
    onreset: () => void;
    onhide: () => void;
    /** Poignée latérale fine (barre) plutôt qu'un bandeau titre. */
    compact?: boolean;
    /** Contenu déjà encadré (carte) : simple onglet de poignée au-dessus. */
    bare?: boolean;
    children: Snippet;
  }

  let {
    title,
    panel,
    area,
    onmove,
    onreset,
    onhide,
    compact = false,
    bare = false,
    children,
  }: Props = $props();

  let element: HTMLElement;
  /** Position pendant le glissement (non enregistrée avant le relâchement). */
  let dragging = $state<{ x: number; y: number } | null>(null);
  let offset = { x: 0, y: 0 };

  const position = $derived(dragging ?? (panel.x !== null && panel.y !== null ? panel : null));

  function bounded(x: number, y: number): { x: number; y: number } {
    const zone = area();
    return zone ? clampPosition(x, y, { width: element.offsetWidth }, zone) : { x, y };
  }

  function start(event: PointerEvent): void {
    if (event.button !== 0) return;
    const rect = element.getBoundingClientRect();
    offset = { x: event.clientX - rect.left, y: event.clientY - rect.top };
    dragging = { x: rect.left, y: rect.top };
    (event.currentTarget as HTMLElement).setPointerCapture?.(event.pointerId);
    event.preventDefault();
  }

  function move(event: PointerEvent): void {
    if (!dragging) return;
    dragging = bounded(event.clientX - offset.x, event.clientY - offset.y);
  }

  function end(): void {
    if (!dragging) return;
    const { x, y } = dragging;
    dragging = null;
    onmove(x, y);
  }

  // Position mémorisée sur un autre écran ou fenêtre rétrécie : le
  // panneau est ramené dans la zone de la carte.
  function reclamp(): void {
    if (dragging || panel.x === null || panel.y === null) return;
    const next = bounded(panel.x, panel.y);
    if (next.x !== panel.x || next.y !== panel.y) onmove(next.x, next.y);
  }

  onMount(reclamp);

  const STEP = 20;

  function key(event: KeyboardEvent): void {
    const delta: Record<string, [number, number]> = {
      ArrowLeft: [-STEP, 0],
      ArrowRight: [STEP, 0],
      ArrowUp: [0, -STEP],
      ArrowDown: [0, STEP],
    };
    const d = delta[event.key];
    if (!d) return;
    event.preventDefault();
    const rect = element.getBoundingClientRect();
    const next = bounded(rect.left + d[0], rect.top + d[1]);
    onmove(next.x, next.y);
  }
</script>

<svelte:window onresize={reclamp} />

<section
  bind:this={element}
  class="floating"
  class:compact
  class:bare
  class:moved={position !== null}
  class:dragging={dragging !== null}
  style={position ? `left: ${position.x}px; top: ${position.y}px;` : undefined}
  aria-label={title}
  data-panel={title}
>
  <div
    class="handle"
    role="button"
    tabindex="0"
    title="Déplacer « {title} » (double-clic : remettre en place)"
    aria-label="Déplacer {title}"
    onpointerdown={start}
    onpointermove={move}
    onpointerup={end}
    onpointercancel={end}
    ondblclick={onreset}
    onkeydown={key}
  >
    <span class="grip" aria-hidden="true">⠿</span>
    {#if !compact}<span class="title">{title}</span>{/if}
  </div>
  <div class="content">
    {@render children()}
  </div>
  <button type="button" class="hide" aria-label="Masquer {title}" title="Masquer" onclick={onhide}
    >×</button
  >
</section>

<style>
  .floating {
    position: relative;
    z-index: 2;
    display: flex;
    flex-direction: column;
    min-height: 0;
    border-radius: 0.5rem;
    background: var(--panel-bg);
    color: var(--fg);
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.25);
  }

  .floating.moved {
    position: fixed;
    z-index: 3;
    max-height: calc(100vh - 6rem);
  }

  .floating.dragging {
    z-index: 4;
    opacity: 0.85;
    user-select: none;
  }

  .handle {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    min-height: 1.4rem;
    padding: 0 2rem 0 0.5rem;
    border-bottom: 1px solid var(--border);
    color: var(--muted);
    font-size: 0.7rem;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    cursor: grab;
    touch-action: none;
  }

  .dragging .handle {
    cursor: grabbing;
  }

  .handle:focus-visible {
    outline: 2px solid var(--fg);
    outline-offset: -2px;
  }

  .content {
    min-height: 0;
    overflow: auto;
  }

  .hide {
    position: absolute;
    top: 0.1rem;
    right: 0.25rem;
    width: 1.3rem;
    height: 1.2rem;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--muted);
    font-size: 1rem;
    line-height: 1;
    cursor: pointer;
  }

  .hide:hover,
  .hide:focus-visible {
    color: var(--fg);
  }

  /* Barre : poignée verticale à gauche, bouton de masquage à droite. */
  .compact {
    flex-direction: row;
    align-items: stretch;
  }

  .compact .handle {
    padding: 0 0.35rem;
    border-bottom: 0;
    border-right: 1px solid var(--border);
  }

  .compact .content {
    flex: 1;
    overflow: visible;
  }

  .compact .hide {
    position: static;
    align-self: center;
    margin: 0 0.25rem;
  }

  /* Carte déjà encadrée : petit onglet de poignée au-dessus, à droite. */
  .bare {
    align-items: flex-end;
    background: transparent;
    box-shadow: none;
  }

  .bare .handle {
    min-height: 1.2rem;
    border: 1px solid var(--border);
    border-bottom: 0;
    border-radius: 0.4rem 0.4rem 0 0;
    background: var(--panel-bg);
    margin-right: 0.6rem;
  }

  .bare .hide {
    right: 0.7rem;
  }

  .bare .content {
    overflow: visible;
  }
</style>
