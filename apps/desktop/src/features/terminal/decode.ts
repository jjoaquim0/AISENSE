/**
 * Decodificação dos chunks de terminal.
 *
 * O core manda os bytes do PTY em base64, e não como texto, porque um caractere
 * UTF-8 pode ser partido entre duas leituras: `ção` vira `\xc3` numa leitura e
 * `\xa7ão` na seguinte. Converter cada leitura para string no Rust comeria o
 * caractere e o terminal mostraria `�`.
 *
 * Aqui só desfazemos o base64; remontar a sequência é com o xterm, que mantém um
 * decodificador incremental.
 */
export function decodeChunk(base64: string): Uint8Array {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}
