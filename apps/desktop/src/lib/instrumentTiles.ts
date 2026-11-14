/**
 * Tuiles de l'onglet Instruments : valeurs en grands caractères, unités
 * séparées, fraîcheur visible.
 *
 * Mise en forme seulement : les valeurs, leur fraîcheur et l'état de
 * navigation sont calculés par le moteur. Une donnée absente s'affiche
 * « — », jamais une valeur par défaut.
 */
import {
  depthParts,
  distanceParts,
  formatAge,
  formatDuration,
  formatLatitude,
  formatLongitude,
  formatUtcClock,
  speedParts,
} from './format';
import { DEPTH_UNITS, DISTANCE_UNITS, SPEED_UNITS, units } from './units.svelte';
import type { InstrumentsSnapshot, Quantity, Reading } from './services/instruments';
import type { NavigationStatus } from './services/routing';

export interface Tile {
  id: string;
  label: string;
  value: string;
  unit: string;
  /** Donnée obsolète (ou moteur muet) : affichée estompée et signalée. */
  stale: boolean;
  /** Aucune donnée. */
  missing: boolean;
  /** Précision sous la valeur (âge, waypoint visé…). */
  detail: string | null;
  /** Tuile double largeur. */
  wide?: boolean;
  /** Niveau (réservoir, charge) dans [0, 1], pour une jauge. */
  ratio?: number;
}

const MISSING = '—';

function degrees(radians: number): string {
  const d = Math.round(((((radians * 180) / Math.PI) % 360) + 360) % 360) % 360;
  return String(d).padStart(3, '0');
}

function distance(meters: number): string {
  return distanceParts(meters).value;
}

function readingTile(
  id: string,
  label: string,
  reading: Reading | undefined,
  silent: boolean,
  format: (value: number) => { value: string; unit: string },
  emptyUnit: string,
): Tile {
  if (!reading) {
    return {
      id,
      label,
      value: MISSING,
      unit: emptyUnit,
      stale: false,
      missing: true,
      detail: null,
    };
  }
  const stale = silent || reading.stale;
  return {
    id,
    label,
    ...format(reading.value),
    stale,
    missing: false,
    detail: stale ? `il y a ${formatAge(reading.ageMs)}` : null,
  };
}

/** Tuiles des instruments du bord, dans l'ordre d'affichage. */
export function instrumentTiles(snapshot: InstrumentsSnapshot | null, silent: boolean): Tile[] {
  const byQuantity = new Map<Quantity, Reading>(
    (snapshot?.readings ?? []).map((r) => [r.quantity, r]),
  );
  const get = (q: Quantity) => byQuantity.get(q);
  const speed = speedParts;
  const kn = SPEED_UNITS[units.speed].label;
  const angle = (v: number) => ({ value: degrees(v), unit: '°' });

  const headingTrue = get('headingTrue');
  const heading = headingTrue ?? get('headingMagnetic');

  return [
    readingTile('sog', 'SOG', get('speedOverGround'), silent, speed, kn),
    readingTile('cog', 'COG', get('courseOverGroundTrue'), silent, angle, '°'),
    readingTile(
      'heading',
      headingTrue || !heading ? 'CAP' : 'CAP MAGN.',
      heading,
      silent,
      angle,
      '°',
    ),
    readingTile(
      'depth',
      'FOND',
      get('depthBelowTransducer'),
      silent,
      depthParts,
      DEPTH_UNITS[units.depth].label,
    ),
    readingTile('stw', 'SURFACE', get('speedThroughWater'), silent, speed, kn),
    readingTile('aws', 'VENT APP.', get('windSpeedApparent'), silent, speed, kn),
    readingTile(
      'awa',
      'ANGLE VENT',
      get('windAngleApparent'),
      silent,
      (v) => {
        const d = Math.round((Math.abs(v) * 180) / Math.PI);
        const side = d === 0 || d === 180 ? '°' : v > 0 ? '° Td' : '° Bd';
        return { value: String(d).padStart(3, '0'), unit: side };
      },
      '°',
    ),
  ];
}

const KELVIN = 273.15;
const CARDINALS = [
  'N',
  'NNE',
  'NE',
  'ENE',
  'E',
  'ESE',
  'SE',
  'SSE',
  'S',
  'SSO',
  'SO',
  'OSO',
  'O',
  'ONO',
  'NO',
  'NNO',
] as const;

