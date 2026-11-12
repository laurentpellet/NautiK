import { describe, expect, it } from 'vitest';
import type { SourcesView } from './services/instruments';
import {
  describeSource,
  formFromView,
  parsePriorities,
  requestFromForm,
  type SourceForm,
} from './sourceForm';

const base: SourcesView = {
  mode: 'simulator',
  signalkUrl: null,
  nmea: null,
  priorities: [],
  tokenStored: false,
  connection: null,
  rejections: { count: 0, lastReason: null },
};

function form(patch: Partial<SourceForm>, view: SourcesView = base): SourceForm {
  return { ...formFromView(view), ...patch };
}

describe('formFromView', () => {
  it('pré-remplit sans jamais exposer de jeton', () => {
    const f = formFromView({
      ...base,
      mode: 'nmea0183',
      nmea: { kind: 'serial', path: 'COM3', baudRate: 38400 },
      priorities: ['nmea0183.GP', 'nmea0183.GN'],
      tokenStored: true,
    });
    expect(f.nmeaKind).toBe('serial');
    expect(f.serialPath).toBe('COM3');
    expect(f.baudRate).toBe(38400);
    expect(f.port).toBe('10110');
    expect(f.priorities).toBe('nmea0183.GP\nnmea0183.GN');
    expect(f.token).toBe('');
  });

  it('reprend une liaison TCP', () => {
    const f = formFromView({ ...base, nmea: { kind: 'tcp', host: 'boat.local', port: 2000 } });
    expect([f.nmeaKind, f.host, f.port]).toEqual(['tcp', 'boat.local', '2000']);
  });
});

describe('requestFromForm', () => {
  it('construit chaque liaison NMEA', () => {
    const tcp = requestFromForm(
      form({ mode: 'nmea0183', nmeaKind: 'tcp', host: ' 192.168.1.10 ', port: '10110' }),
      base,
    );
    expect(tcp.nmea).toEqual({ kind: 'tcp', host: '192.168.1.10', port: 10110 });
    const udp = requestFromForm(form({ mode: 'nmea0183', nmeaKind: 'udp', port: '2000' }), base);
    expect(udp.nmea).toEqual({ kind: 'udp', port: 2000 });
    const serial = requestFromForm(
      form({ mode: 'nmea0183', nmeaKind: 'serial', serialPath: 'COM4', baudRate: 4800 }),
      base,
    );
    expect(serial.nmea).toEqual({ kind: 'serial', path: 'COM4', baudRate: 4800 });
    const replay = requestFromForm(
      form({ mode: 'nmea0183', nmeaKind: 'replay', replayPath: 'C:\\logs\\a.nmea' }),
      base,
    );
    expect(replay.nmea).toEqual({ kind: 'replay', path: 'C:\\logs\\a.nmea' });
  });

  it.each([
    ['hôte vide', { nmeaKind: 'tcp' as const, host: ' ' }, /adresse/],
    ['port non numérique', { nmeaKind: 'tcp' as const, host: 'h', port: '10a' }, /port/],
    ['port nul', { nmeaKind: 'udp' as const, port: '0' }, /port/],
    ['port trop grand', { nmeaKind: 'udp' as const, port: '70000' }, /port/],
    ['port série vide', { nmeaKind: 'serial' as const, serialPath: '' }, /port série/],
    ['aucun fichier', { nmeaKind: 'replay' as const, replayPath: '' }, /enregistrement/],
  ])('refuse : %s', (_label, patch, expected) => {
    expect(() => requestFromForm(form({ mode: 'nmea0183', ...patch }), base)).toThrow(expected);
  });

  it('conserve la liaison enregistrée hors mode NMEA', () => {
    const view: SourcesView = { ...base, nmea: { kind: 'udp', port: 10110 } };
    const r = requestFromForm(form({ mode: 'signalK', signalkUrl: ' http://h:3000 ' }, view), view);
    expect(r.nmea).toEqual({ kind: 'udp', port: 10110 });
    expect(r.signalkUrl).toBe('http://h:3000');
    // Saisie NMEA incomplète sans effet hors mode NMEA.
    expect(() => requestFromForm(form({ mode: 'simulator', host: '' }, view), view)).not.toThrow();
  });

  it('transmet le jeton seulement s’il est saisi', () => {
    expect(requestFromForm(form({ token: '  ' }), base).token).toBeNull();
    expect(requestFromForm(form({ token: 'abc' }), base).token).toBe('abc');
  });
});

describe('priorités et descriptions', () => {
  it('découpe les priorités', () => {
    expect(parsePriorities(' nmea0183.GP \n\n nmea0183.GN, signalk ')).toEqual([
      'nmea0183.GP',
      'nmea0183.GN',
      'signalk',
    ]);
    expect(parsePriorities('')).toEqual([]);
  });

  it('décrit la source active', () => {
    expect(describeSource(base)).toBe('Simulateur');
    expect(describeSource({ ...base, mode: 'signalK', signalkUrl: 'http://h' })).toBe(
      'Signal K — http://h',
    );
    const nmea = (n: SourcesView['nmea']) => describeSource({ ...base, mode: 'nmea0183', nmea: n });
    expect(nmea({ kind: 'tcp', host: 'h', port: 1 })).toBe('NMEA 0183 — TCP h:1');
    expect(nmea({ kind: 'udp', port: 10110 })).toBe('NMEA 0183 — UDP, port 10110');
    expect(nmea({ kind: 'serial', path: 'COM3', baudRate: 4800 })).toBe(
      'NMEA 0183 — série COM3 (4800 Bd)',
    );
    expect(nmea({ kind: 'replay', path: 'C:\\logs\\tiputa.nmea' })).toBe(
      'NMEA 0183 — rejeu de tiputa.nmea',
    );
    expect(nmea(null)).toBe('NMEA 0183 — ?');
  });
});
