import { describe, expect, it } from 'vitest';
import {
  WeatherApi,
  downloadArea,
  formatArea,
  formatUtc,
  parseBounds,
  parseProgress,
  parseWeatherFile,
  parseWindField,
  subscribeDownload,
} from './weather';

const file = {
  id: 'gfs.grb2',
  sizeBytes: 5504,
  referenceTime: '2026-11-12T00:00:00Z',
  validTimes: ['2026-11-12T00:00:00Z', '2026-11-12T03:00:00Z'],
  bounds: { west: -150, south: -18, east: -146, north: -14 },
  skipped: ['champ 0/1/8'],
  error: null,
};

const field = {
  id: 'gfs.grb2',
  referenceTime: '2026-11-12T00:00:00Z',
  validTime: '2026-11-12T03:00:00Z',
  step: 1,
  steps: 2,
  points: [{ lat: -16, lon: -148, speedMps: 7, directionRad: 1.5 }],
};

describe('parseurs', () => {
  it('lit un fichier météo', () => {
    const f = parseWeatherFile(file);
    expect(f.referenceTime).toBe(Date.UTC(2026, 10, 12));
    expect(f.validTimes).toHaveLength(2);
    expect(f.bounds?.west).toBe(-150);
    expect(f.skipped).toEqual(['champ 0/1/8']);
  });

  it('accepte un fichier inexploitable', () => {
    const f = parseWeatherFile({
      ...file,
      referenceTime: null,
      validTimes: [],
      bounds: null,
      error: 'pas de vent',
    });
    expect(f.error).toBe('pas de vent');
    expect(f.referenceTime).toBeNull();
  });

  it('rejette les réponses malformées', () => {
    expect(() => parseWeatherFile({ ...file, sizeBytes: -1 })).toThrow();
    expect(() => parseWeatherFile({ ...file, validTimes: 'x' })).toThrow();
    expect(() => parseWeatherFile(null)).toThrow();
    expect(() => parseBounds({ west: 0, south: 10, east: 1, north: 5 })).toThrow();
    expect(() => parseBounds({ west: 0, south: -95, east: 1, north: 5 })).toThrow();
  });

  it('lit un champ de vent et rejette les vecteurs hors bornes', () => {
    expect(parseWindField(field).points[0]?.speedMps).toBe(7);
    const bad = (p: Record<string, number>) =>
      parseWindField({ ...field, points: [{ ...field.points[0], ...p }] });
    expect(() => bad({ lat: 91 })).toThrow();
    expect(() => bad({ lon: 181 })).toThrow();
    expect(() => bad({ speedMps: -1 })).toThrow();
    expect(() => parseWindField({ ...field, steps: 0 })).toThrow();
  });

  it('lit la progression', () => {
    expect(parseProgress({ done: 3, total: 17 })).toEqual({ done: 3, total: 17 });
    expect(() => parseProgress({ done: 3, total: 0 })).toThrow();
  });
});

describe('formatage', () => {
  it('écrit la date en UTC', () => {
    expect(formatUtc(Date.UTC(2026, 10, 12, 9, 5))).toMatch(/12 nov\.? 09:05 UTC$/);
  });

  it('écrit une zone en clair', () => {
    expect(formatArea({ west: -152, south: -20, east: -144, north: -13 })).toBe(
      '20°S–13°S, 152°O–144°O',
    );
    expect(formatArea({ west: 5, south: 40, east: 10, north: 45 })).toBe('40°N–45°N, 5°E–10°E');
  });
});

describe('downloadArea', () => {
  it('conserve une zone autorisée', () => {
    const view = { west: -150, south: -18, east: -146, north: -14 };
    expect(downloadArea(view)).toBe(view);
  });

  it('réduit une zone trop grande autour de son centre', () => {
    const area = downloadArea({ west: -180, south: -80, east: 180, north: 80 });
    expect(area.east - area.west).toBe(120);
    expect(area.north - area.south).toBe(90);
    expect((area.north + area.south) / 2).toBe(0);
  });

  it("gère l'antiméridien", () => {
    const view = { west: 170, south: -20, east: -170, north: -10 };
    expect(downloadArea(view)).toBe(view);
    const wide = downloadArea({ west: 100, south: -10, east: -100, north: 10 });
    // 160° de large, centre 180° : réduit à 120°, de part et d'autre.
    expect(wide.west).toBe(120);
    expect(wide.east).toBe(-120);
  });
});

describe('WeatherApi', () => {
  it('transmet les arguments et valide les réponses', async () => {
    const calls: [string, unknown][] = [];
    const api = new WeatherApi((command, args) => {
      calls.push([command, args]);
      if (command === 'list_weather_files') return Promise.resolve([file]);
      if (command === 'import_weather_file') return Promise.resolve(null);
      if (command === 'download_weather') return Promise.resolve(file);
      if (command === 'get_wind_field') return Promise.resolve(field);
      return Promise.resolve(null);
    });
    expect(await api.list()).toHaveLength(1);
    expect(await api.importFile()).toBeNull();
    const area = { west: -150, south: -18, east: -146, north: -14 };
    expect((await api.download(area, 48)).id).toBe('gfs.grb2');
    await api.remove('gfs.grb2');
    expect((await api.windField('gfs.grb2', 1, null, 1500)).points).toHaveLength(1);
    expect(calls).toEqual([
      ['list_weather_files', undefined],
      ['import_weather_file', undefined],
      ['download_weather', { area, horizonH: 48 }],
      ['delete_weather_file', { id: 'gfs.grb2' }],
      ['get_wind_field', { id: 'gfs.grb2', step: 1, area: null, maxPoints: 1500 }],
    ]);
  });

  it('rejette une liste malformée', async () => {
    const api = new WeatherApi(() => Promise.resolve([{ id: 1 }]));
    await expect(api.list()).rejects.toThrow();
  });
});

describe('subscribeDownload', () => {
  it('ignore les progressions illisibles', async () => {
    const handlers: ((e: { payload: unknown }) => void)[] = [];
    const seen: unknown[] = [];
    await subscribeDownload(
      (p) => seen.push(p),
      (_event, h) => {
        handlers.push(h as (typeof handlers)[number]);
        return Promise.resolve(() => {});
      },
    );
    for (const h of handlers) {
      h({ payload: { done: 1, total: 2 } });
      h({ payload: 'n/a' });
    }
    expect(handlers).toHaveLength(1);
    expect(seen).toEqual([{ done: 1, total: 2 }]);
  });
});
