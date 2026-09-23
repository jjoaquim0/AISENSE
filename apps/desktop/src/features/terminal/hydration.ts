/**
 * Ordem da reidratação (F03-05). Quando um painel volta a aparecer, o xterm é
 * limpo e o histórico chega do core de forma assíncrona; eventos novos podem chegar
 * antes dele. Escrevê-los na hora colocaria a saída nova *acima* do histórico.
 * O portão segura esses eventos até o histórico ser escrito.
 */
export class HydrationGate {
  private queue: Uint8Array[] | null = null;
  private generation = 0;

  constructor(private readonly write: (data: Uint8Array | string) => void) {}

  /** Começa uma reidratação; devolve o número dela para `finish` conferir. */
  begin(): number {
    this.queue = [];
    this.generation += 1;
    return this.generation;
  }

  /** Um evento de saída: escreve agora ou espera o histórico. */
  push(chunk: Uint8Array): void {
    if (this.queue) this.queue.push(chunk);
    else this.write(chunk);
  }

  /**
   * O histórico chegou. Uma reidratação mais nova já começada (o painel sumiu e
   * voltou no meio) invalida esta: o histórico velho é descartado.
   */
  finish(generation: number, history: Uint8Array | null): void {
    if (generation !== this.generation || !this.queue) return;
    const queued = this.queue;
    this.queue = null;
    if (history && history.length > 0) this.write(history);
    for (const chunk of queued) this.write(chunk);
  }

  /** O painel sumiu no meio: nada do que está na fila vale mais. */
  cancel(): void {
    this.queue = null;
    this.generation += 1;
  }
}
