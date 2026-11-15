<script lang="ts">
  // Bouton Homme à la mer (en-tête, à droite de l'état des données) :
  // bouée rouge, clignotante tant qu'un MOB est en cours.
  interface Props {
    /** Homme à la mer en cours. */
    active: boolean;
    onmob: () => void;
  }

  let { active, onmob }: Props = $props();
</script>

<button
  type="button"
  class="mob"
  class:on={active}
  aria-label="Homme à la mer"
  title={active
    ? 'Homme à la mer en cours (le point n’est pas déplacé)'
    : 'Homme à la mer : marque le point MOB à la position du bateau (Ctrl+M)'}
  onclick={onmob}
>
  <!-- Bouée couronne lancée vers la personne à la mer. -->
  <svg class="buoy" viewBox="0 0 32 32" aria-hidden="true" focusable="false">
    <path class="wave" d="M1 29.5q2.5-2.2 5 0t5 0t5 0t5 0t5 0t5 0" />
    <path class="rope" d="M8.7 21.3q-4.5 1.5-5.5 6.7" />
    <circle class="ring" cx="17" cy="13" r="9" />
    <path
      class="band"
      d="M21.2 17.2L25.5 21.5M12.8 17.2L8.5 21.5M12.8 8.8L8.5 4.5M21.2 8.8L25.5 4.5"
    />
    <circle class="edge" cx="17" cy="13" r="12" />
    <circle class="edge" cx="17" cy="13" r="6" />
  </svg>
</button>

<style>
  .mob {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0.1rem 0.25rem;
    border: 2px solid #e53935;
    border-radius: 0.35rem;
    background: #fff;
    cursor: pointer;
  }

  .buoy {
    width: 1.9rem;
    height: 1.9rem;
  }

  .buoy .ring {
    fill: none;
    stroke: #e53935;
    stroke-width: 6;
  }

  .buoy .band {
    fill: none;
    stroke: #fff;
    stroke-width: 4.2;
  }

  .buoy .edge {
    fill: none;
    stroke: #8e1b18;
    stroke-width: 0.8;
  }

  .buoy .rope {
    fill: none;
    stroke: #6d4c41;
    stroke-width: 1.3;
    stroke-linecap: round;
  }

  .buoy .wave {
    fill: none;
    stroke: #1e88e5;
    stroke-width: 1.4;
    stroke-linecap: round;
  }

  .mob:hover,
  .mob:focus-visible {
    background: #ffebee;
  }

  .mob.on {
    animation: mob-blink 1s steps(2, start) infinite;
  }

  @keyframes mob-blink {
    to {
      background: #e53935;
    }
  }
</style>
