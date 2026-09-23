/**
 * Quem sabe focar o terminal de cada agente. `⌘1..9` (F03-08) foca o painel N e o
 * teclado precisa cair dentro do xterm — que só o próprio `<Terminal />` alcança.
 */
const terminals = new Map<string, () => void>();

export function registerTerminalFocus(agentId: string, focus: () => void): () => void {
  terminals.set(agentId, focus);
  return () => {
    if (terminals.get(agentId) === focus) terminals.delete(agentId);
  };
}

/**
 * Foca o terminal do agente; sem terminal (parado), o painel. Espera um quadro: o
 * painel pode estar montando agora (trocou de vista, voltou para a grade).
 */
export function focusAgentPane(agentId: string): void {
  requestAnimationFrame(() => {
    const terminal = terminals.get(agentId);
    if (terminal) return terminal();
    document.querySelector<HTMLElement>(`[data-agent-pane="${CSS.escape(agentId)}"]`)?.focus();
  });
}
