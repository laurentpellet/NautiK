import { describe, expect, it } from 'vitest';
import { parseOwnShip, subscribeOwnShip } from './ownShip';

const p = (latitudeDeg: number, longitudeDeg: number) => ({ latitudeDeg, longitudeDeg });

const full = {
  computedAt: '2026-11-14T10:00:00Z',
  simulated: true,
  vectorHorizonS: 360,
  position: p(-17.5, -149.6),
  vectorEnd: p(-17.48, -149.6),
  headingEnd: p(-17.48, -149.59),
  trueWindDirectionRad: 1.5,
  vmgWindMps: -2.1,
  vmgWaypointMps: 3.2,
  otherTackCogRad: 3.0,
  laylines: {
    mark: p(-17.4, -149.5),
    starboardEnd: p(-17.45, -149.45),
    portEnd: p(-17.45, -149.55),
    upwind: true,
    windAngleRad: 0.785,
  },
};

describe('parseOwnShip', () => {
  it('lit un état complet', () => {
    const o = parseOwnShip(full);
    expect(o.computedAt).toBe(Date.parse('2026-11-14T10:00:00Z'));
    expect(o.vectorEnd).toEqual(p(-17.48, -149.6));
    expect(o.laylines?.upwind).toBe(true);
    expect(o.vmgWindMps).toBe(-2.1);
  });

  it('accepte les valeurs absentes (données obsolètes)', () => {
    const o = parseOwnShip({
      computedAt: '2026-11-14T10:00:00Z',
      simulated: false,
      vectorHorizonS: 360,
      position: null,
      vectorEnd: null,
      headingEnd: null,
      trueWindDirectionRad: null,
      vmgWindMps: null,
      vmgWaypointMps: null,
      otherTackCogRad: null,
      laylines: null,
    });
    expect(o.position).toBeNull();
    expect(o.laylines).toBeNull();
  });

  it('rejette les données malformées', () => {
    expect(() => parseOwnShip(null)).toThrow();
    expect(() => parseOwnShip({ ...full, simulated: 'non' })).toThrow();
    expect(() => parseOwnShip({ ...full, vectorEnd: p(95, 0) })).toThrow();
    expect(() => parseOwnShip({ ...full, vmgWindMps: 'x' })).toThrow();
    expect(() => parseOwnShip({ ...full, laylines: { ...full.laylines, upwind: 1 } })).toThrow();
  });
});

describe('subscribeOwnShip', () => {
  it('relaie les états valides et signale les autres', async () => {
    let handler: ((event: { payload: unknown }) => void) | undefined;
    const got: unknown[] = [];
    const errors: Error[] = [];
    await subscribeOwnShip(
      (s) => got.push(s),
      (e) => errors.push(e),
      (event, h) => {
        expect(event).toBe('own-ship');
        handler = h;
        return Promise.resolve(() => {});
      },
    );
    handler?.({ payload: full });
    handler?.({ payload: { ...full, computedAt: 3 } });
    expect(got).toHaveLength(1);
    expect(errors).toHaveLength(1);
  });
});
