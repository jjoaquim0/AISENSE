import { AlertTriangle, ArrowLeft, NotebookPen, Plus, Save, Search, Trash2 } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { Badge, Button, Dialog, EmptyState, Input } from '@/components/ui';
import { MarkdownPreview } from '@/features/skills/MarkdownPreview';
import { errorMessage } from '@/features/teams/api';
import { cn } from '@/lib/cn';
import type { DiffLine } from '@/types/generated/DiffLine';
import type { Note } from '@/types/generated/Note';
import type { NoteMatch } from '@/types/generated/NoteMatch';
import type { NoteSummary } from '@/types/generated/NoteSummary';
import type { TeamId } from '@/types/generated/TeamId';
import { notesApi } from './api';
import { isValidSlug, slugify, updatedLabel } from './notesModel';

/** De quanto em quanto tempo o painel confere se um agente mudou a nota aberta. */
export const NOTES_POLL_MS = 3_000;

interface NotesPanelProps {
  teamId: TeamId;
  teamName: string;
  onClose: () => void;
}

/**
 * Notas da equipe (docs/15): a memória compartilhada em `<workdir>/.aisense/notes/`.
 * Lista, editor Markdown com preview (o mesmo das skills), busca e a trava otimista:
 * se um agente mudou a nota enquanto você editava, salvar mostra o diff antes de decidir.
 */
