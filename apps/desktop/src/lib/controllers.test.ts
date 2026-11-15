import { afterEach, describe, expect, it, vi } from 'vitest';
import { MarksController, boundsAround, nextMarkName } from './marks.svelte';
import { MarksApi, type Mark } from './services/marks';
import { ReplayController, indexAt } from './replay.svelte';
import type { TrackProfile } from './services/trackProfile';
import { LOG_ALARM_EVENTS, LogbookController, logRows } from './logbook.svelte';
import { LOG_PAGE, LogbookApi, parseLogEntry } from './services/logbook';
import type { AlarmEvent } from './services/alarms';
import { marksData } from './map/marks';
import { replayData } from './map/replay';

const pos = { latitudeDeg: -17.5, longitudeDeg: -149.6 };

function rawMark(id: string, name: string) {
  return {
    id,
    name,
    position: pos,
    icon: 'flag',
    note: null,
    createdAt: '2026-11-14T06:00:00Z',
    updatedAt: '2026-11-14T06:00:00Z',
  };
}

describe('contrôleur des marques', () => {
  it('propose un nom libre et cadre autour d’un point', () => {
    expect(nextMarkName([])).toBe('Marque 1');
    expect(nextMarkName([{ name: 'Marque 2' }])).toBe('Marque 3');
    expect(nextMarkName([{ name: 'A' }, { name: 'Marque 3' }])).toBe('Marque 4');
    const b = boundsAround({ latitudeDeg: 89.995, longitudeDeg: 179.995 });
    expect(b.north).toBe(90);
    expect(b.east).toBeCloseTo(180.005);
  });

  it('crée, garde l’édition sur refus du moteur, puis enregistre', async () => {
    let stored: unknown[] = [];
    let refuse = true;
    const call = vi.fn((command: string, args?: Record<string, unknown>) => {
      if (command === 'list_marks') return Promise.resolve(stored);
      if (command === 'save_mark') {
        if (refuse) return Promise.reject(new Error('nom vide'));
        stored = [rawMark('m1', String(args?.name))];
        return Promise.resolve(stored[0]);
      }
      if (command === 'import_kml') return Promise.resolve({ imported: 1, warnings: ['x'] });
      return Promise.resolve(true);
    });
    const c = new MarksController(new MarksApi(call));
    await c.refresh();
    c.startNew(pos, 'danger');
    expect(c.draft?.name).toBe('Marque 1');
    await c.save();
    expect(c.error).toBe('nom vide');
    expect(c.draft).not.toBeNull();
    refuse = false;
    await c.save();
    expect(c.draft).toBeNull();
    expect(c.marks).toHaveLength(1);
    c.edit('m1');
    expect(c.draft?.id).toBe('m1');
    await c.remove('m1');
    expect(c.draft).toBeNull();
    await c.importKml();
    expect(c.notice).toMatch(/1 marque\(s\).*1 élément\(s\) écarté\(s\)/);
  });

  it('produit les points de la carte', () => {
    const data = marksData([
      {
        ...rawMark('m1', 'Passe'),
        icon: 'danger',
        createdAt: 0,
        updatedAt: 0,
      } as Mark,
    ]);
    expect(data.features[0]?.geometry.coordinates).toEqual([-149.6, -17.5]);
    expect(data.features[0]?.properties).toMatchObject({ icon: 'danger', label: 'Passe' });
  });
});

function profile(n: number): TrackProfile {
  return {
    stats: {
      distanceM: 0,
      durationS: n * 10,
      movingS: 0,
      averageMps: null,
      movingAverageMps: null,
      maxMps: null,
      minDepthM: null,
    },
    points: Array.from({ length: n }, (_, i) => ({
      at: i * 10_000,
      position: { latitudeDeg: 0, longitudeDeg: i * 0.01 },
      speedMps: i === 0 ? null : 2,
      depthM: null,
      distanceM: i * 100,
    })),
  };
}

function point(i: number) {
  const p = profile(2).points[i];
  if (!p) throw new Error(`point ${i}`);
  return p;
}

describe('relecture de trace', () => {
  afterEach(() => vi.useRealTimers());

  it('trouve le point affiché par dichotomie', () => {
    const pts = profile(5).points;
    expect(indexAt(pts, -1)).toBe(0);
    expect(indexAt(pts, 0)).toBe(0);
    expect(indexAt(pts, 25_000)).toBe(2);
    expect(indexAt(pts, 1e9)).toBe(4);
    expect(indexAt([], 0)).toBe(-1);
  });

  it('charge, rejoue en accéléré et s’arrête au bout', async () => {
    vi.useFakeTimers();
    const r = new ReplayController(() => Promise.resolve(profile(5)));
    await r.load('t1', 'Sortie');
    expect(r.active).toBe(true);
    r.setRate(10);
    r.play();
    // 10 s de trace par seconde : un point par seconde.
    vi.advanceTimersByTime(2_000);
    expect(r.index).toBe(2);
    vi.advanceTimersByTime(5_000);
    expect(r.index).toBe(4);
    expect(r.playing).toBe(false);
    r.setIndex(99);
    expect(r.index).toBe(4);
    r.setRate(7);
    expect(r.rate).toBe(10);
    r.close();
    expect(r.active).toBe(false);
  });

  it('signale une erreur de chargement et ignore une réponse périmée', async () => {
    let resolveFirst: (p: TrackProfile) => void = () => {};
    const fetch = vi
      .fn<(id: string) => Promise<TrackProfile>>()
      .mockImplementationOnce(() => new Promise((res) => (resolveFirst = res)))
      .mockImplementationOnce(() => Promise.reject(new Error('trace introuvable')));
    const r = new ReplayController(fetch);
    const first = r.load('a', 'A');
    await r.load('b', 'B');
    resolveFirst(profile(3));
    await first;
    expect(r.trackId).toBe('b');
    expect(r.profile).toBeNull();
    expect(r.error).toBe('trace introuvable');
  });

  it('colore les segments selon la vitesse, gris sans vitesse', () => {
    const data = replayData(profile(3).points, 1);
    const segments = data.features.filter((f) => f.properties.kind === 'segment');
    expect(segments).toHaveLength(2);
    expect(segments[0]?.properties.speedKn).toBeCloseTo(2 / (1852 / 3600));
    const unknown = replayData(
      [
        { ...point(0), speedMps: null },
        { ...point(1), speedMps: null },
      ],
      null,
    );
    expect(unknown.features[0]?.properties.speedKn).toBe(-1);
    expect(unknown.features.some((f) => f.properties.kind === 'cursor')).toBe(false);
    // Antiméridien : la longitude suivante est déroulée.
    const wrap = replayData(
      [
        { ...point(0), position: { latitudeDeg: 0, longitudeDeg: 179.9 } },
        { ...point(1), position: { latitudeDeg: 0, longitudeDeg: -179.9 } },
      ],
      0,
    );
    const line = wrap.features[0]?.geometry;
    expect(line?.type === 'LineString' ? line.coordinates[1]?.[0] : null).toBeCloseTo(180.1);
  });
});

