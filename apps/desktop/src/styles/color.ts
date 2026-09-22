/**
 * Conversão de cor e contraste WCAG.
 *
 * Os tokens do AISENSE são escritos em OKLCH (docs/08-design-system.md). Para
 * verificar contraste é preciso chegar na luminância relativa, então o caminho é
 * OKLCH → OKLab → LMS → sRGB linear → luminância.
 */

export interface Oklch {
  l: number;
  c: number;
  h: number;
  alpha: number;
}

const OKLCH_PATTERN = /^oklch\(\s*([\d.]+%?)\s+([\d.]+)\s+([\d.]+)\s*(?:\/\s*([\d.]+%?)\s*)?\)$/i;

export function parseOklch(value: string): Oklch | null {
  const match = OKLCH_PATTERN.exec(value.trim());
  if (!match) return null;
  const [, rawL, rawC, rawH, rawAlpha] = match;
  if (rawL === undefined || rawC === undefined || rawH === undefined) return null;

  const toNumber = (raw: string): number =>
    raw.endsWith('%') ? Number.parseFloat(raw) / 100 : Number.parseFloat(raw);

  return {
    l: toNumber(rawL),
    c: Number.parseFloat(rawC),
    h: Number.parseFloat(rawH),
    alpha: rawAlpha === undefined ? 1 : toNumber(rawAlpha),
  };
}

/** sRGB linear (0–1), já recortado para o gamut — é o que a tela mostra. */
export function oklchToLinearRgb({ l, c, h }: Oklch): [number, number, number] {
  const hRad = (h * Math.PI) / 180;
  const a = c * Math.cos(hRad);
  const b = c * Math.sin(hRad);

  const lCone = (l + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const mCone = (l - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const sCone = (l - 0.0894841775 * a - 1.291485548 * b) ** 3;

  const clamp = (n: number): number => Math.min(1, Math.max(0, n));
  return [
    clamp(4.0767416621 * lCone - 3.3077115913 * mCone + 0.2309699292 * sCone),
    clamp(-1.2684380046 * lCone + 2.6097574011 * mCone - 0.3413193965 * sCone),
    clamp(-0.0041960863 * lCone - 0.7034186147 * mCone + 1.707614701 * sCone),
  ];
}

/** Luminância relativa WCAG a partir de sRGB linear. */
export function relativeLuminance(color: Oklch): number {
  const [r, g, b] = oklchToLinearRgb(color);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** Razão de contraste WCAG entre duas cores opacas (1:1 a 21:1). */
export function contrastRatio(foreground: Oklch, background: Oklch): number {
  const a = relativeLuminance(foreground);
  const b = relativeLuminance(background);
  const [lighter, darker] = a > b ? [a, b] : [b, a];
  return (lighter + 0.05) / (darker + 0.05);
}

/** Componente sRGB linear → com gama, no formato de 8 bits. */
function encodeGamma(channel: number): number {
  const value = channel <= 0.0031308 ? channel * 12.92 : 1.055 * channel ** (1 / 2.4) - 0.055;
  return Math.round(Math.min(1, Math.max(0, value)) * 255);
}

/**
 * OKLCH → `#rrggbb`.
 *
 * O xterm.js não entende `oklch()`: ele aceita hex, `rgb()` e nomes. Como nossos
 * tokens são todos OKLCH, a conversão acontece aqui antes de montar o tema do
 * terminal.
 */
export function toHex(color: Oklch): string {
  const [r, g, b] = oklchToLinearRgb(color);
  const hex = [r, g, b].map((channel) => encodeGamma(channel).toString(16).padStart(2, '0'));
  return `#${hex.join('')}`;
}
