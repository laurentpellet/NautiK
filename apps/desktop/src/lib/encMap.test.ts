import { describe, expect, it } from 'vitest';
import { cm93Warnings, EncMapController, encWarnings, type Cm93ViewState } from './encMap.svelte';
import { EncApi, type EncMapCell, type EncMapIndex } from './services/enc';

function cell(name: string, patch: Partial<EncMapCell> = {}): EncMapCell {
  return {
    path: `${name}.000`,
    name,
    usage: 5,
    scale: 10_000,
    bounds: [-71, 42, -70, 43],
    updatesApplied: 0,
    updatesPending: 0,
    updateError: null,
    cancelled: null,
    error: null,
    ...patch,
  };
}

const view = { west: -70.8, south: 42.2, east: -70.2, north: 42.8 };

function index(cells: EncMapCell[], patch: Partial<EncMapIndex> = {}): EncMapIndex {
  return {
    dir: 'D:\\ENC_ROOT',
    cells,
    truncated: false,
    error: null,
    cm93: null,
    cm93Error: null,
    ...patch,
  };
}

const sounding = (name: string) => ({
  type: 'Feature',
  geometry: { type: 'Point', coordinates: [-70.5, 42.5] },
  properties: { cat: 'sounding', class: 'SOUNDG', usage: 5, cell: name, depth: 3.2, label: '3.2' },
});

/** Moteur simulé : index et entités des cellules ; compte les décodages. */
function fakeApi(cells: EncMapCell[], broken: string[] = []) {
  const calls: string[] = [];
  const api = new EncApi((command, args) => {
    if (command === 'get_enc_map_index') {
      return Promise.resolve({
        dir: 'D:\\ENC_ROOT',
        index: { cells, truncated: false },
        error: null,
      });
    }
    if (command === 'get_enc_cell_features') {
      const path = String(args?.path);
      calls.push(path);
      if (broken.includes(path)) return Promise.reject(new Error('cellule ENC illisible'));
      return Promise.resolve([sounding(path)]);
    }
    return Promise.reject(new Error(`commande inattendue ${command}`));
  });
  return { api, calls };
}

describe('avertissements ENC', () => {
  it('ne dit rien sans dossier ni problème', () => {
    expect(encWarnings(null, view, [], new Map())).toEqual([]);
    expect(encWarnings(index([cell('A')]), view, [cell('A')], new Map())).toEqual([]);
  });

  it('signale dossier, index incomplet, cellules pas à jour, annulées, illisibles', () => {
    const outdated = cell('OLD', { updateError: 'mise à jour 3 manquante' });
    const warnings = encWarnings(
      index(
        [
          outdated,
          cell('ANN', { cancelled: '2022-12-30' }),
          cell('ANN-LOIN', { cancelled: '2022-12-30', bounds: [10, 10, 11, 11] }),
          cell('ERR', { error: 'tronqué' }),
          cell('KO'),
        ],
        { truncated: true, error: 'clé USB retirée' },
      ),
      view,
      [outdated],
      new Map([['KO.000', 'illisible']]),
    );
    expect(warnings).toEqual([
      'Dossier ENC inaccessible : clé USB retirée',
      'Trop de cellules ENC : index incomplet',
      'ENC pas à jour : OLD',
      'ENC annulée, non affichée : ANN',
      'ENC illisible : KO',
      '1 cellule(s) ENC illisible(s) dans le dossier',
    ]);
  });

  it('signale les cellules non affichées faute de place', () => {
    const cells = Array.from({ length: 20 }, (_, i) => cell(`C${i}`));
    const warnings = encWarnings(index(cells), view, cells.slice(0, 16), new Map());
    expect(warnings).toEqual(['4 cellule(s) ENC non affichée(s) : zoomez']);
  });
});

