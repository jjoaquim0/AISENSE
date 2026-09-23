import { AlertTriangle, ArrowLeft, Copy, Lock, RotateCw, Save } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { Badge, Button, Dialog } from '@/components/ui';
import { agentsApi, onAgentState } from '@/features/agents/api';
import { errorMessage } from '@/features/teams/api';
import { cn } from '@/lib/cn';
import type { OpenedSkill } from '@/types/generated/OpenedSkill';
import type { SkillCheck } from '@/types/generated/SkillCheck';
import type { SkillUser } from '@/types/generated/SkillUser';
import { skillsApi } from './api';
import {
  bodyOf,
  budgetLevel,
  describeProblem,
  formatCount,
  impactSummary,
  needsRestart,
} from './editorModel';
import { MarkdownPreview } from './MarkdownPreview';

/** O que abrir no editor: rascunho novo, skill da biblioteca ou arquivo que não carregou. */
export type EditorTarget =
  | { kind: 'new'; source: string }
  | { kind: 'skill'; name: string }
  | { kind: 'file'; path: string };

const CHECK_DELAY_MS = 200;

interface SkillEditorProps {
  target: EditorTarget;
  onClose: () => void;
  /** Troca o que está aberto (ex.: "Duplicar para editar" numa embutida). */
  onOpen: (target: EditorTarget) => void;
}

/**
 * Editor de skill em tela cheia (docs/09, T7): Markdown à esquerda, preview à direita,
 * frontmatter validado no topo e, embaixo, quantos agentes usam e quais precisam
 * reiniciar. Skill não se aplica a quente (docs/06): salvar só vale no próximo início.
 */
