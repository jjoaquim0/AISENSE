import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { TooltipProvider } from '@/components/ui';
import type { Agent } from '@/types/generated/Agent';
import type { SkillEntry } from '@/types/generated/SkillEntry';
import type { SkillLibraryView } from '@/types/generated/SkillLibraryView';

const library = vi.fn();
const ofAgent = vi.fn();
const setForAgent = vi.fn();
const plan = vi.fn();
let changed: () => void = () => {};
vi.mock('@/features/skills/api', () => ({
  skillsApi: {
    library: () => library(),
    ofAgent: (...a: unknown[]) => ofAgent(...a),
    setForAgent: (...a: unknown[]) => setForAgent(...a),
    plan: (...a: unknown[]) => plan(...a),
  },
  onSkillsChanged: (handler: () => void) => {
    changed = handler;
    return Promise.resolve(() => {});
  },
}));

const { SkillsTab } = await import('@/features/team-room/components/inspector/SkillsTab');
const { addSkill, available, moveSkill, removeSkill, supports, toggleSkill } = await import(
  '../assign'
);

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const entry = (id: string, over: Partial<SkillEntry> = {}): SkillEntry => ({
  id,
  name: id,
  description: `Faz ${id}.`,
  version: '1.0.0',
  targets: [],
  inject: 'bootstrap',
  priority: 50,
  source: { kind: 'builtin' },
  chars: 10,
  users: [],
  ...over,
});
const view = (skills: SkillEntry[]): SkillLibraryView => ({ skills, problems: [] });

describe('regras de atribuição', () => {
  const a = { skillId: 'a', enabled: true };
  const b = { skillId: 'b', enabled: true };
  it('adiciona no fim sem repetir, remove, liga/desliga e move', () => {
    expect(addSkill([a], 'b')).toEqual([a, b]);
    expect(addSkill([a], 'a')).toEqual([a]);
    expect(removeSkill([a, b], 'a')).toEqual([b]);
    expect(toggleSkill([a], 'a')).toEqual([{ skillId: 'a', enabled: false }]);
    expect(moveSkill([a, b], 'b', -1)).toEqual([b, a]);
    expect(moveSkill([a, b], 'a', -1)).toEqual([a, b]);
  });
  it('compatibilidade e o que ainda dá para adicionar', () => {
    expect(supports(entry('x'), 'codex')).toBe(true);
    expect(supports(entry('x', { targets: ['claude'] }), 'codex')).toBe(false);
    const gone = entry('gone', { source: null });
    expect(available([entry('a'), entry('b'), gone], [a]).map((s) => s.id)).toEqual(['b']);
    // `trabalho-em-equipe` vai em todo agente pelo BOOT.md: não se atribui.
    expect(available([entry('trabalho-em-equipe')], [])).toEqual([]);
  });
});

describe('aba Skills', () => {
  let host: HTMLDivElement;
  let root: Root;
  const agent = { id: 'agt_1', adapterId: 'codex' } as Agent;
  beforeEach(() => {
    host = document.createElement('div');
    document.body.append(host);
    root = createRoot(host);
    ofAgent.mockResolvedValue([{ skillId: 'revisor', enabled: true }]);
    setForAgent.mockResolvedValue(undefined);
    plan.mockResolvedValue({ active: [], ignored: [] });
  });
  afterEach(() => {
    act(() => root.unmount());
    host.remove();
    vi.clearAllMocks();
  });
  const render = async () => {
    await act(async () =>
      root.render(
        <TooltipProvider>
          <SkillsTab agent={agent} running />
        </TooltipProvider>,
      ),
    );
  };

  it('editar o SKILL.md no disco atualiza a lista sem reiniciar', async () => {
    library.mockResolvedValueOnce(view([entry('revisor', { description: 'Versão antiga.' })]));
    await render();
    expect(host.textContent).toContain('Versão antiga.');

    library.mockResolvedValueOnce(
      view([entry('revisor', { description: 'Versão nova.' }), entry('docs')]),
    );
    await act(async () => changed());
    expect(host.textContent).toContain('Versão nova.');
    expect(host.querySelector('[aria-label="Adicionar docs"]')).not.toBeNull();
  });

  it('avisa a skill que não roda no runtime do agente', async () => {
    library.mockResolvedValue(view([entry('revisor', { targets: ['claude'] })]));
    await render();
    expect(host.textContent).toContain('Não roda em codex (só claude).');
  });

  it('mostra o que entra no próximo início e o que fica de fora, com o porquê', async () => {
    library.mockResolvedValue(view([entry('revisor', { targets: ['claude'] }), entry('docs')]));
    plan.mockResolvedValue({
      active: ['docs'],
      ignored: [
        {
          name: 'revisor',
          reason: { kind: 'incompatible', adapterId: 'codex', targets: ['claude'] },
          message: 'skill revisor ignorada: não roda em codex (só claude)',
        },
      ],
    });
    await render();
    const next = host.querySelector('[aria-label="No próximo início"]');
    expect(next?.textContent).toContain('docs');
    expect(next?.textContent).toContain('skill revisor ignorada: não roda em codex (só claude)');
    expect(plan).toHaveBeenCalledWith('agt_1');
  });

  it('adicionar grava a lista inteira e pede reinício com o agente rodando', async () => {
    library.mockResolvedValue(view([entry('revisor'), entry('docs')]));
    await render();
    await act(async () =>
      host.querySelector<HTMLButtonElement>('[aria-label="Adicionar docs"]')?.click(),
    );
    expect(setForAgent).toHaveBeenCalledWith('agt_1', [
      { skillId: 'revisor', enabled: true },
      { skillId: 'docs', enabled: true },
    ]);
    expect(host.textContent).toContain('só mudam quando o agente reinicia');
  });

  it('se o core recusar, volta ao que estava', async () => {
    library.mockResolvedValue(view([entry('revisor')]));
    setForAgent.mockRejectedValue({ code: 'skill_not_found', message: 'skill sumiu' });
    await render();
    await act(async () =>
      host.querySelector<HTMLButtonElement>('[aria-label="Remover revisor"]')?.click(),
    );
    expect(host.querySelector('[aria-label="Remover revisor"]')).not.toBeNull();
    expect(host.querySelector('[role=alert]')).not.toBeNull();
  });
});