describe('contrôleur des ENC', () => {
  it('charge les cellules de la vue une seule fois', async () => {
    const { api, calls } = fakeApi([cell('A'), cell('LOIN', { bounds: [10, 10, 11, 11] })]);
    const enc = new EncMapController(api);
    await enc.show(view);
    expect(enc.displayed.map((c) => c.name)).toEqual(['A']);
    expect(enc.s57Data.features).toHaveLength(1);
    expect(enc.warnings).toEqual([]);
    await enc.show(view);
    expect(calls).toEqual(['A.000']);
  });

  it('signale une cellule illisible sans bloquer les autres', async () => {
    const { api, calls } = fakeApi([cell('A'), cell('KO')], ['KO.000']);
    const enc = new EncMapController(api);
    await enc.show(view);
    expect(enc.displayed.map((c) => c.name)).toEqual(['A']);
    expect(enc.warnings).toEqual(['ENC illisible : KO']);
    // Pas de nouvel essai tant que l'index n'est pas relu.
    await enc.show(view);
    expect(calls.filter((p) => p === 'KO.000')).toHaveLength(1);
    await enc.refresh();
    await enc.show(view);
    expect(calls.filter((p) => p === 'KO.000')).toHaveLength(2);
  });

  it('masque la couche et signale un moteur indisponible', async () => {
    const { api } = fakeApi([cell('A')]);
    const enc = new EncMapController(api);
    await enc.show(view);
    enc.hide();
    expect(enc.s57Data.features).toEqual([]);
    expect(enc.displayed).toEqual([]);

    const down = new EncMapController(new EncApi(() => Promise.reject(new Error('moteur arrêté'))));
    await down.show(view);
    expect(down.s57Data.features).toEqual([]);
    expect(down.warnings).toEqual(['Cartes ENC indisponibles : moteur arrêté']);
  });

  it('ignore un chargement dépassé par une vue plus récente', async () => {
    const { api } = fakeApi([cell('A')]);
    const enc = new EncMapController(api);
    const first = enc.show(view);
    enc.hide();
    await first;
    expect(enc.s57Data.features).toEqual([]);
  });
});

const cm93Cell = (name: string, usage = 7) =>
  cell(name, { path: `02100600/F/${name}`, usage, scale: 20_000 });

function cm93State(patch: Partial<Cm93ViewState> = {}): Cm93ViewState {
  return { view: null, error: null, displayed: [], info: [], failed: [], ...patch };
}

describe('avertissements CM93', () => {
  it('ne dit rien sans base ni cellule dessinée', () => {
    expect(cm93Warnings(null, cm93State())).toEqual([]);
    expect(cm93Warnings(index([]), cm93State())).toEqual([]);
  });

  it('signale une carte non officielle, ancienne et décalée', () => {
    const shown = [cm93Cell('A.F'), cm93Cell('B.F')];
    const warnings = cm93Warnings(
      index([], { cm93: '.' }),
      cm93State({
        displayed: shown,
        info: [
          { dataDate: '20140602', declaredOffsetM: 0 },
          { dataDate: '19990101', declaredOffsetM: 41.6 },
        ],
      }),
    );
    expect(warnings).toEqual([
      'Cartes CM93 non officielles (données de 01/01/1999) : ne pas naviguer avec elles seules',
      "CM93 : décalage WGS 84 déclaré jusqu'à 42 m, non corrigé",
    ]);
    // Sans date ni décalage : l'avertissement de carte non officielle reste.
    expect(
      cm93Warnings(
        index([]),
        cm93State({ displayed: shown, info: [{ dataDate: null, declaredOffsetM: 0.4 }] }),
      ),
    ).toEqual(['Cartes CM93 non officielles : ne pas naviguer avec elles seules']);
  });

  it('signale base illisible, vue impossible, cellules illisibles ou omises', () => {
    const warnings = cm93Warnings(
      index([], { cm93: '.', cm93Error: 'CM93OBJ.DIC illisible' }),
      cm93State({
        view: { base: '.', cells: [], omitted: 5, truncated: true },
        error: 'moteur arrêté',
        failed: ['02170631.F'],
      }),
    );
    expect(warnings).toEqual([
      'Base CM93 illisible : CM93OBJ.DIC illisible',
      'Base CM93 indisponible : moteur arrêté',
      'CM93 illisible : 02170631.F',
      'Trop de cellules CM93 : index incomplet',
      '5 cellule(s) CM93 non affichée(s) : zoomez',
    ]);
  });
});

