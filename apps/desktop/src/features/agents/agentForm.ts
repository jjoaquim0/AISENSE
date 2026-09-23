import { z } from 'zod';
import type { Agent } from '@/types/generated/Agent';
import type { AgentColor } from '@/types/generated/AgentColor';
import type { AgentDraft } from '@/types/generated/AgentDraft';

/**
 * Regras do formulário T5 (docs/09). Espelham as do core (`Handle::parse`, `Agent::create`)
 * para o erro aparecer enquanto se digita — o core valida de novo ao gravar.
 */
export const HANDLE_PATTERN = /^[a-z][a-z0-9-]{1,31}$/;
export const RESERVED_HANDLES = ['all', 'voce'];
export const CUSTOM_ADAPTER = 'custom';

const COLORS = [
  'violet',
  'cyan',
  'emerald',
  'amber',
  'rose',
  'indigo',
  'teal',
  'fuchsia',
] as const satisfies readonly AgentColor[];

const ENV_KEY = /^[A-Za-z_][A-Za-z0-9_]*$/;

/** `CHAVE=valor` por linha; linhas vazias e comentários `#` são ignorados. */
export function parseEnv(text: string): { env: Record<string, string>; error?: string } {
  const env: Record<string, string> = {};
  for (const [index, raw] of text.split('\n').entries()) {
    const line = raw.trim();
    if (line === '' || line.startsWith('#')) continue;
    const eq = line.indexOf('=');
    const key = (eq === -1 ? line : line.slice(0, eq)).trim();
    if (!ENV_KEY.test(key)) return { env, error: `Linha ${index + 1}: nome de variável inválido` };
    if (key.toUpperCase().startsWith('AISENSE_')) {
      return { env, error: `Linha ${index + 1}: ${key} é controlada pelo AISENSE` };
    }
    env[key] = eq === -1 ? '' : line.slice(eq + 1);
  }
  return { env };
}

/** Um argumento por linha, preservando espaços internos. */
export function parseArgs(text: string): string[] {
  return text
    .split('\n')
    .map((l) => l.trim())
    .filter((l) => l !== '');
}

export function agentSchema(siblingHandles: string[]) {
  return z
    .object({
      name: z.string().trim().min(1, 'Dê um nome ao agente').max(64, 'No máximo 64 caracteres'),
      handle: z
        .string()
        .trim()
        .regex(HANDLE_PATTERN, 'Use 2 a 32 letras minúsculas, números ou "-", começando por letra')
        .refine((h) => !RESERVED_HANDLES.includes(h), 'Este endereço é reservado')
        .refine(
          (h) => !siblingHandles.includes(h),
          'Já existe um agente com este endereço na equipe',
        ),
      role: z.string().max(8000, 'No máximo 8 000 caracteres'),
      adapterId: z.string().min(1, 'Escolha um runtime'),
      command: z.string(),
      model: z.string(),
      workdir: z.string(),
      color: z.enum(COLORS),
      autostart: z.boolean(),
      restartPolicy: z.enum(['never', 'on-crash', 'always']),
      deliveryMode: z.enum(['pull', 'push', 'hook']),
      autonomy: z.enum(['ask', 'trusted']),
      envText: z.string().refine((t) => !parseEnv(t).error, {
        error: (issue) => parseEnv(String(issue.input)).error ?? 'Variáveis inválidas',
      }),
      argsText: z.string(),
    })
    .refine((v) => v.adapterId !== CUSTOM_ADAPTER || v.command.trim() !== '', {
      path: ['command'],
      message: 'Informe o comando que o agente vai rodar',
    });
}

export type AgentFormValues = z.infer<ReturnType<typeof agentSchema>>;

export const EMPTY_FORM: AgentFormValues = {
  name: '',
  handle: '',
  role: '',
  adapterId: 'claude',
  command: '',
  model: '',
  workdir: '',
  color: 'violet',
  autostart: true,
  restartPolicy: 'on-crash',
  deliveryMode: 'pull',
  autonomy: 'ask',
  envText: '',
  argsText: '',
};

export function formFromAgent(agent: Agent): AgentFormValues {
  const custom = agent.adapterId === CUSTOM_ADAPTER;
  const [command = '', ...rest] = agent.args;
  const args = custom ? rest : agent.args;
  return {
    name: agent.name,
    handle: agent.handle,
    role: agent.role,
    adapterId: agent.adapterId,
    command: custom ? command : '',
    model: agent.model ?? '',
    workdir: agent.workdir ?? '',
    color: agent.color,
    autostart: agent.autostart,
    restartPolicy: agent.restartPolicy,
    deliveryMode: agent.deliveryMode,
    autonomy: agent.autonomy,
    envText: Object.entries(agent.env)
      .map(([k, v]) => `${k}=${v ?? ''}`)
      .join('\n'),
    argsText: args.join('\n'),
  };
}

/** No `custom`, o comando vira o primeiro argumento (é assim que o supervisor o lê). */
export function draftFromForm(values: AgentFormValues, workbench: Agent['workbench']): AgentDraft {
  const args = parseArgs(values.argsText);
  const custom = values.adapterId === CUSTOM_ADAPTER;
  return {
    name: values.name.trim(),
    handle: values.handle.trim(),
    role: values.role.trim(),
    adapterId: values.adapterId,
    model: !custom && values.model.trim() ? values.model.trim() : undefined,
    workdir: values.workdir.trim() || undefined,
    env: parseEnv(values.envText).env,
    args: custom ? [values.command.trim(), ...args] : args,
    color: values.color,
    autostart: values.autostart,
    restartPolicy: values.restartPolicy,
    deliveryMode: values.deliveryMode,
    autonomy: values.autonomy,
    workbench,
  };
}