function rawEntry(id: string, at: string) {
  return {
    id,
    at,
    kind: 'manual',
    text: id,
    position: null,
    positionStale: false,
    sogMps: null,
    cogRad: null,
    headingRad: null,
    twsMps: null,
    twdRad: null,
    pressurePa: null,
    depthM: null,
    simulated: false,
  };
}

describe('contrôleur du journal', () => {
  it('charge, pagine, ajoute et règle l’intervalle', async () => {
    const page = Array.from({ length: LOG_PAGE }, (_, i) =>
      rawEntry(`e${i}`, new Date(Date.UTC(2026, 10, 14) - i * 60_000).toISOString()),
    );
    const call = vi.fn((command: string, args?: Record<string, unknown>) => {
      if (command === 'list_log_entries') {
        return Promise.resolve(args?.before ? [rawEntry('old', '2026-11-01T00:00:00Z')] : page);
      }
      if (command === 'get_logbook_settings') return Promise.resolve({ autoIntervalMin: 60 });
      if (command === 'set_logbook_settings') return Promise.resolve(args?.settings);
      if (command === 'add_log_entry') {
        return args?.text === ''
          ? Promise.reject(new Error('texte vide'))
          : Promise.resolve(rawEntry('new', '2026-11-14T09:00:00Z'));
      }
      return Promise.resolve(true);
    });
    const c = new LogbookController(new LogbookApi(call));
    await c.refresh();
    expect(c.entries).toHaveLength(LOG_PAGE);
    expect(c.more).toBe(true);
    expect(c.autoIntervalMin).toBe(60);
    await c.loadMore();
    expect(c.entries.at(-1)?.id).toBe('old');
    expect(c.more).toBe(false);
    expect(await c.add('')).toBe(false);
    expect(c.error).toBe('texte vide');
    expect(await c.add('Ris pris')).toBe(true);
    expect(c.entries[0]?.id).toBe('new');
    await c.setInterval(15);
    expect(c.autoIntervalMin).toBe(15);
    await c.remove('new');
    expect(c.entries[0]?.id).toBe('e0');
    await c.exportCsv();
    expect(c.notice).toBe('Journal exporté');
  });

  it('intercale les alarmes par date, sans devancer les pages non chargées', () => {
    const entry = (id: string, iso: string) => parseLogEntry(rawEntry(id, iso));
    const alarm = (id: string, iso: string): AlarmEvent => ({
      id,
      alarm: 'anchor',
      kind: 'activated',
      at: Date.parse(iso),
      message: 'Dérapage',
      distanceM: 40,
      simulated: false,
    });
    const entries = [entry('e1', '2026-11-14T10:00:00Z'), entry('e2', '2026-11-14T08:00:00Z')];
    const alarms = [alarm('a1', '2026-11-14T09:00:00Z'), alarm('a0', '2026-11-13T09:00:00Z')];
    expect(logRows(entries, alarms, false).map((r) => r.id)).toEqual([
      'e1',
      'alarm-a1',
      'e2',
      'alarm-a0',
    ]);
    // Entrées plus anciennes à charger : l'alarme antérieure attend sa page.
    expect(logRows(entries, alarms, true).map((r) => r.id)).toEqual(['e1', 'alarm-a1', 'e2']);
    expect(logRows([], alarms, false)).toHaveLength(2);
  });

  it('charge l’historique des alarmes et signale son échec sans bloquer le journal', async () => {
    const call = vi.fn((command: string) => {
      if (command === 'list_log_entries') {
        return Promise.resolve([rawEntry('e1', '2026-11-14T10:00:00Z')]);
      }
      if (command === 'get_logbook_settings') return Promise.resolve({ autoIntervalMin: 0 });
      return Promise.resolve(true);
    });
    const events = vi.fn(() => Promise.reject(new Error('base verrouillée')));
    const c = new LogbookController(new LogbookApi(call), events);
    await c.refresh();
    expect(events).toHaveBeenCalledWith(LOG_ALARM_EVENTS);
    expect(c.entries).toHaveLength(1);
    expect(c.alarmsError).toBe('Alarmes illisibles : base verrouillée');
    expect(c.rows.map((r) => r.id)).toEqual(['e1']);
  });
});
