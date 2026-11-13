/**
 * Géométrie de l'onglet Instruments : cadran de vent apparent et
 * courbe de tendance. Fonctions pures (SVG), sans calcul de navigation.
 *
 * Cadran : repère SVG 200 × 200, centre (100, 100). Un angle relatif
 * (radians, positif à tribord) se lit dans le sens horaire depuis
 * l'étrave, en haut.
 */

export const DIAL_CENTER = 100;
export const DIAL_RADIUS = 92;

export interface Point {
  x: number;
  y: number;
}

/** Point du cadran à l'angle relatif `angleRad` et au rayon `r`. */
export function dialPoint(angleRad: number, r: number): Point {
  return {
    x: DIAL_CENTER + r * Math.sin(angleRad),
    y: DIAL_CENTER - r * Math.cos(angleRad),
  };
}

const f = (n: number) => n.toFixed(2);

/** Arc de cercle de `fromDeg` à `toDeg` (sens horaire), rayon `r`. */
export function arcPath(fromDeg: number, toDeg: number, r: number): string {
  const a = dialPoint((fromDeg * Math.PI) / 180, r);
  const b = dialPoint((toDeg * Math.PI) / 180, r);
  const large = Math.abs(toDeg - fromDeg) > 180 ? 1 : 0;
  return `M ${f(a.x)} ${f(a.y)} A ${r} ${r} 0 ${large} 1 ${f(b.x)} ${f(b.y)}`;
}

export interface Tick {
  angleDeg: number;
  major: boolean;
  /** Graduation majeure : valeur affichée (0 à 180, symétrique). */
  label: string | null;
  inner: Point;
  outer: Point;
  labelAt: Point;
}

/** Graduations tous les 10°, majeures tous les 30°. */
export function dialTicks(): Tick[] {
  const ticks: Tick[] = [];
  for (let deg = 0; deg < 360; deg += 10) {
    const major = deg % 30 === 0;
    const rad = (deg * Math.PI) / 180;
    const shown = deg <= 180 ? deg : 360 - deg;
    ticks.push({
      angleDeg: deg,
      major,
      label: major && deg !== 0 ? String(shown) : null,
      inner: dialPoint(rad, major ? DIAL_RADIUS - 9 : DIAL_RADIUS - 5),
      outer: dialPoint(rad, DIAL_RADIUS),
      labelAt: dialPoint(rad, DIAL_RADIUS - 19),
    });
  }
  return ticks;
}

/** Graduations d'un compas (nord en haut) : points cardinaux en étiquette. */
export function compassTicks(): Tick[] {
  const CARDINALS: Record<number, string> = { 0: 'N', 90: 'E', 180: 'S', 270: 'O' };
  const ticks: Tick[] = [];
  for (let deg = 0; deg < 360; deg += 10) {
    const major = deg % 30 === 0;
    const rad = (deg * Math.PI) / 180;
    ticks.push({
      angleDeg: deg,
      major,
      label: CARDINALS[deg] ?? null,
      inner: dialPoint(rad, major ? DIAL_RADIUS - 10 : DIAL_RADIUS - 5),
      outer: dialPoint(rad, DIAL_RADIUS),
      labelAt: dialPoint(rad, DIAL_RADIUS - 20),
    });
  }
  return ticks;
}

/** Repère du cap sur le bord du compas (triangle pointant vers le centre). */
export function headingMarker(angleRad: number): string {
  const tip = dialPoint(angleRad, DIAL_RADIUS - 14);
  const left = dialPoint(angleRad - 0.12, DIAL_RADIUS + 2);
  const right = dialPoint(angleRad + 0.12, DIAL_RADIUS + 2);
  return `M ${f(tip.x)} ${f(tip.y)} L ${f(left.x)} ${f(left.y)} L ${f(right.x)} ${f(right.y)} Z`;
}

/** Secteurs colorés : près (25° à 60°) et portant (120° à 160°). */
export const DIAL_SECTORS = [
  { side: 'port', fromDeg: -60, toDeg: -25 },
  { side: 'port', fromDeg: -160, toDeg: -120 },
  { side: 'starboard', fromDeg: 25, toDeg: 60 },
  { side: 'starboard', fromDeg: 120, toDeg: 160 },
] as const;

