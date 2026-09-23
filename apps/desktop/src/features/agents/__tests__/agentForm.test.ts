import { describe, expect, it } from 'vitest';
import type { Agent } from '@/types/generated/Agent';
import {
  agentSchema,
  draftFromForm,
  EMPTY_FORM,
  formFromAgent,
  parseArgs,
  parseEnv,
} from '../agentForm';

const valid = { ...EMPTY_FORM, name: 'Backend', handle: 'backend' };

function errorsFor(values: typeof valid, siblings: string[] = []) {
  const result = agentSchema(siblings).safeParse(values);
  return result.success
    ? {}
    : Object.fromEntries(result.error.issues.map((i) => [i.path[0], i.message]));
}

describe('validação do formulário de agente (T5)', () => {
  it('aceita um agente comum', () => {
    expect(errorsFor(valid)).toEqual({});
  });

  it('impede handle duplicado na equipe antes de submeter', () => {
    expect(errorsFor(valid, ['backend', 'frontend']).handle).toMatch(/Já existe/);
  });

  it('recusa handles reservados e fora do padrão', () => {
    expect(errorsFor({ ...valid, handle: 'all' }).handle).toMatch(/reservado/);
    expect(errorsFor({ ...valid, handle: 'Backend' }).handle).toBeDefined();
    expect(errorsFor({ ...valid, handle: 'b' }).handle).toBeDefined();
  });

  it('exige o comando quando o runtime é customizado', () => {
    expect(errorsFor({ ...valid, adapterId: 'custom' }).command).toBeDefined();
    expect(errorsFor({ ...valid, adapterId: 'custom', command: 'htop' })).toEqual({});
  });

  it('não deixa sobrescrever variáveis do AISENSE', () => {
    expect(errorsFor({ ...valid, envText: 'AISENSE_TOKEN=x' }).envText).toMatch(/AISENSE/);
  });
});

describe('conversões', () => {
  it('lê variáveis linha a linha, ignorando vazias e comentários', () => {
    expect(parseEnv('A=1\n\n# nada\nB = dois=2\nC')).toEqual({
      env: { A: '1', B: ' dois=2', C: '' },
    });
    expect(parseEnv('1X=a').error).toMatch(/Linha 1/);
  });

  it('lê um argumento por linha', () => {
    expect(parseArgs('--a\n\n  --dir=a b  \n')).toEqual(['--a', '--dir=a b']);
  });

  it('no custom, o comando vira o primeiro argumento e volta para o campo ao editar', () => {
    const draft = draftFromForm({
      ...valid,
      adapterId: 'custom',
      command: 'htop',
      argsText: '-d\n10',
    });
    expect(draft.args).toEqual(['htop', '-d', '10']);
    expect(draft.model).toBeUndefined();

    const agent = {
      ...draft,
      id: 'agt_1',
      teamId: 'tem_1',
      model: null,
      workdir: null,
    } as unknown as Agent;
    const form = formFromAgent(agent);
    expect(form.command).toBe('htop');
    expect(form.argsText).toBe('-d\n10');
  });

  it('diretório vazio herda o da equipe', () => {
    expect(draftFromForm(valid).workdir).toBeUndefined();
  });
});
