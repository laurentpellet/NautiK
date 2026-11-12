import { describe, expect, it, vi } from 'vitest';
import {
  INSTRUMENTS_EVENT,
  SIGNALK_STATUS_EVENT,
  SourcesApi,
  parseConnectionState,
  parseInstrumentsSnapshot,
  parseSourcesView,
  subscribeConnection,
  subscribeInstruments,
} from './instruments';
import { EngineProtocolError } from './validate';

const fix = {
  position: { latitudeDeg: -14.968, longitudeDeg: -147.638 },
  cogRad: 1.0,
  sogMps: 3.2,
  source: 'signalK',
  sourceId: 'gps.1',
  measuredAt: '2026-11-12T10:00:00Z',
  receivedAt: '2026-11-12T10:00:00Z',
};

const reading = {
  quantity: 'depthBelowTransducer',
  value: 12.5,
  source: 'signalK',
  sourceId: 'sounder',
  measuredAt: '2026-11-12T10:00:00Z',
  ageMs: 1200,
  stale: false,
};

const snapshot = {
  computedAt: '2026-11-12T10:00:01Z',
  position: { fix, ageMs: 1000, stale: false },
  readings: [reading],
};

describe('parseInstrumentsSnapshot', () => {
  it('accepte un instantané valide', () => {
    const s = parseInstrumentsSnapshot(snapshot);
    expect(s.computedAt).toBe(Date.UTC(2026, 10, 12, 10, 0, 1));
    expect(s.position?.fix.latitudeDeg).toBe(-14.968);
    expect(s.position?.stale).toBe(false);
    expect(s.readings).toEqual([{ ...reading, measuredAt: Date.UTC(2026, 10, 12, 10, 0, 0) }]);
  });

  it('accepte un instantané sans position ni mesure', () => {
    const s = parseInstrumentsSnapshot({ ...snapshot, position: null, readings: [] });
    expect(s.position).toBeNull();
    expect(s.readings).toEqual([]);
  });

  it.each([
    ['grandeur inconnue', { readings: [{ ...reading, quantity: 'rpm' }] }],
    ['valeur non finie', { readings: [{ ...reading, value: null }] }],
    ['âge négatif', { readings: [{ ...reading, ageMs: -1 }] }],
    ['fraîcheur absente', { readings: [{ ...reading, stale: 'non' }] }],
    ['source inconnue', { readings: [{ ...reading, source: 'gps' }] }],
    [
      'position hors bornes',
      {
        position: {
          fix: { ...fix, position: { latitudeDeg: 91, longitudeDeg: 0 } },
          ageMs: 0,
          stale: false,
        },
      },
    ],
    ['mesures absentes', { readings: undefined }],
    ['horodatage invalide', { computedAt: 'maintenant' }],
  ])('rejette : %s', (_label, patch) => {
    expect(() => parseInstrumentsSnapshot({ ...snapshot, ...patch })).toThrow(EngineProtocolError);
  });
});

describe('parseConnectionState', () => {
  it.each([
    [{ state: 'connecting', attempt: 2 }],
    [{ state: 'connected', serverName: 'signalk-server', serverVersion: '2.8.0' }],
    [{ state: 'connected', serverName: null, serverVersion: null }],
    [{ state: 'disconnected', reason: 'refusé', retryInMs: 4000 }],
    [{ state: 'unauthorized', reason: 'HTTP 401', retryInMs: 30000 }],
    [{ state: 'stopped' }],
  ])('accepte %o', (value) => {
    expect(parseConnectionState(value)).toEqual(value);
  });

  it.each([
    [{ state: 'flying' }],
    [{ state: 'connecting', attempt: -1 }],
    [{ state: 'disconnected', reason: 'x' }],
    [{ state: 'unauthorized', retryInMs: 1 }],
    ['connected'],
  ])('rejette %o', (value) => {
    expect(() => parseConnectionState(value)).toThrow(EngineProtocolError);
  });
});

describe('parseSourcesView', () => {
  it('accepte la vue des réglages', () => {
    const v = {
      mode: 'signalK',
      signalkUrl: 'http://boat.local:3000',
      tokenStored: true,
      connection: { state: 'stopped' },
    };
    expect(parseSourcesView(v)).toEqual(v);
  });

  it('rejette un mode inconnu', () => {
    expect(() =>
      parseSourcesView({ mode: 'nmea', signalkUrl: null, tokenStored: false, connection: null }),
    ).toThrow(EngineProtocolError);
  });
});

describe('SourcesApi', () => {
  it('transmet les arguments de set_sources', async () => {
    const view = { mode: 'simulator', signalkUrl: null, tokenStored: false, connection: null };
    const invokeFn = vi.fn(() => Promise.resolve(view));
    const api = new SourcesApi(invokeFn);
    await api.set({ mode: 'simulator', signalkUrl: null, token: 'abc', clearToken: false });
    expect(invokeFn).toHaveBeenCalledWith('set_sources', {
      mode: 'simulator',
      signalkUrl: null,
      token: 'abc',
      clearToken: false,
    });
    expect(await api.get()).toEqual(view);
    expect(invokeFn).toHaveBeenLastCalledWith('get_sources');
  });

  it('rejette une réponse invalide', async () => {
    const api = new SourcesApi(() => Promise.resolve({ mode: 'simulator' }));
    await expect(api.get()).rejects.toThrow(EngineProtocolError);
  });
});

function fakeListen() {
  let handler: ((event: { payload: unknown }) => void) | undefined;
  const unlisten = vi.fn();
  const listenFn = vi.fn((_event: string, h: (event: { payload: unknown }) => void) => {
    handler = h;
    return Promise.resolve(unlisten);
  });
  return { listenFn, unlisten, emit: (payload: unknown) => handler?.({ payload }) };
}

describe('abonnements', () => {
  it('transmet les instantanés valides et signale les invalides', async () => {
    const l = fakeListen();
    const onValue = vi.fn();
    const onError = vi.fn();
    const stop = await subscribeInstruments(onValue, onError, l.listenFn);
    expect(l.listenFn).toHaveBeenCalledWith(INSTRUMENTS_EVENT, expect.any(Function));
    l.emit(snapshot);
    l.emit({ ...snapshot, readings: 'non' });
    expect(onValue).toHaveBeenCalledTimes(1);
    expect(onError).toHaveBeenCalledTimes(1);
    stop();
    expect(l.unlisten).toHaveBeenCalled();
  });

  it("transmet l'état de connexion, y compris null", async () => {
    const l = fakeListen();
    const onValue = vi.fn();
    const onError = vi.fn();
    await subscribeConnection(onValue, onError, l.listenFn);
    expect(l.listenFn).toHaveBeenCalledWith(SIGNALK_STATUS_EVENT, expect.any(Function));
    l.emit({ state: 'connecting', attempt: 1 });
    l.emit(null);
    l.emit({ state: '?' });
    expect(onValue).toHaveBeenNthCalledWith(1, { state: 'connecting', attempt: 1 });
    expect(onValue).toHaveBeenNthCalledWith(2, null);
    expect(onError).toHaveBeenCalledTimes(1);
  });
});