export function NotesPanel({ teamId, teamName, onClose }: NotesPanelProps) {
  const [notes, setNotes] = useState<NoteSummary[] | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [note, setNote] = useState<Note | null>(null);
  const [text, setText] = useState('');
  const [problem, setProblem] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [changedOnDisk, setChangedOnDisk] = useState(false);
  const [stale, setStale] = useState<{ currentHash: string; diff: DiffLine[] } | null>(null);
  const [creating, setCreating] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [query, setQuery] = useState('');
  const [matches, setMatches] = useState<NoteMatch[] | null>(null);
  const dirty = note !== null && text !== note.content;

  const reloadList = useCallback(() => {
    notesApi
      .list(teamId)
      .then(setNotes)
      .catch((e: unknown) => setProblem(errorMessage(e)));
  }, [teamId]);

  useEffect(reloadList, [reloadList]);

  const open = useCallback(
    (slug: string) => {
      setProblem(null);
      setNotice(null);
      setChangedOnDisk(false);
      notesApi
        .read(teamId, slug)
        .then((loaded) => {
          setSelected(slug);
          setNote(loaded);
          setText(loaded.content);
        })
        .catch((e: unknown) => setProblem(errorMessage(e)));
    },
    [teamId],
  );

  // Abre a mais recente na primeira carga.
  useEffect(() => {
    if (selected === null && notes && notes.length > 0 && notes[0]) open(notes[0].slug);
  }, [notes, selected, open]);

  // Agentes escrevem nas notas enquanto você lê: sem edição pendente, acompanha o disco;
  // com edição pendente, só avisa — quem decide é você, na hora de salvar.
  useEffect(() => {
    if (!selected || !note) return;
    const timer = setInterval(() => {
      reloadList();
      notesApi
        .read(teamId, selected)
        .then((current) => {
          if (current.hash === note.hash) return;
          if (text === note.content) {
            setNote(current);
            setText(current.content);
          } else {
            setChangedOnDisk(true);
          }
        })
        .catch(() => {});
    }, NOTES_POLL_MS);
    return () => clearInterval(timer);
  }, [teamId, selected, note, text, reloadList]);

  useEffect(() => {
    if (query.trim() === '') {
      setMatches(null);
      return;
    }
    const timer = setTimeout(() => {
      notesApi
        .search(teamId, query)
        .then(setMatches)
        .catch(() => setMatches([]));
    }, 200);
    return () => clearTimeout(timer);
  }, [teamId, query]);

  const save = async (expectHash: string | null) => {
    if (!selected || !note) return;
    setProblem(null);
    try {
      const result = await notesApi.save(teamId, selected, text, expectHash);
      if (result.kind === 'stale') {
        setStale({ currentHash: result.currentHash, diff: result.diff });
        return;
      }
      setNote(result.note);
      setStale(null);
      setChangedOnDisk(false);
      setNotice('Salvo.');
      reloadList();
    } catch (e: unknown) {
      setProblem(errorMessage(e));
    }
  };

  const discardMine = () => {
    setStale(null);
    if (selected) open(selected);
  };

  const remove = async () => {
    if (!selected) return;
    try {
      await notesApi.remove(teamId, selected);
      setDeleting(false);
      setSelected(null);
      setNote(null);
      setText('');
      reloadList();
    } catch (e: unknown) {
      setProblem(errorMessage(e));
    }
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={`Notas da equipe ${teamName}`}
      className="fixed inset-0 z-30 flex flex-col bg-base text-primary"
    >
      <header className="flex h-12 shrink-0 items-center gap-3 border-b border-subtle bg-surface px-3">
        <Button variant="ghost" size="sm" onClick={onClose}>
          <ArrowLeft size={14} /> {teamName}
        </Button>
        <h1 className="text-heading">Notas da equipe</h1>
        <div className="relative ml-auto w-64">
          <Search
            size={14}
            className="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-muted"
          />
          <Input
            aria-label="Buscar nas notas"
            placeholder="Buscar nas notas"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            className="w-full pl-8"
          />
        </div>
        <Button variant="primary" size="sm" onClick={() => setCreating(true)}>
          <Plus size={14} /> Nova nota
        </Button>
      </header>

      <div className="flex min-h-0 flex-1">
        <nav
          aria-label="Notas"
          className="flex w-64 shrink-0 flex-col gap-0.5 overflow-y-auto border-r border-subtle bg-surface p-2"
        >
          {matches !== null ? (
            <SearchResults matches={matches} onOpen={open} />
          ) : (
            <>
              {notes?.length === 0 && (
                <p className="px-2 py-1.5 text-caption text-muted">Nenhuma nota ainda.</p>
              )}
              {notes?.map((n) => (
                <button
                  key={n.slug}
                  type="button"
                  onClick={() => open(n.slug)}
                  aria-current={n.slug === selected ? 'page' : undefined}
                  className={cn(
                    'flex flex-col items-start rounded-md px-2 py-1.5 text-left',
                    n.slug === selected ? 'bg-hover' : 'hover:bg-hover',
                  )}
                >
                  <span className="w-full truncate text-body text-primary">{n.title}</span>
                  <span className="text-caption text-muted">
                    <span className="font-mono">{n.slug}</span> · {updatedLabel(n.updatedAt)}
                  </span>
                </button>
              ))}
            </>
          )}
        </nav>

        {note && selected ? (
          <section aria-label={`Nota ${selected}`} className="flex min-w-0 flex-1 flex-col">
            <div className="flex items-center gap-2 border-b border-subtle px-4 py-2">
              <span className="min-w-0 truncate font-mono text-label">{selected}.md</span>
              {dirty && <Badge variant="outline">não salvo</Badge>}
              {notice && !dirty && <span className="text-caption text-idle">{notice}</span>}
              <div className="ml-auto flex items-center gap-2">
                <Button size="sm" variant="ghost" onClick={() => setDeleting(true)}>
                  <Trash2 size={13} /> Excluir
                </Button>
                <Button
                  size="sm"
                  variant="primary"
                  disabled={!dirty}
                  onClick={() => void save(note.hash)}
                >
                  <Save size={13} /> Salvar
                </Button>
              </div>
            </div>
            {changedOnDisk && (
              <p
                role="status"
                className="flex items-center gap-1.5 border-b border-subtle px-4 py-1.5 text-caption text-awaiting"
              >
                <AlertTriangle size={12} />
                Um agente mudou esta nota enquanto você editava. Ao salvar, você vê o que mudou
                antes de decidir.
              </p>
            )}
            {problem && (
              <p
                role="alert"
                className="border-b border-subtle px-4 py-1.5 text-caption text-failed"
              >
                {problem}
              </p>
            )}
            <div className="grid min-h-0 flex-1 grid-cols-2">
              <label className="flex min-h-0 flex-col border-r border-subtle">
                <span className="sr-only">Conteúdo da nota</span>
                <textarea
                  value={text}
                  spellCheck={false}
                  onChange={(e) => {
                    setText(e.target.value);
                    setNotice(null);
                  }}
                  onKeyDown={(e) => {
                    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 's') {
                      e.preventDefault();
                      if (dirty) void save(note.hash);
                    }
                  }}
                  className="min-h-0 flex-1 resize-none bg-base p-4 font-mono text-body text-primary outline-none"
                />
              </label>
              <section aria-label="Preview" className="min-h-0 overflow-y-auto p-4">
                <MarkdownPreview markdown={text} />
              </section>
            </div>
          </section>
        ) : (
          <div className="flex flex-1 items-center justify-center">
            <EmptyState
              icon={<NotebookPen size={22} />}
              title="A memória da equipe"
              description="Decisões, contratos e convenções que todo agente lê. Os agentes veem o índice das notas no BOOT.md e leem a que precisarem."
              action={
                <Button variant="primary" onClick={() => setCreating(true)}>
                  <Plus size={14} /> Nova nota
                </Button>
              }
            />
            {problem && <p className="text-caption text-failed">{problem}</p>}
          </div>
        )}
      </div>

      <NewNoteDialog
        open={creating}
        onOpenChange={setCreating}
        onCreate={async (slug, title) => {
          await notesApi.create(teamId, slug, title);
          reloadList();
          open(slug);
        }}
      />
      <Dialog
        open={deleting}
        onOpenChange={setDeleting}
        title={`Excluir ${selected ?? ''}?`}
        description="O arquivo sai de .aisense/notes/. Se a pasta está no git, dá para recuperar pelo histórico."
        footer={
          <>
            <Button onClick={() => setDeleting(false)}>Cancelar</Button>
            <Button variant="danger" onClick={() => void remove()}>
              Excluir
            </Button>
          </>
        }
      />
      <Dialog
        open={stale !== null}
        onOpenChange={(o) => !o && setStale(null)}
        title="A nota mudou desde que você abriu"
        description="Um agente gravou nesta nota. Veja o que sairia do disco (−) e o que entraria da sua versão (+)."
        size="lg"
        footer={
          <>
            <Button onClick={() => setStale(null)}>Continuar editando</Button>
            <Button onClick={discardMine}>Usar a versão do disco</Button>
            <Button variant="danger" onClick={() => stale && void save(stale.currentHash)}>
              Gravar a minha por cima
            </Button>
          </>
        }
      >
        {stale && <DiffView diff={stale.diff} />}
      </Dialog>
    </div>
  );
}

