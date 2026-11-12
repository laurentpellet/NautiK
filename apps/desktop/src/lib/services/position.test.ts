import { describe, expect, it } from 'vitest';
import { parsePositionFix } from './position';
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
