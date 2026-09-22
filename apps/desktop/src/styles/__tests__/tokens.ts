import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { type Oklch, parseOklch } from '@/styles/color';

const TOKENS_PATH = join(dirname(fileURLToPath(import.meta.url)), '..', 'tokens.css');

export type ThemeName = 'light' | 'dark';

/** Remove comentários: um `:` dentro deles confunde o parser de declarações. */
function stripComments(css: string): string {
  return css.replace(/\/\*[\s\S]*?\*\//g, '');
}

/** Lê um bloco `seletor { ... }` do CSS e devolve os pares `--token: valor`. */
function readBlock(rawCss: string, selector: string): Record<string, string> {
  const css = stripComments(rawCss);
  const start = css.indexOf(selector);
  if (start === -1) throw new Error(`Seletor não encontrado em tokens.css: ${selector}`);
  const open = css.indexOf('{', start);
  const close = css.indexOf('}', open);
  const body = css.slice(open + 1, close);

  const declarations: Record<string, string> = {};
  for (const line of body.split(';')) {
    const [rawName, ...rest] = line.split(':');
    if (rawName === undefined || rest.length === 0) continue;
    const name = rawName.trim();
    if (!name.startsWith('--')) continue;
    declarations[name] = rest.join(':').trim();
  }
  return declarations;
}

/**
 * Todos os tokens de um tema, com `var(--x)` já resolvido para o valor final.
 * O tema escuro herda do `:root` e sobrescreve o que redefine.
 */
export function loadTheme(theme: ThemeName): Record<string, Oklch> {
  const css = readFileSync(TOKENS_PATH, 'utf8');
  const raw = readBlock(css, ':root {');
  if (theme === 'dark') Object.assign(raw, readBlock(css, ":root[data-theme='dark']"));

  const resolve = (value: string, seen = new Set<string>()): string => {
    const reference = /^var\(\s*(--[\w-]+)\s*\)$/.exec(value.trim());
    if (!reference) return value;
    const name = reference[1];
    if (name === undefined || seen.has(name)) {
      throw new Error(`Referência circular ou ausente em tokens.css: ${value}`);
    }
    seen.add(name);
    const target = raw[name];
    if (target === undefined) throw new Error(`Token inexistente em tokens.css: ${name}`);
    return resolve(target, seen);
  };

  const resolved: Record<string, Oklch> = {};
  for (const [name, value] of Object.entries(raw)) {
    const color = parseOklch(resolve(value));
    if (color) resolved[name] = color;
  }
  return resolved;
}