/** Point cardinal (16 secteurs) d'un angle vrai en radians. */
export function cardinal(radians: number): string {
  const deg = ((((radians * 180) / Math.PI) % 360) + 360) % 360;
  return CARDINALS[Math.round(deg / 22.5) % 16] ?? 'N';
}

export interface SystemTiles {
  engine: Tile[];
  energy: Tile[];
  tanks: Tile[];
  environment: Tile[];
}

/** Tuiles moteur, énergie, réservoirs et environnement. */
export function systemTiles(snapshot: InstrumentsSnapshot | null, silent: boolean): SystemTiles {
  const byQuantity = new Map<Quantity, Reading>(
    (snapshot?.readings ?? []).map((r) => [r.quantity, r]),
  );
  const tile = (
    id: string,
    label: string,
    q: Quantity,
    format: (v: number) => { value: string; unit: string },
    emptyUnit: string,
  ) => readingTile(id, label, byQuantity.get(q), silent, format, emptyUnit);
  const gauge = (id: string, label: string, q: Quantity): Tile => {
    const t = tile(id, label, q, (v) => ({ value: String(Math.round(v * 100)), unit: '%' }), '%');
    const r = byQuantity.get(q);
    return r ? { ...t, ratio: Math.min(Math.max(r.value, 0), 1) } : t;
  };
  const celsius = (digits: number) => (k: number) => ({
    value: (k - KELVIN).toFixed(digits),
    unit: '°C',
  });
  const twd = tile(
    'twd',
    'DIR. VENT RÉEL',
    'windDirectionTrue',
    (v) => ({ value: degrees(v), unit: '°' }),
    '°',
  );
  const twdReading = byQuantity.get('windDirectionTrue');
  return {
    engine: [
      tile(
        'rpm',
        'RÉGIME',
        'engineSpeed',
        (hz) => ({ value: String(Math.round((hz * 60) / 10) * 10), unit: 'tr/min' }),
        'tr/min',
      ),
      tile('coolant', 'TEMP. MOTEUR', 'engineCoolantTemperature', celsius(0), '°C'),
      tile(
        'oil',
        'PRESSION HUILE',
        'engineOilPressure',
        (pa) => ({ value: (pa / 100_000).toFixed(1), unit: 'bar' }),
        'bar',
      ),
    ],
    energy: [
      gauge('soc', 'BATTERIE', 'batteryStateOfCharge'),
      tile(
        'voltage',
        'TENSION',
        'batteryVoltage',
        (v) => ({ value: v.toFixed(2), unit: 'V' }),
        'V',
      ),
      tile(
        'current',
        'COURANT',
        'batteryCurrent',
        (a) => ({ value: `${a > 0 ? '+' : ''}${a.toFixed(1)}`, unit: 'A' }),
        'A',
      ),
      tile(
        'solar',
        'SOLAIRE',
        'solarPower',
        (w) => ({ value: String(Math.round(w)), unit: 'W' }),
        'W',
      ),
    ],
    tanks: [
      gauge('fuel', 'CARBURANT', 'fuelLevel'),
      gauge('water', 'EAU DOUCE', 'freshWaterLevel'),
    ],
    environment: [
      tile('tws', 'VENT RÉEL', 'windSpeedTrue', speedParts, SPEED_UNITS[units.speed].label),
      twdReading && !twd.stale ? { ...twd, detail: cardinal(twdReading.value) } : twd,
      tile('water-temp', 'EAU', 'waterTemperature', celsius(1), '°C'),
      tile('air-temp', 'AIR', 'airTemperature', celsius(1), '°C'),
      tile(
        'pressure',
        'PRESSION',
        'atmosphericPressure',
        (pa) => ({ value: (pa / 100).toFixed(0), unit: 'hPa' }),
        'hPa',
      ),
    ],
  };
}

/** Tuile de position (double largeur). */
export function positionTile(snapshot: InstrumentsSnapshot | null, silent: boolean): Tile {
  const position = snapshot?.position;
  if (!position) {
    return {
      id: 'position',
      label: 'POSITION',
      value: 'Aucune position',
      unit: '',
      stale: false,
      missing: true,
      detail: null,
      wide: true,
    };
  }
  const stale = silent || position.stale;
  return {
    id: 'position',
    label: 'POSITION',
    value: `${formatLatitude(position.fix.latitudeDeg)}  ${formatLongitude(position.fix.longitudeDeg)}`,
    unit: '',
    stale,
    missing: false,
    detail: `${position.fix.sourceId} — il y a ${formatAge(position.ageMs)}`,
    wide: true,
  };
}

