/**
 * Abonnement à un événement du moteur pour la durée de vie d'un effet
 * Svelte : le désabonnement est garanti même si l'effet est détruit
 * avant la fin de l'abonnement.
 *
 * @returns la fonction de nettoyage à retourner depuis `$effect`.
 */
export function holdSubscription(
  subscribe: () => Promise<() => void>,
  onError: (error: unknown) => void,
): () => void {
  let unlisten: (() => void) | null = null;
  let disposed = false;
  subscribe()
    .then((u) => {
      if (disposed) u();
      else unlisten = u;
    })
    .catch(onError);
  return () => {
    disposed = true;
    unlisten?.();
  };
}
