import { describe, expect, it } from 'vitest';
import { decodeChunk } from '../decode';

/**
 * Compara como lista de números.
 *
 * No jsdom, `TextEncoder` vem do realm do Node e `Uint8Array` do realm da janela,
 * então `toEqual` reprova dois arrays idênticos ("no visual difference"). Comparar
 * os números tira o realm da jogada sem afrouxar o teste.
 */
function bytes(value: Uint8Array): number[] {
  return Array.from(value);
}

/** Base64 de um trecho de bytes, como o core faz. */
function encode(bytes: Uint8Array): string {
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

describe('decodeChunk', () => {
  it('devolve exatamente os bytes recebidos', () => {
    const original = new TextEncoder().encode('ls -la\r\n');
    expect(bytes(decodeChunk(encode(original)))).toEqual(bytes(original));
  });

  it('preserva sequências ANSI', () => {
    const original = new TextEncoder().encode('\u001b[31merro\u001b[0m\r\n');
    expect(bytes(decodeChunk(encode(original)))).toEqual(bytes(original));
  });

  it('não corrompe um caractere partido entre dois chunks', () => {
    // É o caso que justifica o base64: o PTY entrega pedaços arbitrários.
    const texto = 'configuração 🚀 pronta';
    const codificado = new TextEncoder().encode(texto);

    // Corta no meio do 'ç' (2 bytes) para forçar o pior caso.
    const corte = texto.indexOf('ç') + 1;
    const primeiro = codificado.slice(0, corte);
    const segundo = codificado.slice(corte);

    const remontado = new Uint8Array([
      ...decodeChunk(encode(primeiro)),
      ...decodeChunk(encode(segundo)),
    ]);

    expect(bytes(remontado)).toEqual(bytes(codificado));
    expect(new TextDecoder().decode(remontado)).toBe(texto);
  });

  it('demonstra por que texto puro não serviria', () => {
    // Se cada chunk virasse String antes de trafegar, o caractere partido seria
    // substituído por U+FFFD e o dano seria irreversível. Este teste documenta o
    // motivo da decisão para quem pensar em "simplificar" para string.
    const codificado = new TextEncoder().encode('ção');
    const decoder = new TextDecoder();
    const ingenuo = decoder.decode(codificado.slice(0, 1)) + decoder.decode(codificado.slice(1));

    expect(ingenuo).not.toBe('ção');
    expect(ingenuo).toContain('�');
  });

  it('aceita chunk vazio', () => {
    expect(bytes(decodeChunk(''))).toEqual([]);
  });

  it('decodifica um chunk grande sem perder byte', () => {
    const original = new Uint8Array(64 * 1024);
    for (let index = 0; index < original.length; index += 1) original[index] = index % 256;
    expect(bytes(decodeChunk(encode(original)))).toEqual(bytes(original));
  });
});
