import { describe, expect, it, vi } from 'vitest';
import { MarksApi, parseMark, parseMarksImport } from './marks';
import { LogbookApi, parseLogEntry, parseLogbookSettings } from './logbook';
import { parsePlace, searchPlaces } from './search';
import { getTrackProfile, parseTrackProfile } from './trackProfile';
import { EngineProtocolError } from './validate';

const rawMark = {
  id: '5b1f6c1e-0000-4000-8000-000000000001',
  name: 'Passe',
  position: { latitudeDeg: -17.5, longitudeDeg: -149.6 },
  icon: 'danger',
  note: null,
  createdAt: '2026-11-14T06:00:00Z',
  updatedAt: '2026-11-14T06:00:00Z',
};

describe('marques', () => {
  it('valide les marques reçues', () => {
    const m = parseMark(rawMark);
    expect(m.icon).toBe('danger');
    expect(m.createdAt).toBe(Date.UTC(2026, 10, 14, 6));
    for (const bad of [
      { ...rawMark, icon: 'licorne' },
      { ...rawMark, name: 'x'.repeat(101) },
      { ...rawMark, position: { latitudeDeg: 91, longitudeDeg: 0 } },
      { ...rawMark, createdAt: 'hier' },
      null,
    ]) {
      expect(() => parseMark(bad)).toThrow(EngineProtocolError);
    }
    expect(parseMarksImport({ imported: 2, warnings: ['ligne ignorée'] }).imported).toBe(2);
    expect(() => parseMarksImport({ imported: -1, warnings: [] })).toThrow(EngineProtocolError);
  });

  it('appelle les commandes du moteur', async () => {
    const call = vi.fn((command: string) =>
      Promise.resolve(
        command === 'list_marks'
          ? [rawMark]
          : command === 'save_mark'
            ? rawMark
            : command === 'import_kml'
              ? null
              : true,
      ),
    );
    const api = new MarksApi(call);
    expect(await api.list()).toHaveLength(1);
    await api.save({
      id: null,
      name: 'Passe',
      position: rawMark.position,
      icon: 'danger',
      note: '  ',
    });
    expect(call).toHaveBeenCalledWith('save_mark', {
      id: null,
      name: 'Passe',
      position: rawMark.position,
      icon: 'danger',
      note: null,
    });
    expect(await api.importKml()).toBeNull();
    expect(await api.exportKml()).toBe(true);
  });
});

const rawEntry = {
  id: 'e1',
  at: '2026-11-14T08:00:00Z',
  kind: 'auto',
  text: null,
  position: { latitudeDeg: -17.5, longitudeDeg: -149.6 },
  positionStale: true,
  sogMps: 2.5,
  cogRad: 1,
  headingRad: null,
  twsMps: null,
  twdRad: null,
  pressurePa: 101300,
  depthM: null,
  simulated: false,
};

describe('journal de bord', () => {
  it('valide les entrées et les réglages', () => {
    const e = parseLogEntry(rawEntry);
    expect(e.positionStale).toBe(true);
    expect(e.headingRad).toBeNull();
    for (const bad of [
      { ...rawEntry, kind: 'robot' },
      { ...rawEntry, positionStale: 'oui' },
      { ...rawEntry, sogMps: 'vite' },
      { ...rawEntry, text: 'x'.repeat(2001) },
    ]) {
      expect(() => parseLogEntry(bad)).toThrow(EngineProtocolError);
    }
    expect(parseLogbookSettings({ autoIntervalMin: 15 }).autoIntervalMin).toBe(15);
    expect(() => parseLogbookSettings({ autoIntervalMin: 7 })).toThrow(EngineProtocolError);
  });

  it('pagine par date et transmet les réglages', async () => {
    const call = vi.fn((command: string) =>
      Promise.resolve(
        command === 'list_log_entries'
          ? [rawEntry]
          : command === 'add_log_entry'
            ? { ...rawEntry, kind: 'manual', text: 'Ris' }
            : command.includes('settings')
              ? { autoIntervalMin: 30 }
              : true,
      ),
    );
    const api = new LogbookApi(call);
    await api.list(10, Date.UTC(2026, 10, 14));
    expect(call).toHaveBeenCalledWith('list_log_entries', {
      limit: 10,
      before: '2026-11-14T00:00:00.000Z',
    });
    expect((await api.add('Ris')).text).toBe('Ris');
    await api.setSettings({ autoIntervalMin: 30 });
    expect(call).toHaveBeenCalledWith('set_logbook_settings', {
      settings: { autoIntervalMin: 30 },
    });
    expect(await api.exportCsv()).toBe(true);
  });
});

describe('recherche', () => {
  it('valide les résultats', async () => {
    const raw = { kind: 'poi', name: 'Moorea', detail: 'island', position: rawMark.position };
    expect(parsePlace(raw).kind).toBe('poi');
    expect(() => parsePlace({ ...raw, kind: 'ville' })).toThrow(EngineProtocolError);
    const call = vi.fn(() => Promise.resolve([raw]));
    expect(await searchPlaces('moo', call)).toHaveLength(1);
    expect(call).toHaveBeenCalledWith('search_places', { query: 'moo' });
  });
});

const rawProfile = {
  stats: {
    distanceM: 2000,
    durationS: 600,
    movingS: 500,
    averageMps: 3.3,
    movingAverageMps: 4,
    maxMps: 5,
    minDepthM: null,
  },
  points: [
    {
      at: '2026-11-14T08:00:00Z',
      position: rawMark.position,
      speedMps: null,
      depthM: null,
      distanceM: 0,
    },
    {
      at: '2026-11-14T08:10:00Z',
      position: rawMark.position,
      speedMps: 4,
      depthM: 12,
      distanceM: 2000,
    },
  ],
};

describe('profil de trace', () => {
  it('valide le profil reçu', async () => {
    const p = parseTrackProfile(rawProfile);
    expect(p.points[1]?.speedMps).toBe(4);
    const reversed = { ...rawProfile, points: [...rawProfile.points].reverse() };
    expect(() => parseTrackProfile(reversed)).toThrow(EngineProtocolError);
    const negative = { ...rawProfile, stats: { ...rawProfile.stats, maxMps: -1 } };
    expect(() => parseTrackProfile(negative)).toThrow(EngineProtocolError);
    const call = vi.fn(() => Promise.resolve(rawProfile));
    await getTrackProfile('t1', call);
    expect(call).toHaveBeenCalledWith('get_track_profile', { id: 't1' });
  });
});
