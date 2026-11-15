import { describe, expect, it } from 'vitest';
import { followBoat, loadFollowBoat, setFollowBoat } from './followBoat.svelte';

const memory = () => {
  const data = new Map<string, string>();
  return {
    data,
    getItem: (k: string) => data.get(k) ?? null,
    setItem: (k: string, v: string) => void data.set(k, v),
  };
};

describe('défilement de la carte avec le bateau', () => {
  it('est activé par défaut et pour toute valeur autre que false', () => {
    const storage = memory();
    expect(loadFollowBoat(storage)).toBe(true);
    storage.data.set('nautik.followBoat', 'n’importe quoi');
    expect(loadFollowBoat(storage)).toBe(true);
    const broken = {
      getItem: () => {
        throw new Error('bloqué');
      },
    };
    expect(loadFollowBoat(broken)).toBe(true);
  });

  it('se désactive, se réactive et se mémorise', () => {
    const storage = memory();
    setFollowBoat(false, storage);
    expect(followBoat.enabled).toBe(false);
    expect(loadFollowBoat(storage)).toBe(false);
    setFollowBoat(true, storage);
    expect(followBoat.enabled).toBe(true);
    expect(loadFollowBoat(storage)).toBe(true);
    // Stockage indisponible : l'option change quand même.
    setFollowBoat(false, {
      setItem: () => {
        throw new Error('plein');
      },
    });
    expect(followBoat.enabled).toBe(false);
    setFollowBoat(true, undefined);
  });
});
