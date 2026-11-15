/**
 * Défilement de la carte avec le bateau (Paramètres → Général).
 *
 * Option activée : le bouton « Centrer sur le bateau » devient un
 * interrupteur ; actif, la carte suit chaque nouvelle position, et un
 * déplacement de la carte à la main l'arrête. Option désactivée : le
 * bouton centre une seule fois. Préférence mémorisée localement, activée
 * par défaut ; seule la valeur `false` la désactive.
 */

const STORAGE_KEY = 'nautik.followBoat';

export function loadFollowBoat(
  storage: Pick<Storage, 'getItem'> | undefined = globalThis.localStorage,
): boolean {
  try {
    return storage?.getItem(STORAGE_KEY) !== 'false';
  } catch {
    return true;
  }
}

/** Option courante (réactive). */
export const followBoat: { enabled: boolean } = $state({ enabled: loadFollowBoat() });

/** Active ou désactive l'option et la mémorise. */
export function setFollowBoat(
  enabled: boolean,
  storage: Pick<Storage, 'setItem'> | undefined = globalThis.localStorage,
): void {
  followBoat.enabled = enabled;
  try {
    storage?.setItem(STORAGE_KEY, String(enabled));
  } catch {
    // Préférence non mémorisée : sans conséquence.
  }
}
