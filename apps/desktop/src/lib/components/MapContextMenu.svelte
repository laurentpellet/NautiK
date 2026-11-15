<script lang="ts">
  // Menu du clic droit sur la carte : position visée avec distance et
  // relèvement depuis le bateau (moteur), « naviguer jusqu'ici », « bateau
  // ici » (simulateur seulement) et mesure depuis ce point. Se ferme par
  // Échap, un clic ailleurs ou après un choix.
  import { formatBearing, formatDistance, formatLatitude, formatLongitude } from '../format';
  import type { FromBoat } from '../mapTools.svelte';
  import type { LatLon } from '../services/routing';

  interface Props {
    position: LatLon;
    /** Point écran, relatif au volet de carte. */
    x: number;
    y: number;
    /** Le bateau peut être placé (mode simulateur). */
    canPlaceBoat: boolean;
    /** La mesure est possible (pas de tracé de route en cours). */
    canMeasure?: boolean;
    onplaceboat: (position: LatLon) => void;
    onmeasure: (position: LatLon) => void;
    /** Pose une marque personnelle à ce point. */
    onmark?: (position: LatLon) => void;
    /** Distance et relèvement depuis le bateau ; absent : non affichés. */
    fromBoat?: FromBoat;
    /** Crée une route directe du bateau à ce point et l'active. */
    ongoto?: (position: LatLon) => void;
    /** Raison du refus de « naviguer jusqu'ici » ; `null` : possible. */
    goToBlocked?: string | null;
    onclose: () => void;
  }

  let {
    position,
    x,
    y,
    canPlaceBoat,
    canMeasure = true,
    onplaceboat,
    onmeasure,
    onmark,
    fromBoat,
    ongoto,
    goToBlocked = null,
    onclose,
  }: Props = $props();

  let menu: HTMLDivElement;

  // Focus sur le premier choix actif, pour le clavier.
  $effect(() => {
    menu.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus();
  });

  function onWindowPointer(event: PointerEvent) {
    if (!menu.contains(event.target as Node)) onclose();
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.stopPropagation();
      onclose();
    }
  }

  /** Position lue avant la fermeture (le menu disparaît avec elle). */
  function choose(action: (position: LatLon) => void) {
    const at = position;
    onclose();
    action(at);
  }
</script>

<svelte:window onpointerdown={onWindowPointer} />

<!-- Positionné au point cliqué, sans sortir du volet. -->
<div
  class="menu"
  role="menu"
  tabindex="-1"
  aria-label="Menu de la carte"
  bind:this={menu}
  style:left={`min(${x}px, calc(100% - 15rem))`}
  style:top={`min(${y}px, calc(100% - 14rem))`}
  onkeydown={onKeydown}
  oncontextmenu={(e) => e.preventDefault()}
>
  <p class="where" data-testid="context-position">
    {formatLatitude(position.latitudeDeg)}<br />{formatLongitude(position.longitudeDeg)}
    {#if fromBoat}
      <span class="range" data-testid="context-from-boat">
        {#if fromBoat.kind === 'ok'}
          <strong>{formatDistance(fromBoat.distanceM)}</strong> ·
          <strong>{formatBearing(fromBoat.bearingRad)}</strong> depuis le bateau
        {:else if fromBoat.kind === 'pending'}
          Distance et relèvement…
        {:else}
          {fromBoat.reason}
        {/if}
      </span>
    {/if}
  </p>
  {#if ongoto}
    <button
      type="button"
      role="menuitem"
      disabled={goToBlocked !== null}
      title={goToBlocked ?? 'Créer une route directe du bateau à ce point et l’activer'}
      onclick={() => choose(ongoto)}>Naviguer jusqu'ici</button
    >
  {/if}
  <button
    type="button"
    role="menuitem"
    disabled={!canPlaceBoat}
    title={canPlaceBoat
      ? 'Placer le bateau simulé à ce point'
      : 'Disponible seulement avec la source Simulateur'}
    onclick={() => choose(onplaceboat)}>Bateau ici</button
  >
  <button
    type="button"
    role="menuitem"
    disabled={!canMeasure}
    title={canMeasure
      ? 'Mesurer à partir de ce point'
      : 'Indisponible pendant le tracé d’une route'}
    onclick={() => choose(onmeasure)}>Mesurer depuis ici</button
  >
  {#if onmark}
    <button
      type="button"
      role="menuitem"
      title="Poser une marque personnelle à ce point"
      onclick={() => choose(onmark)}>Marque ici</button
    >
  {/if}
</div>

<style>
  .menu {
    position: absolute;
    z-index: 5;
    display: flex;
    flex-direction: column;
    min-width: 12rem;
    padding: 0.3rem;
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    background: var(--panel-bg);
    color: var(--fg);
    box-shadow: 0 4px 14px rgb(0 0 0 / 30%);
    font-size: 0.85rem;
  }

  .where {
    margin: 0 0 0.25rem;
    padding: 0.2rem 0.4rem 0.35rem;
    border-bottom: 1px solid var(--border);
    color: var(--muted);
    font-size: 0.78rem;
    font-variant-numeric: tabular-nums;
  }

  .range {
    display: block;
    margin-top: 0.25rem;
  }

  .range strong {
    color: var(--fg);
    font-size: 0.9rem;
  }

  button {
    padding: 0.35rem 0.5rem;
    border: 0;
    border-radius: 0.3rem;
    background: transparent;
    color: var(--fg);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  button:hover:not(:disabled),
  button:focus-visible {
    background: var(--button-bg);
  }

  button:disabled {
    opacity: 0.45;
    cursor: default;
  }
</style>
