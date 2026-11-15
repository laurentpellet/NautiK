import { describe, expect, it } from 'vitest';
import {
  LABEL_IMAGE_PREFIX,
  MAX_LABEL_CHARS,
  labelImageExpression,
  labelText,
  parseLabelImageId,
} from './labels';

describe('étiquettes de la carte', () => {
  it('normalise et borne le texte', () => {
    expect(labelText('  Passe\tTiputa \n')).toBe('Passe Tiputa');
    expect(labelText('a\u0000b')).toBe('a b');
    const long = labelText('x'.repeat(100));
    expect([...long]).toHaveLength(MAX_LABEL_CHARS);
    expect(long.endsWith('…')).toBe(true);
    expect(labelText('Île de Pâques')).toBe('Île de Pâques');
  });

  it('construit et reconnaît les identifiants d’image', () => {
    expect(labelImageExpression('night', 'island')).toEqual([
      'concat',
      `${LABEL_IMAGE_PREFIX}night:island:`,
      ['get', 'label'],
    ]);
    expect(parseLabelImageId(`${LABEL_IMAGE_PREFIX}day:wp:Départ`)).toEqual({
      theme: 'day',
      style: 'wp',
      text: 'Départ',
    });
    expect(parseLabelImageId(`${LABEL_IMAGE_PREFIX}day:mark:a:b`)?.text).toBe('a:b');
  });

  it('refuse les identifiants inconnus ou trop longs', () => {
    expect(parseLabelImageId('enc-sounding-day-deep-12')).toBeNull();
    expect(parseLabelImageId(`${LABEL_IMAGE_PREFIX}dusk:wp:x`)).toBeNull();
    expect(parseLabelImageId(`${LABEL_IMAGE_PREFIX}day:other:x`)).toBeNull();
    expect(parseLabelImageId(`${LABEL_IMAGE_PREFIX}day:wp:`)).toBeNull();
    expect(parseLabelImageId(`${LABEL_IMAGE_PREFIX}day:wp:${'x'.repeat(41)}`)).toBeNull();
  });
});
