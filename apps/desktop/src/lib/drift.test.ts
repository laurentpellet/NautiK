import { describe, expect, it } from 'vitest';
import {
  DEFAULT_DRIFT_MINUTES,
  driftProjection,
  loadDriftMinutes,
  parseDriftMinutes,
  setDriftMinutes,
} from './drift.svelte';

const store = (value: string | null) => ({ getItem: () => value });

describe('durée de projection des lignes de dérive', () => {
  it('vaut 5 minutes par défaut', () => {
    expect(DEFAULT_DRIFT_MINUTES).toBe(5);
    expect(loadDriftMinutes(store(null))).toBe(5);
    expect(loadDriftMinutes(undefined)).toBe(5);
  });

  it('accepte un entier de 1 à 30 minutes', () => {
    expect(parseDriftMinutes(1)).toBe(1);
    expect(parseDriftMinutes('30')).toBe(30);
    expect(loadDriftMinutes(store('12'))).toBe(12);
  });

  it('refuse les valeurs illisibles ou hors bornes', () => {
    for (const bad of [0, 31, -2, 2.5, Number.NaN, '', ' ', 'abc', null, undefined, {}]) {
      expect(parseDriftMinutes(bad)).toBeNull();
    }
    expect(loadDriftMinutes(store('0'))).toBe(5);
    expect(loadDriftMinutes(store('n'))).toBe(5);
    const broken = {
      getItem: () => {
        throw new Error('stockage bloqué');
      },
    };
    expect(loadDriftMinutes(broken)).toBe(5);
  });

  it('mémorise un choix valide et garde l’ancien sinon', () => {
    let saved: string | null = null;
    const storage = { setItem: (_k: string, v: string) => (saved = v) };
    expect(setDriftMinutes('3', storage)).toBe(true);
    expect(driftProjection.minutes).toBe(3);
    expect(saved).toBe('3');
    expect(setDriftMinutes('99', storage)).toBe(false);
    expect(driftProjection.minutes).toBe(3);
    const failing = {
      setItem: () => {
        throw new Error('plein');
      },
    };
    expect(setDriftMinutes(7, failing)).toBe(true);
    expect(driftProjection.minutes).toBe(7);
    setDriftMinutes(DEFAULT_DRIFT_MINUTES, storage);
  });
});
