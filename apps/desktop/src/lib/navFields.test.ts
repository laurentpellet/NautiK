import { beforeEach, describe, expect, it } from 'vitest';
import {
  NAV_FIELDS,
  defaultNavFields,
  loadNavFields,
  moveNavField,
  navBar,
  parseNavFields,
  resetNavFields,
  setNavFieldShown,
  shownNavFields,
} from './navFields.svelte';

const memory = () => {
  const data = new Map<string, string>();
  return {
    data,
    getItem: (k: string) => data.get(k) ?? null,
    setItem: (k: string, v: string) => void data.set(k, v),
  };
};

describe('valeurs de la barre Navigation', () => {
  beforeEach(() => resetNavFields(undefined));

  it('affiche toutes les valeurs dans l’ordre d’origine par défaut', () => {
    expect(shownNavFields(defaultNavFields())).toEqual(NAV_FIELDS.map((f) => f.id));
    expect(loadNavFields(memory())).toEqual(defaultNavFields());
  });

  it('valide une liste mémorisée : inconnues et doublons ignorés, manquantes ajoutées', () => {
    const list = parseNavFields([
      { id: 'rudder', shown: true },
      { id: 'sog', shown: false },
      { id: 'inconnu', shown: true },
      { id: 'rudder', shown: false },
      'texte',
    ]);
    expect(list.slice(0, 2)).toEqual([
      { id: 'rudder', shown: true },
      { id: 'sog', shown: false },
    ]);
    expect(list).toHaveLength(NAV_FIELDS.length);
    expect(list.slice(2).every((f) => f.shown)).toBe(true);
  });

  it('revient au défaut si rien n’est lisible ou si tout est masqué', () => {
    expect(parseNavFields('x')).toEqual(defaultNavFields());
    expect(parseNavFields(NAV_FIELDS.map((f) => ({ id: f.id, shown: false })))).toEqual(
      defaultNavFields(),
    );
    const broken = {
      getItem: () => {
        throw new Error('bloqué');
      },
    };
    expect(loadNavFields(broken)).toEqual(defaultNavFields());
    const storage = memory();
    storage.data.set('nautik.navBar.fields', '{pas du json');
    expect(loadNavFields(storage)).toEqual(defaultNavFields());
  });

  it('masque, réordonne et mémorise ; garde au moins une valeur', () => {
    const storage = memory();
    expect(setNavFieldShown('depth', false, storage)).toBe(true);
    expect(shownNavFields(navBar.fields)).not.toContain('depth');
    expect(moveNavField('rudder', -1, storage)).toBe(true);
    expect(navBar.fields.at(-1)?.id).toBe('awa');
    expect(moveNavField('sog', -1, storage)).toBe(false);
    expect(moveNavField('awa', 1, storage)).toBe(false);
    expect(loadNavFields(storage)).toEqual(navBar.fields);

    for (const f of NAV_FIELDS) setNavFieldShown(f.id, false, storage);
    expect(shownNavFields(navBar.fields)).toHaveLength(1);
  });
});
