import { open as openDialog } from '@tauri-apps/plugin-dialog';
import {
  AlertTriangle,
  Copy,
  Download,
  Lock,
  MoreHorizontal,
  Pencil,
  Plus,
  Search,
  Trash2,
  Upload,
} from 'lucide-react';
import { useState } from 'react';
import {
  Button,
  Dialog,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
  IconButton,
  Input,
} from '@/components/ui';
import { errorMessage } from '@/features/teams/api';
import type { SkillEntry } from '@/types/generated/SkillEntry';
import { skillsApi } from './api';
import {
  ALWAYS_ON_SKILL,
  agentCount,
  describeProblem,
  librarySections,
  NEW_SKILL_TEMPLATE,
} from './editorModel';
import { type EditorTarget, SkillEditor } from './SkillEditor';
import { useSkillLibrary } from './useSkillLibrary';

/**
 * T7 — Biblioteca de Skills (docs/09): embutidas e as suas, busca, importar, nova skill.
 * Acompanha o disco ao vivo (`skills:changed`); o editor abre por cima, em tela cheia.
 */
export function SkillLibraryScreen() {
  const { library, problem: loadProblem } = useSkillLibrary();
  const [query, setQuery] = useState('');
  const [editor, setEditor] = useState<EditorTarget | null>(null);
  const [message, setMessage] = useState<{ text: string; error: boolean } | null>(null);
  const [deleting, setDeleting] = useState<SkillEntry | null>(null);

  const report = (text: string, error = false) => setMessage({ text, error });
  const attempt = async (action: () => Promise<void>) => {
    setMessage(null);
    try {
      await action();
    } catch (e: unknown) {
      report(errorMessage(e), true);
    }
  };

  const importSkill = () =>
    attempt(async () => {
      const picked = await openDialog({ directory: true, title: 'Pasta da skill (com SKILL.md)' });
      if (typeof picked !== 'string') return;
      const skill = await skillsApi.importFrom(picked);
      report(`Skill ${skill.name} importada.`);
    });

  const exportSkill = (skill: SkillEntry) =>
    attempt(async () => {
      const picked = await openDialog({ directory: true, title: `Exportar ${skill.name} para…` });
      if (typeof picked !== 'string') return;
      const dir = await skillsApi.exportTo(skill.name, picked);
      report(`Exportada para ${dir}.`);
    });

  const duplicate = (skill: SkillEntry) =>
    attempt(async () => {
      setEditor({ kind: 'new', source: await skillsApi.duplicate(skill.name) });
    });

  const sections = library ? librarySections(library.skills, query) : null;
  const actions: CardActions = {
    open: (skill) => setEditor({ kind: 'skill', name: skill.name }),
    duplicate: (skill) => void duplicate(skill),
    exportTo: (skill) => void exportSkill(skill),
    remove: setDeleting,
  };

  return (
    <div className="mx-auto flex max-w-6xl flex-col gap-5 px-6 py-6">
      <header className="flex flex-wrap items-center gap-3">
        <h1 className="mr-auto text-display text-primary">Skills</h1>
        <div className="relative w-64">
          <Search
            size={14}
            className="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-muted"
          />
          <Input
            aria-label="Buscar skills"
            placeholder="Buscar"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            className="w-full pl-8"
          />
        </div>
        <Button onClick={() => void importSkill()}>
          <Upload size={14} /> Importar
        </Button>
        <Button
          variant="primary"
          onClick={() => setEditor({ kind: 'new', source: NEW_SKILL_TEMPLATE })}
        >
          <Plus size={15} /> Nova skill
        </Button>
      </header>

      {message && (
        <p
          role={message.error ? 'alert' : 'status'}
          className={message.error ? 'text-caption text-failed' : 'text-caption text-idle'}
        >
          {message.text}
        </p>
      )}
      {loadProblem && <p className="text-caption text-failed">{loadProblem}</p>}
      {!library && !loadProblem && <p className="text-caption text-muted">Carregando…</p>}

      {sections && (
        <>
          <Section
            title="Embutidas"
            empty="Nenhuma embutida ainda."
            skills={sections.builtin}
            actions={actions}
          />
          <Section
            title="Suas"
            empty={query ? 'Nada encontrado.' : 'Crie uma skill ou importe uma pasta com SKILL.md.'}
            skills={sections.user}
            actions={actions}
          />
          {sections.missing.length > 0 && (
            <Section
              title="Não estão mais no disco"
              empty=""
              skills={sections.missing}
              actions={actions}
            />
          )}
        </>
      )}

      {library && library.problems.length > 0 && (
        <section aria-labelledby="skill-problems" className="flex flex-col gap-1.5">
          <h2 id="skill-problems" className="text-caption tracking-[0.02em] text-muted uppercase">
            Não carregaram
          </h2>
          <ul className="flex flex-col gap-1">
            {library.problems.map((p) => (
              <li
                key={describeProblem(p)}
                className="flex items-start gap-2 rounded-md border border-subtle bg-surface px-3 py-2 text-caption"
              >
                <AlertTriangle size={13} className="mt-0.5 shrink-0 text-failed" />
                <span className="min-w-0 flex-1 font-mono break-all text-secondary">
                  {describeProblem(p)}
                </span>
                {!p.path.startsWith('builtin:') && (
                  <Button size="sm" onClick={() => setEditor({ kind: 'file', path: p.path })}>
                    Consertar
                  </Button>
                )}
              </li>
            ))}
          </ul>
        </section>
      )}

      {editor && (
        <SkillEditor
          key={JSON.stringify(editor)}
          target={editor}
          onClose={() => setEditor(null)}
          onOpen={setEditor}
        />
      )}
      <DeleteSkillDialog
        skill={deleting}
        onClose={() => setDeleting(null)}
        onDeleted={(name) => report(`Skill ${name} excluída.`)}
      />
    </div>
  );
}

