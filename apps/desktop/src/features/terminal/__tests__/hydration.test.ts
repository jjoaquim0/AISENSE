import { describe, expect, it } from 'vitest';
import { HydrationGate } from '../hydration';

const bytes = (text: string) => new TextEncoder().encode(text);
const text = (chunks: (Uint8Array | string)[]) =>
  chunks.map((c) => (typeof c === 'string' ? c : new TextDecoder().decode(c))).join('');

describe('reidratação de um painel que voltou a aparecer', () => {
  it('sem reidratação, a saída ao vivo passa direto', () => {
    const out: (Uint8Array | string)[] = [];
    const gate = new HydrationGate((d) => out.push(d));
    gate.push(bytes('a'));
    expect(text(out)).toBe('a');
  });

  it('saída que chega antes do histórico espera e sai depois dele', () => {
    const out: (Uint8Array | string)[] = [];
    const gate = new HydrationGate((d) => out.push(d));
    const g = gate.begin();
    gate.push(bytes('novo1 '));
    gate.push(bytes('novo2'));
    expect(out).toHaveLength(0);
    gate.finish(g, bytes('histórico '));
    expect(text(out)).toBe('histórico novo1 novo2');
    gate.push(bytes(' depois'));
    expect(text(out)).toBe('histórico novo1 novo2 depois');
  });

  it('histórico de uma reidratação já superada é descartado', () => {
    const out: (Uint8Array | string)[] = [];
    const gate = new HydrationGate((d) => out.push(d));
    const first = gate.begin();
    gate.cancel(); // sumiu
    const second = gate.begin(); // voltou
    gate.finish(first, bytes('velho'));
    expect(out).toHaveLength(0);
    gate.finish(second, bytes('atual'));
    expect(text(out)).toBe('atual');
  });

  it('falha ao buscar o histórico ainda libera a saída que esperava', () => {
    const out: (Uint8Array | string)[] = [];
    const gate = new HydrationGate((d) => out.push(d));
    const g = gate.begin();
    gate.push(bytes('ao vivo'));
    gate.finish(g, null);
    expect(text(out)).toBe('ao vivo');
  });
});
