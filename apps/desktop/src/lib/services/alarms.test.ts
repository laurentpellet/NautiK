import { describe, expect, it } from 'vitest';
import {
  AlarmsApi,
  alertingAlarms,
  parseAlarmEvent,
  parseAlarmsSnapshot,
  subscribeAlarms,
} from './alarms';

const snapshot = {
  computedAt: '2026-11-12T10:00:00Z',
  alarms: [
    {
      kind: 'anchor',
      label: 'Mouillage',
      state: 'active',
      since: '2026-11-12T09:59:50Z',
      message: 'DÉRAPAGE — 62 m de l’ancre, rayon 50 m',
    },
    {
      kind: 'positionLost',
      label: 'Perte de position',
      state: 'normal',
      since: null,
      message: 'Position reçue',
    },
  ],
  anchor: {
    position: { latitudeDeg: -17.5, longitudeDeg: -149.5 },
    radiusM: 50,
    hysteresisM: 5,
    delayS: 10,
    distanceM: 62,
    bearingRad: 1.2,
  },
  anchorRadiusM: 50,
  sounding: true,
  simulated: false,
};

const event = {
  id: 'e1',
  alarm: 'anchor',
  kind: 'activated',
  at: '2026-11-12T09:59:50Z',
  details: { message: 'Dérapage', distanceM: 62, simulated: true },
};

describe('parseAlarmsSnapshot', () => {
  it('lit un état complet', () => {
    const s = parseAlarmsSnapshot(snapshot);
    expect(s.sounding).toBe(true);
    expect(s.alarms[0]?.since).toBe(Date.parse('2026-11-12T09:59:50Z'));
    expect(s.anchor?.distanceM).toBe(62);
    expect(alertingAlarms(s).map((a) => a.kind)).toEqual(['anchor']);
    expect(alertingAlarms(null)).toEqual([]);
  });

  it('accepte un mouillage à distance inconnue', () => {
    const s = parseAlarmsSnapshot({
      ...snapshot,
      anchor: { ...snapshot.anchor, distanceM: null, bearingRad: null },
    });
    expect(s.anchor?.distanceM).toBeNull();
  });

  it('rejette les réponses malformées', () => {
    expect(() => parseAlarmsSnapshot({ ...snapshot, sounding: 'oui' })).toThrow();
    expect(() =>
      parseAlarmsSnapshot({ ...snapshot, alarms: [{ ...snapshot.alarms[0], state: 'x' }] }),
    ).toThrow();
    expect(() =>
      parseAlarmsSnapshot({ ...snapshot, alarms: [{ ...snapshot.alarms[0], kind: 'depth' }] }),
    ).toThrow();
    expect(() =>
      parseAlarmsSnapshot({ ...snapshot, anchor: { ...snapshot.anchor, radiusM: 5 } }),
    ).toThrow();
    expect(() =>
      parseAlarmsSnapshot({ ...snapshot, anchor: { ...snapshot.anchor, distanceM: -1 } }),
    ).toThrow();
    expect(() =>
      parseAlarmsSnapshot({
        ...snapshot,
        anchor: { ...snapshot.anchor, position: { latitudeDeg: 95, longitudeDeg: 0 } },
      }),
    ).toThrow();
    expect(() => parseAlarmsSnapshot(null)).toThrow();
  });
});

describe('parseAlarmEvent', () => {
  it('lit un événement', () => {
    expect(parseAlarmEvent(event)).toEqual({
      id: 'e1',
      alarm: 'anchor',
      kind: 'activated',
      at: Date.parse('2026-11-12T09:59:50Z'),
      message: 'Dérapage',
      distanceM: 62,
      simulated: true,
    });
  });

  it('tolère des détails minimaux et rejette les inconnus', () => {
    const e = parseAlarmEvent({ ...event, details: { message: 'x' } });
    expect(e.distanceM).toBeNull();
    expect(e.simulated).toBe(false);
    expect(() => parseAlarmEvent({ ...event, kind: 'deleted' })).toThrow();
    expect(() => parseAlarmEvent({ ...event, details: null })).toThrow();
  });
});

describe('AlarmsApi', () => {
  it('transmet les commandes et valide les réponses', async () => {
    const calls: [string, unknown][] = [];
    const api = new AlarmsApi((command, args) => {
      calls.push([command, args]);
      return Promise.resolve(command === 'list_alarm_events' ? [event] : snapshot);
    });
    await api.get();
    await api.dropAnchor(60, 10);
    await api.raiseAnchor();
    await api.acknowledge('anchor');
    await api.setEnabled('positionLost', false);
    expect(await api.events(50)).toHaveLength(1);
    expect(calls).toEqual([
      ['get_alarms', undefined],
      ['drop_anchor', { radiusM: 60, delayS: 10 }],
      ['raise_anchor', undefined],
      ['acknowledge_alarm', { kind: 'anchor' }],
      ['set_alarm_enabled', { kind: 'positionLost', enabled: false }],
      ['list_alarm_events', { limit: 50 }],
    ]);
  });

  it("propage l'erreur du moteur", async () => {
    const api = new AlarmsApi(() =>
      Promise.reject(new Error('aucune position récente : impossible de poser l’ancre')),
    );
    await expect(api.dropAnchor(50, 10)).rejects.toThrow('aucune position récente');
  });
});

describe('subscribeAlarms', () => {
  it('signale un état illisible', async () => {
    const handlers: ((e: { payload: unknown }) => void)[] = [];
    const seen: unknown[] = [];
    const errors: string[] = [];
    await subscribeAlarms(
      (s) => seen.push(s.sounding),
      (e) => errors.push(e.message),
      (_event, h) => {
        handlers.push(h);
        return Promise.resolve(() => {});
      },
    );
    for (const h of handlers) {
      h({ payload: snapshot });
      h({ payload: { nope: 1 } });
    }
    expect(seen).toEqual([true]);
    expect(errors).toHaveLength(1);
  });
});