export function SkillEditor({ target, onClose, onOpen }: SkillEditorProps) {
  const [opened, setOpened] = useState<OpenedSkill | null>(null);
  const [text, setText] = useState('');
  const [baseline, setBaseline] = useState('');
  /** Nome com que a skill está salva — é por ele que os agentes a encontram. */
  const [savedName, setSavedName] = useState<string | null>(
    target.kind === 'skill' ? target.name : null,
  );
  const [check, setCheck] = useState<SkillCheck | null>(null);
  const [users, setUsers] = useState<SkillUser[]>([]);
  const [problem, setProblem] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [confirmLeave, setConfirmLeave] = useState(false);

  useEffect(() => {
    const load =
      target.kind === 'new'
        ? Promise.resolve<OpenedSkill>({ source: target.source, dir: null, builtin: false })
        : target.kind === 'skill'
          ? skillsApi.open(target.name)
          : skillsApi.openFile(target.path);
    load
      .then((skill) => {
        setOpened(skill);
        setText(skill.source);
        // Um rascunho novo ainda não existe no disco: tudo nele é alteração.
        setBaseline(target.kind === 'new' ? '' : skill.source);
      })
      .catch((e: unknown) => setProblem(errorMessage(e)));
  }, [target]);

  const editing = opened?.dir ?? null;
  const readOnly = opened?.builtin ?? false;
  const dirty = opened !== null && text !== baseline;

  useEffect(() => {
    if (!opened) return;
    const timer = setTimeout(() => {
      skillsApi
        .check(text, editing)
        .then(setCheck)
        .catch((e: unknown) => setProblem(errorMessage(e)));
    }, CHECK_DELAY_MS);
    return () => clearTimeout(timer);
  }, [opened, text, editing]);

  const loadUsers = useCallback(() => {
    if (!savedName) return;
    skillsApi
      .users(savedName)
      .then(setUsers)
      .catch(() => setUsers([]));
  }, [savedName]);

  // Quem precisa reiniciar muda quando um agente sobe ou para.
  useEffect(() => {
    loadUsers();
    const unlisten = onAgentState(() => loadUsers());
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [loadUsers]);

  const canSave = !readOnly && dirty && check?.skill != null && check.problems.length === 0;

  const save = async () => {
    if (!canSave) return;
    setBusy(true);
    setProblem(null);
    try {
      const skill = await skillsApi.save(text, editing);
      const dir = skill.source.kind === 'user' ? skill.source.dir : null;
      setOpened({ source: text, dir, builtin: false });
      setBaseline(text);
      setSavedName(skill.name);
      loadUsers();
      setNotice('Salvo. Vale no próximo início de cada agente.');
    } catch (e: unknown) {
      setProblem(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const duplicate = async () => {
    if (!savedName) return;
    try {
      onOpen({ kind: 'new', source: await skillsApi.duplicate(savedName) });
    } catch (e: unknown) {
      setProblem(errorMessage(e));
    }
  };

  const restartAll = async () => {
    setBusy(true);
    setProblem(null);
    const failed: string[] = [];
    for (const user of needsRestart(users)) {
      try {
        await agentsApi.restart(user.agentId);
      } catch (e: unknown) {
        failed.push(`@${user.handle}: ${errorMessage(e)}`);
      }
    }
    setBusy(false);
    if (failed.length > 0) setProblem(failed.join('\n'));
    else setNotice('Agentes reiniciados com a skill atualizada.');
    loadUsers();
  };

  const leave = () => (dirty ? setConfirmLeave(true) : onClose());
  const skill = check?.skill ?? null;
  const title = skill?.name ?? savedName ?? 'Nova skill';
  const level = check ? budgetLevel(check.chars, check.bootLimit) : 'ok';
  const restart = needsRestart(users);

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={`Editor da skill ${title}`}
      className="fixed inset-0 z-30 flex flex-col bg-base text-primary"
    >
      <header className="flex h-12 shrink-0 items-center gap-3 border-b border-subtle bg-surface px-3">
        <Button variant="ghost" size="sm" onClick={leave}>
          <ArrowLeft size={14} /> Biblioteca
        </Button>
        <h1 className="min-w-0 truncate font-mono text-heading">{title}</h1>
        {skill && <Badge variant="outline">v{skill.version}</Badge>}
        {readOnly && (
          <Badge>
            <Lock size={11} /> embutida · só leitura
          </Badge>
        )}
        {dirty && !readOnly && <Badge variant="outline">não salvo</Badge>}
        <div className="ml-auto flex items-center gap-2">
          {savedName && (
            <Button size="sm" onClick={() => void duplicate()}>
              <Copy size={13} /> {readOnly ? 'Duplicar para editar' : 'Duplicar'}
            </Button>
          )}
          {!readOnly && (
            <Button
              variant="primary"
              size="sm"
              onClick={() => void save()}
              disabled={!canSave || busy}
            >
              <Save size={13} /> Salvar
            </Button>
          )}
        </div>
      </header>

      <Validation check={check} />
      {problem && (
        <p
          role="alert"
          className="border-b border-subtle px-4 py-1.5 text-caption whitespace-pre-line text-failed"
        >
          {problem}
        </p>
      )}

      <div className="grid min-h-0 flex-1 grid-cols-2">
        <label className="flex min-h-0 flex-col border-r border-subtle">
          <span className="sr-only">Conteúdo do SKILL.md</span>
          <textarea
            value={text}
            readOnly={readOnly}
            spellCheck={false}
            onChange={(e) => {
              setText(e.target.value);
              setNotice(null);
            }}
            onKeyDown={(e) => {
              if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 's') {
                e.preventDefault();
                void save();
              }
            }}
            className="min-h-0 flex-1 resize-none bg-base p-4 font-mono text-body text-primary outline-none"
          />
        </label>
        <section aria-label="Preview" className="min-h-0 overflow-y-auto p-4">
          {skill && <Frontmatter skill={skill} />}
          <MarkdownPreview markdown={skill?.body ?? bodyOf(text)} />
        </section>
      </div>

      <footer className="flex min-h-10 shrink-0 flex-wrap items-center gap-x-4 gap-y-1 border-t border-subtle bg-surface px-4 py-1.5 text-caption">
        {check && (
          <span
            className={cn(
              'font-mono',
              level === 'ok' && 'text-muted',
              level === 'near' && 'text-awaiting',
              level === 'over' && 'text-failed',
            )}
            title="Caracteres do corpo. O BOOT.md inteiro do agente tem esse limite."
          >
            {formatCount(check.chars)} / {formatCount(check.bootLimit)} caracteres
          </span>
        )}
        <span
          className={cn('min-w-0 flex-1', restart.length > 0 ? 'text-awaiting' : 'text-secondary')}
        >
          {savedName ? impactSummary(users) : 'Skill nova: nenhum agente usa ainda.'}
        </span>
        {notice && <span className="text-idle">{notice}</span>}
        {restart.length > 0 && !dirty && (
          <Button size="sm" onClick={() => void restartAll()} disabled={busy}>
            <RotateCw size={13} /> Reiniciar {restart.length === 1 ? 'agente' : 'agentes'}
          </Button>
        )}
      </footer>

      <Dialog
        open={confirmLeave}
        onOpenChange={setConfirmLeave}
        title="Descartar alterações?"
        description="O que você mudou nesta skill ainda não foi salvo."
        footer={
          <>
            <Button onClick={() => setConfirmLeave(false)}>Continuar editando</Button>
            <Button variant="danger" onClick={onClose}>
              Descartar
            </Button>
          </>
        }
      />
    </div>
  );
}

function Validation({ check }: { check: SkillCheck | null }) {
  if (!check || (check.problems.length === 0 && check.warnings.length === 0)) return null;
  return (
    <ul aria-label="Validação" className="border-b border-subtle px-4 py-1.5 text-caption">
      {check.problems.map((p) => (
        <li key={describeProblem(p)} className="flex items-start gap-1.5 text-failed">
          <AlertTriangle size={12} className="mt-0.5 shrink-0" />
          <span className="font-mono">{describeProblem(p)}</span>
        </li>
      ))}
      {check.warnings.map((w) => (
        <li key={w} className="flex items-start gap-1.5 text-awaiting">
          <AlertTriangle size={12} className="mt-0.5 shrink-0" />
          {w}
        </li>
      ))}
    </ul>
  );
}

/** "Realce de frontmatter" (docs/06): os campos como o app os entendeu. */
function Frontmatter({ skill }: { skill: NonNullable<SkillCheck['skill']> }) {
  const rows: [string, string][] = [
    ['descrição', skill.description],
    ['runtimes', skill.targets.length > 0 ? skill.targets.join(', ') : 'todos'],
    ['injeção', skill.inject],
    ['prioridade', String(skill.priority)],
  ];
  const env = Object.keys(skill.env);
  if (env.length > 0) rows.push(['env', env.join(', ')]);
  return (
    <dl className="mb-3 grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5 rounded-md border border-subtle bg-surface p-2.5 text-caption">
      {rows.map(([key, value]) => (
        <div key={key} className="contents">
          <dt className="text-muted">{key}</dt>
          <dd className="text-secondary">{value}</dd>
        </div>
      ))}
    </dl>
  );
}