describe('contrôleur CM93', () => {
  /** Moteur simulé avec une base CM93 et une ENC officielle. */
  function cm93Api(cm93Cells: EncMapCell[], broken: string[] = []) {
    const calls: string[] = [];
    const api = new EncApi((command, args) => {
      if (command === 'get_enc_map_index') {
        return Promise.resolve({
          dir: 'D:\\ENC_ROOT',
          index: { cells: [cell('S57')], truncated: false },
          error: null,
          cm93: '.',
          cm93Error: null,
        });
      }
      if (command === 'get_enc_cell_features') {
        calls.push(String(args?.path));
        return Promise.resolve([sounding('S57')]);
      }
      if (command === 'get_cm93_view') {
        calls.push('vue');
        return Promise.resolve({ base: '.', cells: cm93Cells, omitted: 0, truncated: false });
      }
      if (command === 'get_cm93_cell_features') {
        const path = String(args?.path);
        calls.push(path);
        if (broken.includes(path)) return Promise.reject(new Error('cellule CM93 tronquée'));
        return Promise.resolve({
          features: [sounding(path), sounding(path)],
          dataDate: '20140602',
          declaredOffsetM: 0,
        });
      }
      return Promise.reject(new Error(`commande inattendue ${command}`));
    });
    return { api, calls };
  }

  it('dessine la CM93 sous les ENC et la charge une seule fois', async () => {
    const { api, calls } = cm93Api([cm93Cell('A.F')]);
    const enc = new EncMapController(api);
    await enc.show(view);
    const cells = enc.layer({ enc: true, cm93: true }).features.map((f) => f.properties.cell);
    expect(cells).toEqual(['02100600/F/A.F', '02100600/F/A.F', 'S57']);
    expect(enc.displayed.map((c) => c.name)).toEqual(['S57']);
    expect(enc.warnings).toEqual([
      'Cartes CM93 non officielles (données de 02/06/2014) : ne pas naviguer avec elles seules',
    ]);
    await enc.show(view);
    expect(calls.filter((c) => c === '02100600/F/A.F')).toHaveLength(1);
    expect(calls.filter((c) => c === 'vue')).toHaveLength(2);
  });

  it('signale une cellule CM93 illisible sans bloquer les autres', async () => {
    const { api } = cm93Api([cm93Cell('A.F'), cm93Cell('KO.F')], ['02100600/F/KO.F']);
    const enc = new EncMapController(api);
    await enc.show(view);
    expect(enc.cm93Data.features).toHaveLength(2);
    expect(enc.s57Data.features).toHaveLength(1);
    expect(enc.warnings).toContain('CM93 illisible : KO.F');
  });

  it('ne demande rien à la base CM93 quand le dossier n’en a pas', async () => {
    const { api, calls } = fakeApi([cell('A')]);
    const enc = new EncMapController(api);
    await enc.show(view);
    expect(calls).toEqual(['A.000']);
  });

  it('compose la couche de chaque volet selon ses cartes', async () => {
    const { api } = cm93Api([cm93Cell('A.F')]);
    const enc = new EncMapController(api);
    await enc.show(view);
    const cellsOf = (enc93: { enc: boolean; cm93: boolean }) =>
      enc.layer(enc93).features.map((f) => f.properties.cell);
    expect(cellsOf({ enc: true, cm93: false })).toEqual(['S57']);
    expect(cellsOf({ enc: false, cm93: true })).toEqual(['02100600/F/A.F', '02100600/F/A.F']);
    expect(cellsOf({ enc: false, cm93: false })).toEqual([]);
  });

  it('ne charge ni ne signale les cartes voulues dans aucun volet', async () => {
    const { api, calls } = cm93Api([cm93Cell('A.F')]);
    const enc = new EncMapController(api);
    await enc.show(view, { enc: true, cm93: false });
    expect(calls).toEqual(['S57.000']);
    expect(enc.cm93Data.features).toEqual([]);
    expect(enc.warnings).toEqual([]);

    const only93 = cm93Api([cm93Cell('A.F')]);
    const enc93 = new EncMapController(only93.api);
    await enc93.show(view, { enc: false, cm93: true });
    expect(only93.calls).toEqual(['vue', '02100600/F/A.F']);
    expect(enc93.s57Data.features).toEqual([]);
    expect(enc93.displayed).toEqual([]);
    expect(enc93.warnings).toHaveLength(1);
    expect(enc93.warnings[0]).toContain('CM93 non officielles');
  });
});