interface CardActions {
  open: (skill: SkillEntry) => void;
  duplicate: (skill: SkillEntry) => void;
  exportTo: (skill: SkillEntry) => void;
  remove: (skill: SkillEntry) => void;
}

function Section({
  title,
  empty,
  skills,
  actions,
}: {
  title: string;
  empty: string;
  skills: SkillEntry[];
  actions: CardActions;
}) {
  const id = `skills-${title.replace(/\s+/g, '-').toLowerCase()}`;
  return (
    <section aria-labelledby={id} className="flex flex-col gap-2">
      <h2 id={id} className="text-caption tracking-[0.02em] text-muted uppercase">
        {title} {skills.length > 0 && <span>({skills.length})</span>}
      </h2>
      {skills.length === 0 ? (
        <p className="text-caption text-muted">{empty}</p>
      ) : (
        <ul className="grid grid-cols-[repeat(auto-fill,minmax(15rem,1fr))] gap-3">
          {skills.map((skill) => (
            <SkillCard key={skill.id} skill={skill} actions={actions} />
          ))}
        </ul>
      )}
    </section>
  );
}

function SkillCard({ skill, actions }: { skill: SkillEntry; actions: CardActions }) {
  const onDisk = skill.source !== null;
  const builtin = skill.source?.kind === 'builtin';
  const alwaysOn = builtin && skill.name === ALWAYS_ON_SKILL;
  return (
    <li className="flex min-h-32 flex-col gap-1.5 rounded-xl border border-subtle bg-surface p-3">
      <div className="flex items-start justify-between gap-2">
        <h3 className="flex min-w-0 items-center gap-1.5 font-mono text-label text-primary">
          {onDisk ? (
            <button
              type="button"
              onClick={() => actions.open(skill)}
              className="truncate text-left hover:underline"
            >
              {skill.name}
            </button>
          ) : (
            skill.name
          )}
          {builtin && <Lock size={12} aria-label="Embutida" className="shrink-0 text-muted" />}
        </h3>
        {onDisk && (
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <IconButton label={`Ações da skill ${skill.name}`} size="sm">
                <MoreHorizontal size={14} />
              </IconButton>
            </DropdownMenuTrigger>
            <DropdownMenuContent>
              <DropdownMenuItem onSelect={() => actions.open(skill)}>
                <span className="flex items-center gap-2">
                  <Pencil size={14} /> {builtin ? 'Ver' : 'Editar'}
                </span>
              </DropdownMenuItem>
              <DropdownMenuItem onSelect={() => actions.duplicate(skill)}>
                <span className="flex items-center gap-2">
                  <Copy size={14} /> Duplicar
                </span>
              </DropdownMenuItem>
              <DropdownMenuItem onSelect={() => actions.exportTo(skill)}>
                <span className="flex items-center gap-2">
                  <Download size={14} /> Exportar…
                </span>
              </DropdownMenuItem>
              {!builtin && (
                <>
                  <DropdownMenuSeparator />
                  <DropdownMenuItem danger onSelect={() => actions.remove(skill)}>
                    <span className="flex items-center gap-2">
                      <Trash2 size={14} /> Excluir…
                    </span>
                  </DropdownMenuItem>
                </>
              )}
            </DropdownMenuContent>
          </DropdownMenu>
        )}
      </div>
      <p className="line-clamp-2 text-caption text-secondary" title={skill.description}>
        {skill.description}
      </p>
      <p className="mt-auto text-caption text-muted">
        {alwaysOn
          ? 'sempre ativa'
          : onDisk
            ? `v${skill.version} · ${agentCount(skill.users.length)}`
            : `fora do disco · ${agentCount(skill.users.length)}`}
      </p>
    </li>
  );
}

function DeleteSkillDialog({
  skill,
  onClose,
  onDeleted,
}: {
  skill: SkillEntry | null;
  onClose: () => void;
  onDeleted: (name: string) => void;
}) {
  const [problem, setProblem] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const dir = skill?.source?.kind === 'user' ? skill.source.dir : null;
  const users = skill?.users.length ?? 0;

  const remove = async () => {
    if (!skill || !dir) return;
    setBusy(true);
    setProblem(null);
    try {
      await skillsApi.remove(dir);
      onDeleted(skill.name);
      onClose();
    } catch (e: unknown) {
      setProblem(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      open={skill !== null}
      onOpenChange={(open) => {
        if (!open) {
          setProblem(null);
          onClose();
        }
      }}
      title={`Excluir ${skill?.name ?? ''}?`}
      description="A pasta da skill sai do disco, com os arquivos de apoio."
      footer={
        <>
          <Button onClick={onClose}>Cancelar</Button>
          <Button variant="danger" onClick={() => void remove()} disabled={busy}>
            Excluir
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-2 text-body text-secondary">
        {dir && <p className="font-mono text-caption break-all text-muted">{dir}</p>}
        {users > 0 && (
          <p className="text-awaiting">
            {agentCount(users)} {users === 1 ? 'tem' : 'têm'} esta skill atribuída. A atribuição
            fica, marcada como fora do disco, e o agente é avisado no próximo início.
          </p>
        )}
        {problem && (
          <p role="alert" className="text-caption text-failed">
            {problem}
          </p>
        )}
      </div>
    </Dialog>
  );
}