export function DiffView({ diff }: { diff: DiffLine[] }) {
  return (
    <pre className="max-h-80 overflow-auto rounded-md border border-subtle bg-surface p-2 font-mono text-caption">
      {diff.map((line, i) => (
        <div
          // biome-ignore lint/suspicious/noArrayIndexKey: linhas do diff não têm id e não mudam de ordem
          key={i}
          className={cn(
            line.kind === 'removed' && 'text-failed',
            line.kind === 'added' && 'text-idle',
            line.kind === 'same' && 'text-muted',
          )}
        >
          {line.kind === 'removed' ? '− ' : line.kind === 'added' ? '+ ' : '  '}
          {line.text}
        </div>
      ))}
    </pre>
  );
}

function SearchResults({
  matches,
  onOpen,
}: {
  matches: NoteMatch[];
  onOpen: (slug: string) => void;
}) {
  if (matches.length === 0) return <p className="px-2 py-1.5 text-caption text-muted">Nada.</p>;
  return (
    <ul aria-label="Resultados da busca" className="flex flex-col gap-0.5">
      {matches.map((m) => (
        <li key={`${m.slug}:${m.line}`}>
          <button
            type="button"
            onClick={() => onOpen(m.slug)}
            className="flex w-full flex-col items-start rounded-md px-2 py-1.5 text-left hover:bg-hover"
          >
            <span className="font-mono text-caption text-muted">
              {m.slug}:{m.line}
            </span>
            <span className="w-full truncate text-caption text-secondary">{m.text}</span>
          </button>
        </li>
      ))}
    </ul>
  );
}

function NewNoteDialog({
  open,
  onOpenChange,
  onCreate,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onCreate: (slug: string, title: string) => Promise<void>;
}) {
  const [title, setTitle] = useState('');
  const [slug, setSlug] = useState('');
  const [slugTouched, setSlugTouched] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  const effectiveSlug = slugTouched ? slug : slugify(title);
  const valid = title.trim() !== '' && isValidSlug(effectiveSlug);

  const reset = () => {
    setTitle('');
    setSlug('');
    setSlugTouched(false);
    setProblem(null);
  };

  const create = async () => {
    try {
      await onCreate(effectiveSlug, title.trim());
      reset();
      onOpenChange(false);
    } catch (e: unknown) {
      setProblem(errorMessage(e));
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => {
        if (!o) reset();
        onOpenChange(o);
      }}
      title="Nova nota"
      description="Fica em .aisense/notes/ no diretório da equipe e entra no índice do BOOT.md."
      footer={
        <>
          <Button onClick={() => onOpenChange(false)}>Cancelar</Button>
          <Button variant="primary" disabled={!valid} onClick={() => void create()}>
            Criar
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-3">
        <Input
          label="Título"
          value={title}
          autoFocus
          placeholder="Decisões técnicas"
          onChange={(e) => setTitle(e.target.value)}
        />
        <Input
          label="Nome do arquivo"
          value={effectiveSlug}
          className="font-mono"
          onChange={(e) => {
            setSlugTouched(true);
            setSlug(e.target.value);
          }}
          error={
            effectiveSlug !== '' && !isValidSlug(effectiveSlug)
              ? 'Só letras minúsculas, números e hífen.'
              : undefined
          }
          hint={
            effectiveSlug ? `Os agentes leem com: aisense notes read ${effectiveSlug}` : undefined
          }
        />
        {problem && (
          <p role="alert" className="text-caption text-failed">
            {problem}
          </p>
        )}
      </div>
    </Dialog>
  );
}