/**
 * Tuiles de navigation (route active). `positionStale` : la position
 * qui sert au calcul est obsolète, les valeurs le sont aussi.
 */
export function navigationTiles(status: NavigationStatus | null, positionStale: boolean): Tile[] {
  if (!status) return [];
  const tile = (id: string, label: string, value: string, unit: string, detail: string | null) => ({
    id,
    label,
    value,
    unit,
    stale: positionStale,
    missing: value === MISSING,
    detail,
  });
  const xte = status.crossTrackM;
  const dist = DISTANCE_UNITS[units.distance].label;
  const xteUnit = xte === null || xte === 0 ? dist : xte > 0 ? `${dist} D` : `${dist} G`;
  let eta = MISSING;
  if (status.etaTarget !== null) {
    const d = new Date(status.etaTarget);
    eta = `${String(d.getUTCHours()).padStart(2, '0')}:${String(d.getUTCMinutes()).padStart(2, '0')}`;
  }
  const target = `vers ${status.targetName}`;
  return [
    tile('dtw', 'DISTANCE WP', distance(status.distanceToTargetM), dist, target),
    tile(
      'btw',
      'RELÈVEMENT WP',
      status.bearingToTargetRad === null ? MISSING : degrees(status.bearingToTargetRad),
      '°',
      target,
    ),
    tile('xte', 'ÉCART ROUTE', xte === null ? MISSING : distance(Math.abs(xte)), xteUnit, null),
    tile('eta', 'ETA WP', eta, 'UTC', status.finished ? 'route terminée' : target),
  ];
}

/**
 * Éléments de la barre de route : distance, relèvement, écart, temps et
 * heures d'arrivée au waypoint puis en fin de route. Les valeurs viennent
 * du moteur ; seuls les temps restants (ETA − maintenant) sont déduits pour
 * l'affichage. `positionStale` : valeurs obsolètes, estompées et signalées.
 */
export function routeBarTiles(
  status: NavigationStatus | null,
  now: number,
  positionStale: boolean,
): Tile[] {
  if (!status) return [];
  const tile = (
    id: string,
    label: string,
    value: string,
    unit: string,
    detail: string | null,
  ): Tile => ({
    id,
    label,
    value,
    unit,
    stale: positionStale && value !== MISSING,
    missing: value === MISSING,
    detail: positionStale ? `calculé il y a ${formatAge(now - status.computedAt)}` : detail,
  });
  const dist = DISTANCE_UNITS[units.distance].label;
  const xte = status.crossTrackM;
  const xteUnit = xte === null || xte === 0 ? dist : xte > 0 ? `${dist} D` : `${dist} G`;
  const clock = (time: number | null) => (time === null ? MISSING : formatUtcClock(time));
  const togo = (time: number | null) => (time === null ? MISSING : formatDuration(time - now));
  const noSpeed = 'vitesse fond nulle ou inconnue';
  return [
    tile('dtw', 'DTW', distance(status.distanceToTargetM), dist, 'Distance au waypoint'),
    tile(
      'btw',
      'BTW',
      status.bearingToTargetRad === null ? MISSING : degrees(status.bearingToTargetRad),
      '°',
      'Relèvement vrai du waypoint',
    ),
    tile(
      'xte',
      'XTE',
      xte === null ? MISSING : distance(Math.abs(xte)),
      xteUnit,
      'Écart de route (D : à droite, G : à gauche)',
    ),
    tile(
      'ttg',
      'TEMPS WP',
      togo(status.etaTarget),
      '',
      status.etaTarget === null ? noSpeed : 'Temps restant jusqu’au waypoint',
    ),
    tile('eta', 'ETA WP', clock(status.etaTarget), 'UTC', 'Heure d’arrivée au waypoint'),
    tile(
      'remaining',
      'RESTE',
      distance(status.remainingDistanceM),
      dist,
      'Distance restante jusqu’à la fin de la route',
    ),
    tile(
      'ttgEnd',
      'DURÉE',
      togo(status.etaEnd),
      '',
      status.etaEnd === null ? noSpeed : 'Durée restante jusqu’à la fin de la route',
    ),
    tile('etaEnd', 'ETA FIN', clock(status.etaEnd), 'UTC', 'Heure d’arrivée en fin de route'),
  ];
}
