import { describe, expect, it } from 'vitest';
import {
  cardinal,
  instrumentTiles,
  navigationTiles,
  positionTile,
  systemTiles,
} from './instrumentTiles';
import type { InstrumentsSnapshot, Quantity, Reading } from './services/instruments';
import type { NavigationStatus } from './services/routing';

const KNOT = 1852 / 3600;

const reading = (quantity: Quantity, value: number, stale = false): Reading => ({
  quantity,
  value,
  source: 'nmea0183',
  sourceId: 'nmea0183.II',
  measuredAt: 0,
  ageMs: stale ? 15_000 : 500,
  stale,
  freshSources: 1,
});

const snapshot = (readings: Reading[], position: InstrumentsSnapshot['position'] = null) => ({
  computedAt: 0,
  position,
  readings,
});

const byId = <T extends { id: string }>(tiles: T[], id: string) => tiles.find((t) => t.id === id);

describe('tuiles moteur, énergie, réservoirs, environnement', () => {
  it('convertit dans les unités du bord', () => {
    const g = systemTiles(
      snapshot([
        reading('engineSpeed', 2143 / 60),
        reading('engineCoolantTemperature', 273.15 + 81.6),
        reading('engineOilPressure', 345_000),
        reading('batteryStateOfCharge', 0.843),
        reading('batteryVoltage', 12.716),
        reading('batteryCurrent', -6.24),
        reading('solarPower', 212.4),
        reading('fuelLevel', 0.15),
        reading('freshWaterLevel', 0.64),
        reading('windSpeedTrue', 10 * KNOT),
        reading('windDirectionTrue', (112 * Math.PI) / 180),
        reading('waterTemperature', 273.15 + 27.24),
        reading('atmosphericPressure', 101_325),
      ]),
      false,
    );
    expect(byId(g.engine, 'rpm')).toMatchObject({ value: '2140', unit: 'tr/min' });
    expect(byId(g.engine, 'coolant')).toMatchObject({ value: '82', unit: '°C' });
    expect(byId(g.engine, 'oil')).toMatchObject({ value: '3.5', unit: 'bar' });
    expect(byId(g.energy, 'soc')).toMatchObject({ value: '84', unit: '%', ratio: 0.843 });
    expect(byId(g.energy, 'voltage')?.value).toBe('12.72');
    expect(byId(g.energy, 'current')?.value).toBe('-6.2');
    expect(byId(g.energy, 'solar')?.value).toBe('212');
    expect(byId(g.tanks, 'fuel')).toMatchObject({ value: '15', ratio: 0.15 });
    expect(byId(g.environment, 'tws')).toMatchObject({ value: '10.0', unit: 'kn' });
    expect(byId(g.environment, 'twd')).toMatchObject({ value: '112', detail: 'ESE' });
    expect(byId(g.environment, 'water-temp')?.value).toBe('27.2');
    expect(byId(g.environment, 'pressure')).toMatchObject({ value: '1013', unit: 'hPa' });
    expect(byId(g.environment, 'air-temp')).toMatchObject({ value: '—', missing: true });
  });

  it("ne dessine pas de jauge sans donnée et signale l'obsolescence", () => {
    const g = systemTiles(snapshot([reading('fuelLevel', 0.5, true)]), false);
    expect(byId(g.tanks, 'water')?.ratio).toBeUndefined();
    expect(byId(g.tanks, 'fuel')).toMatchObject({ stale: true, detail: 'il y a 15 s' });
    expect(byId(systemTiles(null, true).energy, 'soc')?.missing).toBe(true);
  });

  it('nomme les points cardinaux', () => {
    expect(cardinal(0)).toBe('N');
    expect(cardinal(Math.PI)).toBe('S');
    expect(cardinal((350 * Math.PI) / 180)).toBe('N');
    expect(cardinal((-90 * Math.PI) / 180)).toBe('O');
  });
});

