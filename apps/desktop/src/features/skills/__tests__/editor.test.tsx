import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { TooltipProvider } from '@/components/ui';
import type { SkillCheck } from '@/types/generated/SkillCheck';
import type { SkillEntry } from '@/types/generated/SkillEntry';
import type { SkillUser } from '@/types/generated/SkillUser';

const open = vi.fn();
const check = vi.fn();
const save = vi.fn();
const users = vi.fn();
const restart = vi.fn();
vi.mock('@/features/skills/api', () => ({
  skillsApi: {
    open: (...a: unknown[]) => open(...a),
    check: (...a: unknown[]) => check(...a),
    save: (...a: unknown[]) => save(...a),
    users: (...a: unknown[]) => users(...a),
  },
  onSkillsChanged: () => Promise.resolve(() => {}),
}));
vi.mock('@/features/agents/api', () => ({
  agentsApi: { restart: (...a: unknown[]) => restart(...a) },
  onAgentState: () => Promise.resolve(() => {}),
}));

const { SkillEditor } = await import('../SkillEditor');
const model = await import('../editorModel');

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const entry = (name: string, over: Partial<SkillEntry> = {}): SkillEntry => ({
  id: name,
  name,
  description: `Faz ${name}.`,
  version: '1.0.0',
  targets: [],
  inject: 'bootstrap',
  priority: 50,
  source: { kind: 'user', dir: `/skills/${name}` },
  chars: 10,
  users: [],
  ...over,
});

const user = (handle: string, enabled: boolean, running: boolean): SkillUser => ({
  agentId: handle,
  handle,
  teamId: 't1',
  teamName: 'Squad',
  enabled,
  running,
});

describe('regras da biblioteca', () => {
  it('separa embutidas, suas e fora do disco, e busca sem acento', () => {
    const skills = [
      entry('revisor', { description: 'Revisão de código' }),
      entry('coordenador', { source: { kind: 'builtin' } }),
      entry('sumida', { source: null }),
    ];
    const all = model.librarySections(skills, '');
    expect(all.builtin.map((s) => s.name)).toEqual(['coordenador']);
    expect(all.user.map((s) => s.name)).toEqual(['revisor']);
    expect(all.missing.map((s) => s.name)).toEqual(['sumida']);
    expect(model.librarySections(skills, 'REVISAO').user.map((s) => s.name)).toEqual(['revisor']);
    expect(model.librarySections(skills, 'xyz').user).toEqual([]);
  });

  it('só quem está ligado e rodando precisa reiniciar', () => {
    const list = [user('ana', true, false), user('bia', true, true), user('caio', false, true)];
    expect(model.needsRestart(list).map((u) => u.handle)).toEqual(['bia']);
    expect(model.impactSummary(list)).toBe(
      'Usada por 3 agentes. 1 agente precisa reiniciar para aplicar: @bia (Squad).',
    );
    expect(model.impactSummary([user('ana', true, false)])).toBe(
      'Usada por 1 agente. Nenhum está rodando com ela: vale no próximo início.',
    );
    expect(model.impactSummary([])).toBe('Nenhum agente usa esta skill.');
  });

  it('contador muda de cor perto e acima do limite do BOOT.md', () => {
    expect(model.budgetLevel(100, 12_000)).toBe('ok');
    expect(model.budgetLevel(9_600, 12_000)).toBe('near');
    expect(model.budgetLevel(12_001, 12_000)).toBe('over');
  });

  it('corpo sem o frontmatter, com BOM e \\r\\n', () => {
    expect(model.bodyOf('﻿---\r\nname: a\r\n---\r\n# Oi\r\n')).toBe('# Oi\r\n');
    expect(model.bodyOf('sem frontmatter')).toBe('sem frontmatter');
  });

  it('o modelo de skill nova já começa válido no formato do docs/06', () => {
    expect(model.NEW_SKILL_TEMPLATE.startsWith('---\nname: minha-skill\n')).toBe(true);
  });
});

