import { describe, expect, it, vi } from 'vitest';
import { POSITION_EVENT, parsePositionFix, subscribePosition } from './position';
import { EngineProtocolError } from './validate';

const validFix = {
  position: { latitudeDeg: -17.535, longitudeDeg: -149.569 },
  cogRad: null,
  sogMps: 0,
  source: 'simulator',
  sourceId: 'simulator',
  measuredAt: '2026-11-12T10:00:00Z',
  receivedAt: '2026-11-12T10:00:00Z',
};

describe('parsePositionFix', () => {
  it('accepte un relevé simulé valide', () => {
    expect(parsePositionFix(validFix)).toEqual({
      latitudeDeg: -17.535,
      longitudeDeg: -149.569,
      cogRad: null,
      sogMps: 0,
      source: 'simulator',
      sourceId: 'simulator',
      measuredAt: Date.UTC(2026, 10, 12, 10, 0, 0),
    });
  });

  it.each([
    ['latitude hors bornes', { position: { latitudeDeg: 91, longitudeDeg: 0 } }],
    ['longitude hors bornes', { position: { latitudeDeg: 0, longitudeDeg: -181 } }],
    ['latitude non numérique', { position: { latitudeDeg: '17', longitudeDeg: 0 } }],
    ['position absente', { position: null }],
    ['source inconnue', { source: 'gps' }],
    ['horodatage invalide', { measuredAt: 'hier' }],
    ['horodatage absent', { measuredAt: undefined }],
    ['vitesse non finie', { sogMps: 'NaN' }],
  ])('rejette : %s', (_label, patch) => {
    expect(() => parsePositionFix({ ...validFix, ...patch })).toThrow(EngineProtocolError);
  });
});

describe('subscribePosition', () => {
  it('transmet les relevés valides et signale les invalides', async () => {
    let handler: ((event: { payload: unknown }) => void) | undefined;
    const unlisten = vi.fn();
    const listenFn = vi.fn((_event: string, h: (event: { payload: unknown }) => void) => {
      handler = h;
      return Promise.resolve(unlisten);
    });
    const onFix = vi.fn();
    const onError = vi.fn();

    const result = await subscribePosition(onFix, onError, listenFn);
    expect(listenFn).toHaveBeenCalledWith(POSITION_EVENT, expect.any(Function));

    handler?.({ payload: validFix });
    handler?.({ payload: { ...validFix, position: { latitudeDeg: 100, longitudeDeg: 0 } } });
    expect(onFix).toHaveBeenCalledTimes(1);
    expect(onError).toHaveBeenCalledTimes(1);

    result();
    expect(unlisten).toHaveBeenCalled();
  });
});