describe('tuiles des instruments', () => {
  it('sépare valeur et unité', () => {
    const tiles = instrumentTiles(
      snapshot([
        reading('speedOverGround', 6.46 * KNOT),
        reading('courseOverGroundTrue', Math.PI / 2),
        reading('depthBelowTransducer', 12.34),
        reading('windAngleApparent', (-45 * Math.PI) / 180),
      ]),
      false,
    );
    expect(byId(tiles, 'sog')).toMatchObject({ value: '6.5', unit: 'kn', stale: false });
    expect(byId(tiles, 'cog')).toMatchObject({ value: '090', unit: '°' });
    expect(byId(tiles, 'depth')).toMatchObject({ value: '12.3', unit: 'm' });
    expect(byId(tiles, 'awa')).toMatchObject({ value: '045', unit: '° Bd' });
  });

  it("n'invente aucune valeur absente", () => {
    const tiles = instrumentTiles(null, false);
    expect(tiles).toHaveLength(7);
    expect(tiles.every((t) => t.missing && t.value === '—')).toBe(true);
  });

  it('signale les valeurs obsolètes et le silence du moteur', () => {
    const stale = instrumentTiles(snapshot([reading('speedOverGround', 1, true)]), false);
    expect(byId(stale, 'sog')).toMatchObject({ stale: true, detail: 'il y a 15 s' });
    const silent = instrumentTiles(snapshot([reading('speedOverGround', 1)]), true);
    expect(byId(silent, 'sog')?.stale).toBe(true);
  });

  it('préfère le cap vrai, sinon le cap magnétique étiqueté', () => {
    const magnetic = instrumentTiles(snapshot([reading('headingMagnetic', 0)]), false);
    expect(byId(magnetic, 'heading')).toMatchObject({ label: 'CAP MAGN.', value: '000' });
    const both = instrumentTiles(
      snapshot([reading('headingMagnetic', 0), reading('headingTrue', Math.PI)]),
      false,
    );
    expect(byId(both, 'heading')).toMatchObject({ label: 'CAP', value: '180' });
  });

  it('affiche la position ou son absence', () => {
    expect(positionTile(null, false)).toMatchObject({ missing: true, value: 'Aucune position' });
    const tile = positionTile(
      snapshot([], {
        fix: {
          latitudeDeg: -14.968,
          longitudeDeg: -147.638,
          cogRad: null,
          sogMps: null,
          source: 'simulator',
          sourceId: 'simulator',
          measuredAt: 0,
        },
        ageMs: 6000,
        stale: true,
        freshSources: 1,
      }),
      false,
    );
    expect(tile.value).toContain('14°58.080′ S');
    expect(tile).toMatchObject({ stale: true, wide: true, detail: 'simulator — il y a 6 s' });
  });
});

const status: NavigationStatus = {
  routeId: 'r',
  routeName: 'Tiputa → Avatoru',
  targetIndex: 0,
  waypointCount: 2,
  targetName: 'WP001',
  targetPosition: { latitudeDeg: 0, longitudeDeg: 0 },
  distanceToTargetM: 2778,
  bearingToTargetRad: Math.PI,
  crossTrackM: -92.6,
  remainingDistanceM: 5000,
  etaTarget: Date.UTC(2026, 10, 12, 14, 5),
  etaEnd: null,
  reachedWaypoint: null,
  finished: false,
  simulated: false,
  computedAt: 0,
};

describe('tuiles de navigation', () => {
  it('distance, relèvement, écart et ETA', () => {
    const tiles = navigationTiles(status, false);
    expect(byId(tiles, 'dtw')).toMatchObject({ value: '1.5', unit: 'NM', detail: 'vers WP001' });
    expect(byId(tiles, 'btw')).toMatchObject({ value: '180', unit: '°' });
    expect(byId(tiles, 'xte')).toMatchObject({ value: '0.05', unit: 'NM G' });
    expect(byId(tiles, 'eta')).toMatchObject({ value: '14:05', unit: 'UTC' });
  });

  it('valeurs inconnues et position obsolète', () => {
    const tiles = navigationTiles(
      { ...status, bearingToTargetRad: null, crossTrackM: null, etaTarget: null },
      true,
    );
    expect(tiles.every((t) => t.stale)).toBe(true);
    expect(byId(tiles, 'btw')).toMatchObject({ value: '—', missing: true });
    expect(byId(tiles, 'eta')).toMatchObject({ value: '—', missing: true });
    expect(navigationTiles(null, false)).toEqual([]);
  });
});