/** Flèche du vent apparent : pointe vers le centre depuis le bord. */
export function windArrow(angleRad: number): string {
  const tip = dialPoint(angleRad, DIAL_RADIUS - 52);
  const base = DIAL_RADIUS - 14;
  const left = dialPoint(angleRad - 0.16, base);
  const right = dialPoint(angleRad + 0.16, base);
  return `M ${f(tip.x)} ${f(tip.y)} L ${f(left.x)} ${f(left.y)} L ${f(right.x)} ${f(right.y)} Z`;
}

// --- Tendance -------------------------------------------------------------

export interface Sample {
  /** Horodatage (ms). */
  t: number;
  /** Valeur SI. */
  v: number;
}

export interface TrendStats {
  avg: number;
  max: number;
  last: number;
}

/**
 * Ajoute une mesure (si elle est plus récente que la dernière) et
 * retire celles sorties de la fenêtre. Retourne un nouveau tableau.
 */
export function appendSample(samples: Sample[], sample: Sample, windowMs: number): Sample[] {
  const last = samples.at(-1);
  if (last && sample.t <= last.t) return samples;
  const oldest = sample.t - windowMs;
  const first = samples.findIndex((s) => s.t >= oldest);
  return [...(first < 0 ? [] : samples.slice(first)), sample];
}

/** Échantillons de la fenêtre `[now - windowMs, now]`. */
export function inWindow(samples: Sample[], now: number, windowMs: number): Sample[] {
  return samples.filter((s) => s.t >= now - windowMs && s.t <= now);
}

export function trendStats(samples: Sample[]): TrendStats | null {
  const last = samples.at(-1);
  if (!last) return null;
  let sum = 0;
  let max = -Infinity;
  for (const s of samples) {
    sum += s.v;
    max = Math.max(max, s.v);
  }
  return { avg: sum / samples.length, max, last: last.v };
}

/** Échelle verticale : maximum arrondi au multiple de `step` supérieur. */
export function trendScale(max: number, step: number): number {
  return Math.max(step, Math.ceil(max / step) * step);
}

export interface TrendFrame {
  now: number;
  windowMs: number;
  /** Valeur en haut de l'échelle (en bas si `invert`). */
  yMax: number;
  /** Valeur en bas de l'échelle (0 par défaut). */
  yMin?: number;
  width: number;
  height: number;
  /** Trou (ms) au-delà duquel la courbe est interrompue. */
  gapMs?: number;
  /** Saut de valeur qui interrompt la courbe (passage 359° → 0°). */
  maxJump?: number;
  /** Axe vertical inversé (profondeur : 0 en haut). */
  invert?: boolean;
}

/**
 * Tracé SVG de la courbe (`width` × `height`), le temps de gauche à
 * droite. Un trou de plus de `gapMs` (donnée absente ou obsolète)
 * interrompt la courbe : aucune valeur n'est interpolée.
 */
export function trendPath(samples: Sample[], frame: TrendFrame): string {
  const { now, windowMs, yMax, width, height } = frame;
  const gapMs = frame.gapMs ?? 5_000;
  const maxJump = frame.maxJump ?? Infinity;
  const x = (t: number) => f(((t - (now - windowMs)) / windowMs) * width);
  const yMin = frame.yMin ?? 0;
  const span = yMax - yMin || 1;
  const y = (v: number) => {
    const ratio = (Math.min(Math.max(v, yMin), yMax) - yMin) / span;
    return f(frame.invert ? ratio * height : height - ratio * height);
  };
  let path = '';
  let previous: Sample | null = null;
  for (const s of samples) {
    const move =
      previous === null || s.t - previous.t > gapMs || Math.abs(s.v - previous.v) > maxJump;
    path += `${move ? 'M' : 'L'} ${x(s.t)} ${y(s.v)} `;
    previous = s;
  }
  return path.trim();
}
