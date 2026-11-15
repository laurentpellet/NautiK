/**
 * Profil d'une trace pour sa relecture (moteur) : statistiques et vitesse
 * de chaque point. Aucune vitesse n'est recalculée ici.
 */
import { invoke } from '@tauri-apps/api/core';
import type { InvokeFn } from './engine';
import {
  EngineProtocolError,
  expectArray,
  expectNumber,
  expectPosition,
  expectRecord,
  expectTimestamp,
  optional,
  type LatLon,
} from './validate';

export interface ProfilePoint {
  at: number;
  position: LatLon;
  speedMps: number | null;
  depthM: number | null;
  distanceM: number;
}

export interface TrackStats {
  distanceM: number;
  durationS: number;
  movingS: number;
  averageMps: number | null;
  movingAverageMps: number | null;
  maxMps: number | null;
  minDepthM: number | null;
}

export interface TrackProfile {
  stats: TrackStats;
  points: ProfilePoint[];
}

const num = (what: string) => (v: unknown) => expectNumber(v, what);

function nonNegative(value: unknown, what: string): number {
  const n = expectNumber(value, what);
  if (n < 0) throw new EngineProtocolError(`${what} : valeur négative`);
  return n;
}

export function parseProfilePoint(value: unknown): ProfilePoint {
  const r = expectRecord(value, 'point');
  return {
    at: expectTimestamp(r.at, 'point.at'),
    position: expectPosition(r.position, 'point.position'),
    speedMps: optional(r.speedMps, (v) => nonNegative(v, 'point.speedMps')),
    depthM: optional(r.depthM, num('point.depthM')),
    distanceM: nonNegative(r.distanceM, 'point.distanceM'),
  };
}

export function parseTrackProfile(value: unknown): TrackProfile {
  const r = expectRecord(value, 'profil');
  const s = expectRecord(r.stats, 'profil.stats');
  const points = expectArray(r.points, 'profil.points', parseProfilePoint);
  for (let i = 1; i < points.length; i += 1) {
    if ((points[i]?.at ?? 0) < (points[i - 1]?.at ?? 0)) {
      throw new EngineProtocolError('profil.points : ordre chronologique attendu');
    }
  }
  return {
    stats: {
      distanceM: nonNegative(s.distanceM, 'stats.distanceM'),
      durationS: nonNegative(s.durationS, 'stats.durationS'),
      movingS: nonNegative(s.movingS, 'stats.movingS'),
      averageMps: optional(s.averageMps, (v) => nonNegative(v, 'stats.averageMps')),
      movingAverageMps: optional(s.movingAverageMps, (v) =>
        nonNegative(v, 'stats.movingAverageMps'),
      ),
      maxMps: optional(s.maxMps, (v) => nonNegative(v, 'stats.maxMps')),
      minDepthM: optional(s.minDepthM, num('stats.minDepthM')),
    },
    points,
  };
}

export async function getTrackProfile(id: string, call: InvokeFn = invoke): Promise<TrackProfile> {
  return parseTrackProfile(await call('get_track_profile', { id }));
}
