/**
 * Étiquettes de texte de la carte (noms des waypoints, des îles, des
 * amers) sans polices hors ligne : chaque texte devient une image créée à
 * la demande (`styleimagemissing`), comme les sondes ENC.
 *
 * L'identifiant d'image porte le thème, le style et le texte ; le texte
 * est borné en longueur, et les images ne sont créées que pour des
 * identifiants reconnus.
 */
import type { ExpressionSpecification } from 'maplibre-gl';
import type { Theme } from './style';

export const LABEL_IMAGE_PREFIX = 'nautik-label:';

/** Styles d'étiquette : waypoint, île (italique), amer, longueur de segment (italique). */
export type LabelStyle = 'wp' | 'island' | 'mark' | 'leg';

const STYLES: LabelStyle[] = ['wp', 'island', 'mark', 'leg'];

/** Longueur maximale affichée (caractères) ; au-delà, « … ». */
export const MAX_LABEL_CHARS = 40;

/** Texte d'étiquette : espaces normalisés, contrôles retirés, borné. */
export function labelText(name: string): string {
  const clean = name
    // eslint-disable-next-line no-control-regex
    .replace(/[\u0000-\u001f\u007f]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
  const chars = [...clean];
  return chars.length > MAX_LABEL_CHARS
    ? `${chars.slice(0, MAX_LABEL_CHARS - 1).join('')}…`
    : clean;
}

/** Expression MapLibre de l'image d'étiquette (propriété `label` déjà bornée). */
export function labelImageExpression(theme: Theme, style: LabelStyle): ExpressionSpecification {
  return ['concat', `${LABEL_IMAGE_PREFIX}${theme}:${style}:`, ['get', 'label']];
}

/** Décompose un identifiant d'image d'étiquette ; `null` si ce n'en est pas une. */
export function parseLabelImageId(
  id: string,
): { theme: Theme; style: LabelStyle; text: string } | null {
  if (!id.startsWith(LABEL_IMAGE_PREFIX)) return null;
  const rest = id.slice(LABEL_IMAGE_PREFIX.length);
  const m = /^(day|night):(wp|island|mark|leg):(.+)$/s.exec(rest);
  if (!m) return null;
  const text = m[3] ?? '';
  const style = m[2] as LabelStyle;
  if (!STYLES.includes(style) || [...text].length > MAX_LABEL_CHARS) return null;
  return { theme: m[1] as Theme, style, text };
}

const COLORS: Record<Theme, Record<LabelStyle, string> & { halo: string }> = {
  day: { wp: '#4a148c', island: '#3b3b3b', mark: '#0d47a1', leg: '#212121', halo: '#ffffff' },
  night: { wp: '#ce93d8', island: '#bdb39c', mark: '#90caf9', leg: '#ffe082', halo: '#081422' },
};

/** Image d'une étiquette ; `null` si le canevas est indisponible. */
export function labelImage(
  text: string,
  style: LabelStyle,
  theme: Theme,
  ratio = 1,
): ImageData | null {
  const colors = COLORS[theme];
  const fontPx = (style === 'island' ? 13 : 11) * ratio;
  const canvas = document.createElement('canvas');
  const ctx = canvas.getContext('2d');
  if (!ctx) return null;
  const font = `${style === 'island' || style === 'leg' ? 'italic ' : ''}${style === 'wp' ? 'bold ' : '600 '}${fontPx}px sans-serif`;
  ctx.font = font;
  const width = Math.max(1, Math.ceil(ctx.measureText(text).width + 6 * ratio));
  const height = Math.ceil(fontPx + 6 * ratio);
  canvas.width = width;
  canvas.height = height;
  ctx.font = font;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.lineJoin = 'round';
  ctx.lineWidth = 3 * ratio;
  ctx.strokeStyle = colors.halo;
  ctx.strokeText(text, width / 2, height / 2);
  ctx.fillStyle = colors[style];
  ctx.fillText(text, width / 2, height / 2);
  return ctx.getImageData(0, 0, width, height);
}
