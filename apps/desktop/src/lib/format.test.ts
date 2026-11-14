import { describe, expect, it } from 'vitest';
import {
  ENGINE_SILENCE_MS,
  formatAge,
  formatBearing,
  formatCrossTrack,
  formatDepth,
  formatDistance,
  formatDuration,
  formatEta,
  formatUtcClock,
  formatLatitude,
  formatLongitude,
  formatRelativeAngle,
  formatSpeed,
  describeConnection,
  isEngineSilent,
} from './format';

describe('coordonnées', () => {
  it('formate latitude et longitude en degrés-minutes', () => {
    expect(formatLatitude(-17.535)).toBe('17°32.100′ S');
    expect(formatLongitude(-149.569)).toBe('149°34.140′ W');
    expect(formatLatitude(48.5)).toBe('48°30.000′ N');
    expect(formatLongitude(2.25)).toBe('002°15.000′ E');
  });

  it('gère équateur, méridien et bornes', () => {
    expect(formatLatitude(0)).toBe('00°00.000′ N');
    expect(formatLongitude(0)).toBe('000°00.000′ E');
    expect(formatLatitude(-90)).toBe('90°00.000′ S');
    expect(formatLongitude(180)).toBe('180°00.000′ E');
  });

  it('reporte l’arrondi des minutes sur les degrés', () => {
    expect(formatLatitude(10.9999999)).toBe('11°00.000′ N');
  });
});

describe('âge et obsolescence', () => {
  it('formate les âges', () => {
    expect(formatAge(0)).toBe('0 s');
    expect(formatAge(-500)).toBe('0 s');
    expect(formatAge(59_999)).toBe('59 s');
    expect(formatAge(60_000)).toBe('1 min');
    expect(formatAge(3_600_000)).toBe('1 h');
  });

  it('détecte le silence du moteur au-delà du seuil', () => {
    expect(isEngineSilent(null, 0)).toBe(true);
    expect(isEngineSilent(0, ENGINE_SILENCE_MS)).toBe(false);
    expect(isEngineSilent(0, ENGINE_SILENCE_MS + 1)).toBe(true);
  });
});

describe('formatage instruments', () => {
  it('formate les vitesses en nœuds', () => {
    expect(formatSpeed(null)).toBe('—');
    expect(formatSpeed(0)).toBe('0.0 kn');
    expect(formatSpeed(1852 / 3600)).toBe('1.0 kn');
    expect(formatSpeed(4.115)).toBe('8.0 kn');
  });

  it('formate les profondeurs', () => {
    expect(formatDepth(null)).toBe('—');
    expect(formatDepth(0)).toBe('0.0 m');
    expect(formatDepth(12.46)).toBe('12.5 m');
    expect(formatDepth(125.4)).toBe('125 m');
  });

  it('formate les angles relatifs avec le bord', () => {
    expect(formatRelativeAngle(null)).toBe('—');
    expect(formatRelativeAngle(0)).toBe('000°');
    expect(formatRelativeAngle(Math.PI / 4)).toBe('045° Td');
    expect(formatRelativeAngle((-2 * Math.PI) / 3)).toBe('120° Bd');
    expect(formatRelativeAngle(Math.PI)).toBe('180°');
  });
});

describe('formatage navigation', () => {
  it('formate les distances en milles marins', () => {
    expect(formatDistance(1852)).toBe('1.0 NM');
    expect(formatDistance(92.6)).toBe('0.05 NM');
    expect(formatDistance(0)).toBe('0.00 NM');
    expect(formatDistance(345_000)).toBe('186.3 NM');
  });

  it('formate les relèvements sur trois chiffres, normalisés', () => {
    expect(formatBearing(0)).toBe('000°');
    expect(formatBearing(Math.PI / 2)).toBe('090°');
    expect(formatBearing(-Math.PI / 2)).toBe('270°');
    expect(formatBearing(2 * Math.PI - 0.001)).toBe('000°');
    expect(formatBearing(null)).toBe('—');
  });

  it("indique le côté de l'écart de route", () => {
    expect(formatCrossTrack(185.2)).toBe('0.10 NM D');
    expect(formatCrossTrack(-185.2)).toBe('0.10 NM G');
    expect(formatCrossTrack(0)).toBe('0.00 NM');
    expect(formatCrossTrack(null)).toBe('—');
  });

  it('formate une ETA en UTC avec la durée restante', () => {
    const now = Date.parse('2026-11-12T10:00:00Z');
    expect(formatEta(Date.parse('2026-11-12T10:25:00Z'), now)).toBe('10:25 UTC (dans 25 min)');
    expect(formatEta(Date.parse('2026-11-12T11:05:00Z'), now)).toBe('11:05 UTC (dans 1 h 05)');
    expect(formatEta(null, now)).toBe('—');
  });

  it('formate une durée et une heure UTC', () => {
    expect(formatDuration(0)).toBe('0 min');
    expect(formatDuration(-5 * 60_000)).toBe('0 min');
    expect(formatDuration(59 * 60_000)).toBe('59 min');
    expect(formatDuration(65 * 60_000)).toBe('1 h 05');
    expect(formatDuration(47 * 3_600_000 + 59 * 60_000)).toBe('47 h 59');
    expect(formatDuration(50 * 3_600_000)).toBe('2 j 02 h');
    expect(formatUtcClock(Date.parse('2026-11-12T03:07:59Z'))).toBe('03:07');
  });
});

describe('état de connexion', () => {
  it('décrit chaque état avec sa gravité', () => {
    expect(describeConnection(null).level).toBe('pending');
    expect(describeConnection({ state: 'connecting', attempt: 3 })).toEqual({
      label: 'Connexion en cours (tentative 3)',
      level: 'pending',
    });
    expect(
      describeConnection({
        state: 'connected',
        serverName: 'signalk-server',
        serverVersion: '2.8.0',
      }),
    ).toEqual({ label: 'Connecté — signalk-server 2.8.0', level: 'ok' });
    expect(
      describeConnection({ state: 'connected', serverName: null, serverVersion: null }).label,
    ).toBe('Connecté');
    expect(
      describeConnection({ state: 'disconnected', reason: 'refusé', retryInMs: 4000 }),
    ).toEqual({ label: 'Déconnecté : refusé — nouvel essai dans 4 s', level: 'alert' });
    const denied = describeConnection({
      state: 'unauthorized',
      reason: 'HTTP 401',
      retryInMs: 30_000,
    });
    expect(denied.level).toBe('alert');
    expect(denied.label).toContain('jeton');
    expect(describeConnection({ state: 'stopped' }).level).toBe('alert');
  });
});
