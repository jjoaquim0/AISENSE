import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { TooltipProvider } from '@/components/ui';
import type { Agent } from '@/types/generated/Agent';
import type { SessionSummary } from '@/types/generated/SessionSummary';

const transcript = vi.fn();
const exportTranscript = vi.fn();
const save = vi.fn();
vi.mock('@/features/agents/api', () => ({
  agentsApi: {
    transcript: (...args: unknown[]) => transcript(...args),
    exportTranscript: (...args: unknown[]) => exportTranscript(...args),
  },
}));
vi.mock('@tauri-apps/plugin-dialog', () => ({ save: (...args: unknown[]) => save(...args) }));

const { LogsTab, sessionLabel } = await import('../components/inspector/LogsTab');
const { exportFileName, findMatches, formatDuration, splitByMatches } = await import(
  '../transcriptSearch'
);

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
// jsdom não rola nada; a busca pede `scrollIntoView` na ocorrência atual.
Element.prototype.scrollIntoView = () => {};

const AGENT = { id: 'agt_1', handle: 'backend' } as Agent;
const session = (id: string, over: Partial<SessionSummary> = {}): SessionSummary => ({
  id,
  pid: 42,
  startedAt: Date.UTC(2026, 8, 23, 15, 30),
  endedAt: null,
  exitCode: null,
  available: true,
  ...over,
});

describe('aba Logs', () => {
  let host: HTMLDivElement;
  let root: Root;
  beforeEach(() => {
    host = document.createElement('div');
    document.body.append(host);
    root = createRoot(host);
    transcript.mockImplementation((_agent: string, sessionId: string) =>
      Promise.resolve({
        sessionId,
        text: `cargo test\nerro: falhou\nok\nErro de novo em ${sessionId}\n`,
        truncated: false,
        bytes: 60,
      }),
    );
  });
  afterEach(() => {
    act(() => root.unmount());
    host.remove();
    vi.clearAllMocks();
  });

  const render = async (sessions: SessionSummary[]) => {
    await act(async () =>
      root.render(
        <TooltipProvider>
          <LogsTab agent={AGENT} sessions={sessions} onReload={() => {}} />
        </TooltipProvider>,
      ),
    );
  };
  const type = async (value: string) => {
    const input = host.querySelector<HTMLInputElement>('input[type=search]');
    if (!input) throw new Error('sem busca');
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
    await act(async () => {
      setter?.call(input, value);
      input.dispatchEvent(new Event('input', { bubbles: true }));
    });
  };

  it('mostra a transcrição da sessão mais recente', async () => {
    await render([session('new'), session('old', { endedAt: 1, exitCode: 0 })]);
    expect(transcript).toHaveBeenCalledWith('agt_1', 'new');
    expect(host.querySelector('pre')?.textContent).toContain('Erro de novo em new');
  });

  it('busca sem diferenciar maiúsculas, marca as ocorrências e anda entre elas', async () => {
    await render([session('new')]);
    await type('erro');
    const marks = [...host.querySelectorAll('mark')];
    expect(marks.map((m) => m.textContent)).toEqual(['erro', 'Erro']);
    expect(host.textContent).toContain('1/2');
    await act(async () =>
      host.querySelector<HTMLButtonElement>('[aria-label="Próxima ocorrência"]')?.click(),
    );
    expect(host.textContent).toContain('2/2');
    await type('inexistente');
    expect(host.querySelectorAll('mark')).toHaveLength(0);
    expect(host.textContent).toContain('nada');
  });

  it('exporta a sessão escolhida para o arquivo do diálogo de salvar', async () => {
    save.mockResolvedValue('/home/eu/backend.txt');
    exportTranscript.mockResolvedValue(4096);
    await render([session('new')]);
    const button = [...host.querySelectorAll('button')].find(
      (b) => b.textContent?.trim() === 'Exportar',
    );
    await act(async () => button?.click());
    expect(save).toHaveBeenCalledWith(
      expect.objectContaining({ defaultPath: expect.stringMatching(/^backend-2026-09-23-/) }),
    );
    expect(exportTranscript).toHaveBeenCalledWith('agt_1', 'new', '/home/eu/backend.txt');
    expect(host.textContent).toContain('Transcrição exportada (4 KB).');
  });

  it('cancelar o diálogo não exporta nada', async () => {
    save.mockResolvedValue(null);
    await render([session('new')]);
    const button = [...host.querySelectorAll('button')].find(
      (b) => b.textContent?.trim() === 'Exportar',
    );
    await act(async () => button?.click());
    expect(exportTranscript).not.toHaveBeenCalled();
  });

  it('sessão sem transcrição avisa em vez de pedir ao core', async () => {
    await render([session('old', { available: false, endedAt: 1, exitCode: 1 })]);
    expect(transcript).not.toHaveBeenCalled();
    expect(host.textContent).toContain('não está mais no log');
  });
});

describe('regras da aba Logs', () => {
  it('findMatches e splitByMatches remontam o texto original', () => {
    const text = 'Ação ação AÇÃO';
    const matches = findMatches(text, 'ação');
    expect(matches).toEqual([
      [0, 4],
      [5, 9],
      [10, 14],
    ]);
    expect(
      splitByMatches(text, matches)
        .map((p) => p.text)
        .join(''),
    ).toBe(text);
    expect(findMatches(text, '   ')).toEqual([]);
  });

  it('rótulos de sessão e duração', () => {
    expect(sessionLabel(session('a'), 0)).toContain('em andamento');
    expect(sessionLabel(session('a', { endedAt: 2, exitCode: 0 }), 1)).toContain('saiu bem');
    expect(sessionLabel(session('a', { endedAt: 2, exitCode: 3 }), 1)).toContain('código 3');
    expect(formatDuration(42_000)).toBe('42 s');
    expect(formatDuration(3 * 60_000)).toBe('3 min');
    expect(formatDuration(65 * 60_000)).toBe('1 h 05 min');
    expect(exportFileName('api', new Date(2026, 0, 5, 9, 7).getTime())).toBe(
      'api-2026-01-05-0907.txt',
    );
  });
});
