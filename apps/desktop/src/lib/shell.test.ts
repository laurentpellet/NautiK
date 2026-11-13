import { describe, expect, it } from 'vitest';
import type { AlarmView } from './services/alarms';
import type { PositionStatus } from './services/instruments';
import { activeAlerts, statusPills, utcClock, type ShellInputs } from './shell';

const position = (over: Partial<PositionStatus> = {}, source = 'nmea0183'): PositionStatus => ({
  fix: {
    latitudeDeg: -17.5,
    longitudeDeg: -149.8,
    cogRad: null,
    sogMps: null,
    source: source as PositionStatus['fix']['source'],
    sourceId: 'gps.1',
    measuredAt: 0,
  },
  ageMs: 500,
  stale: false,
  freshSources: 1,
  ...over,
});

const inputs = (over: Partial<ShellInputs> = {}): ShellInputs => ({
  position: position(),
  silent: false,
  connection: null,
  ais: null,
  ...over,
});

const pill = (over: Partial<ShellInputs>, id: string) =>
  statusPills(inputs(over)).find((p) => p.id === id);

describe("pastilles d'état", () => {
  it('tout va bien', () => {
    const pills = statusPills(inputs());
    expect(pills.map((p) => p.id)).toEqual(['position', 'link', 'ais', 'alarm', 'engine']);
    expect(pills.map((p) => p.level)).toEqual(['ok', 'ok', 'off', 'off', 'ok']);
    expect(activeAlerts(pills)).toEqual([]);
  });

  it('résume les alarmes du moteur', () => {
    const view = (kind: AlarmView['kind'], state: AlarmView['state']): AlarmView => ({
      kind,
      label: kind === 'anchor' ? 'Mouillage' : 'Perte de position',
      state,
      since: null,
      message: `${kind} ${state}`,
    });
    const alarms = (a: AlarmView['state'], p: AlarmView['state'], anchored = false) => ({
      computedAt: 0,
      alarms: [view('anchor', a), view('positionLost', p)],
      anchor: anchored
        ? {
            position: { latitudeDeg: 0, longitudeDeg: 0 },
            radiusM: 50,
            hysteresisM: 5,
            delayS: 10,
            distanceM: 3,
            bearingRad: null,
          }
        : null,
      anchorRadiusM: 50,
      sounding: false,
      simulated: false,
    });
    expect(pill({ alarms: alarms('off', 'normal') }, 'alarm')).toMatchObject({
      state: 'Veille',
      level: 'ok',
    });
    expect(pill({ alarms: alarms('normal', 'normal', true) }, 'alarm')?.state).toBe('Mouillage');
    expect(pill({ alarms: alarms('pending', 'normal', true) }, 'alarm')?.level).toBe('warn');
    expect(pill({ alarms: alarms('off', 'off') }, 'alarm')).toMatchObject({
      state: 'Réduites',
      level: 'warn',
    });
    expect(pill({ alarms: alarms('active', 'normal', true) }, 'alarm')).toMatchObject({
      state: 'Mouillage',
      level: 'alert',
    });
    const both = statusPills(inputs({ alarms: alarms('acknowledged', 'active', true) }));
    expect(both.find((p) => p.id === 'alarm')?.state).toBe('2 actives');
    expect(activeAlerts(both)[0]).toContain('anchor acknowledged');
  });

  it('signale position absente, obsolète, simulée ou rejouée', () => {
    expect(pill({ position: null }, 'position')).toMatchObject({ state: 'Aucune', level: 'alert' });
    expect(pill({ position: position({ stale: true, ageMs: 12_000 }) }, 'position')).toMatchObject({
      state: 'Obsolète',
      level: 'alert',
    });
    expect(pill({ position: position({}, 'simulator') }, 'position')?.level).toBe('warn');
    expect(pill({ position: position({}, 'replay') }, 'position')?.state).toBe('Rejeu');
  });

  it('ne masque pas une perte de liaison', () => {
    expect(pill({ connection: undefined }, 'link')?.level).toBe('off');
    expect(pill({ connection: { state: 'connecting', attempt: 2 } }, 'link')).toMatchObject({
      state: 'Connexion…',
      level: 'warn',
    });
    const lost = pill(
      { connection: { state: 'disconnected', reason: 'coupure', retryInMs: 2000 } },
      'link',
    );
    expect(lost).toMatchObject({ state: 'En défaut', level: 'alert' });
    expect(lost?.title).toContain('coupure');
  });

  it('compte les cibles AIS et signale la simulation', () => {
    const ais = { computedAt: 0, simulated: true, targets: [] };
    expect(pill({ ais }, 'ais')).toMatchObject({ state: '0 cible', level: 'warn' });
  });

  it('moteur muet = alerte', () => {
    const pills = statusPills(inputs({ silent: true, position: null }));
    expect(activeAlerts(pills)).toHaveLength(2);
    expect(activeAlerts(pills)[1]).toContain('Moteur');
  });
});

describe('horloge', () => {
  it('affiche UTC', () => {
    const clock = utcClock(Date.UTC(2026, 10, 12, 14, 5, 9));
    expect(clock.time).toBe('14:05:09 UTC');
    expect(clock.date).toContain('2026');
    expect(clock.date).toContain('12');
  });
});