describe('<SkillEditor />', () => {
  let container: HTMLDivElement;
  let root: Root;
  const source = '---\nname: revisor\ndescription: Revisa.\n---\n# Revisor\n\nCorpo.\n';
  const valid = (body: string): SkillCheck => ({
    skill: {
      name: 'revisor',
      description: 'Revisa.',
      version: '1.0.0',
      targets: [],
      inject: 'bootstrap',
      priority: 50,
      env: {},
      body,
      source: { kind: 'user', dir: '/skills/revisor' },
    },
    problems: [],
    warnings: [],
    chars: body.length,
    bootLimit: 12_000,
  });

  beforeEach(() => {
    vi.useFakeTimers();
    open.mockResolvedValue({ source, dir: '/skills/revisor', builtin: false });
    check.mockImplementation((text: string) => Promise.resolve(valid(model.bodyOf(text))));
    users.mockResolvedValue([
      user('bia', true, true),
      user('ana', true, false),
      user('caio', false, true),
    ]);
    save.mockResolvedValue(valid('').skill);
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  const render = async () => {
    await act(async () => {
      root.render(
        <TooltipProvider>
          <SkillEditor
            target={{ kind: 'skill', name: 'revisor' }}
            onClose={() => {}}
            onOpen={() => {}}
          />
        </TooltipProvider>,
      );
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(250);
    });
  };

  const type = async (value: string) => {
    const textarea = container.querySelector('textarea') as HTMLTextAreaElement;
    await act(async () => {
      const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')?.set;
      setter?.call(textarea, value);
      textarea.dispatchEvent(new Event('input', { bubbles: true }));
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(250);
    });
  };

  it('editar uma skill em uso mostra a lista exata de quem precisa reiniciar', async () => {
    await render();
    expect(users).toHaveBeenCalledWith('revisor');
    await type(`${source}Mais uma linha.\n`);
    const footer = container.querySelector('footer')?.textContent ?? '';
    expect(footer).toContain('Usada por 3 agentes.');
    expect(footer).toContain('1 agente precisa reiniciar para aplicar: @bia (Squad).');
    expect(footer).not.toContain('@caio');
    expect(container.textContent).toContain('não salvo');
    // Preview do Markdown e o contador do orçamento.
    expect(container.querySelector('section[aria-label="Preview"] h1')?.textContent).toBe(
      'Revisor',
    );
    expect(footer).toContain('/ 12.000 caracteres');
  });

  it('salvar manda o texto e a pasta aberta; depois oferece reiniciar os afetados', async () => {
    restart.mockResolvedValue({});
    await render();
    const edited = `${source}Mais.\n`;
    await type(edited);
    const saveButton = [...container.querySelectorAll('button')].find((b) =>
      b.textContent?.includes('Salvar'),
    ) as HTMLButtonElement;
    expect(saveButton.disabled).toBe(false);
    await act(async () => saveButton.click());
    expect(save).toHaveBeenCalledWith(edited, '/skills/revisor');

    const restartButton = [...container.querySelectorAll('button')].find((b) =>
      b.textContent?.includes('Reiniciar agente'),
    ) as HTMLButtonElement;
    await act(async () => restartButton.click());
    expect(restart).toHaveBeenCalledTimes(1);
    expect(restart).toHaveBeenCalledWith('bia');
  });

  it('frontmatter inválido mostra caminho:linha e trava o salvar', async () => {
    await render();
    check.mockResolvedValue({
      skill: null,
      problems: [{ path: 'SKILL.md', line: 3, message: 'invalid YAML' }],
      warnings: [],
      chars: 10,
      bootLimit: 12_000,
    } satisfies SkillCheck);
    await type('---\nname: revisor\ndescription: [\n---\n');
    expect(container.querySelector('[aria-label="Validação"]')?.textContent).toContain(
      'SKILL.md:3: invalid YAML',
    );
    const saveButton = [...container.querySelectorAll('button')].find((b) =>
      b.textContent?.includes('Salvar'),
    ) as HTMLButtonElement;
    expect(saveButton.disabled).toBe(true);
  });
});
