import { describe, expect, it, vi } from 'vitest';
import {
  AIS_EVENT,
  navigationStatusLabel,
  parseAisSnapshot,
  subscribeAis,
  targetLabel,
} from './ais';
import { EngineProtocolError } from './validate';

const target = {
  mmsi: '227006760',
  class: 'A',
  name: 'ARANUI 5',
  callsign: 'FOPQ',
  shipType: 70,
  navigationStatus: 0,
  position: { latitudeDeg: -14.97, longitudeDeg: -147.64 },
  sogMps: 6.2,
  cogRad: 1.2,
  headingRad: null,
  vectorEnd: { latitudeDeg: -14.96, longitudeDeg: -147.62 },
  lastSeenAt: '2026-11-12T10:00:00Z',
  positionAgeMs: 1500,
  state: 'active',
  history: [{ latitudeDeg: -14.98, longitudeDeg: -147.65 }],
};

const snapshot = {
  computedAt: '2026-11-12T10:00:01Z',
  simulated: true,
  targets: [target],
};

describe('parseAisSnapshot', () => {
  it('accepte un instantané valide', () => {
    const s = parseAisSnapshot(snapshot);
    expect(s.simulated).toBe(true);
    expect(s.computedAt).toBe(Date.UTC(2026, 10, 12, 10, 0, 1));
    expect(s.targets[0]).toEqual({ ...target, lastSeenAt: Date.UTC(2026, 10, 12, 10, 0, 0) });
  });

  it('accepte une cible sans position ni données dynamiques', () => {
    const t = parseAisSnapshot({
      ...snapshot,
      targets: [
        {
          ...target,
          class: 'B',
          name: null,
          callsign: null,
          shipType: null,
          navigationStatus: null,
          position: null,
          sogMps: null,
          cogRad: null,
          vectorEnd: null,
          positionAgeMs: null,
          state: 'lost',
          history: [],
        },
      ],
    }).targets[0];
    expect(t?.position).toBeNull();
    expect(t?.state).toBe('lost');
  });

  it.each([
    ['MMSI court', { mmsi: '12345' }],
    ['MMSI nul', { mmsi: '000000000' }],
    ['MMSI numérique', { mmsi: 227006760 }],
    ['classe inconnue', { class: 'C' }],
    ['état inconnu', { state: 'gone' }],
    ['position hors bornes', { position: { latitudeDeg: 91, longitudeDeg: 0 } }],
    ['vitesse négative', { sogMps: -1 }],
    ['route hors bornes', { cogRad: 7 }],
    ['cap non fini', { headingRad: 'nord' }],
    ['statut réservé', { navigationStatus: 15 }],
    ['type nul', { shipType: 0 }],
    ['âge négatif', { positionAgeMs: -1 }],
    ['historique absent', { history: undefined }],
    ['historique invalide', { history: [{ latitudeDeg: 0 }] }],
  ])('rejette : %s', (_label, patch) => {
    expect(() => parseAisSnapshot({ ...snapshot, targets: [{ ...target, ...patch }] })).toThrow(
      EngineProtocolError,
    );
  });

  it('rejette un instantané mal formé', () => {
    expect(() => parseAisSnapshot({ ...snapshot, simulated: 'non' })).toThrow(EngineProtocolError);
    expect(() => parseAisSnapshot({ ...snapshot, targets: null })).toThrow(EngineProtocolError);
  });
});

describe('libellés', () => {
  it('nomme les statuts de navigation et les cibles', () => {
    expect(navigationStatusLabel(1)).toBe('Au mouillage');
    expect(navigationStatusLabel(null)).toBeNull();
    expect(navigationStatusLabel(42)).toBeNull();
    const t = parseAisSnapshot(snapshot).targets[0];
    if (!t) throw new Error('cible attendue');
    expect(targetLabel(t)).toBe('ARANUI 5');
    expect(targetLabel({ ...t, name: null })).toBe('MMSI 227006760');
  });
});

describe('subscribeAis', () => {
  it('transmet les instantanés valides et signale les invalides', async () => {
    let handler: ((event: { payload: unknown }) => void) | undefined;
    const unlisten = vi.fn();
    const listenFn = vi.fn((_event: string, h: (event: { payload: unknown }) => void) => {
      handler = h;
      return Promise.resolve(unlisten);
    });
    const onValue = vi.fn();
    const onError = vi.fn();
    const stop = await subscribeAis(onValue, onError, listenFn);
    expect(listenFn).toHaveBeenCalledWith(AIS_EVENT, expect.any(Function));
    handler?.({ payload: snapshot });
    handler?.({ payload: { ...snapshot, targets: [{ ...target, mmsi: 'x' }] } });
    expect(onValue).toHaveBeenCalledTimes(1);
    expect(onError).toHaveBeenCalledTimes(1);
    stop();
    expect(unlisten).toHaveBeenCalled();
  });
});
