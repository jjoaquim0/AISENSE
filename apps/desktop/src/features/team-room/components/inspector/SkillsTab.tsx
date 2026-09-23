import { AlertTriangle, ChevronDown, ChevronUp, Plus, X } from 'lucide-react';
import { useEffect, useState } from 'react';
import { IconButton } from '@/components/ui';
import { skillsApi } from '@/features/skills/api';
import {
  addSkill,
  available,
  moveSkill,
  removeSkill,
  supports,
  toggleSkill,
} from '@/features/skills/assign';
import { useSkillLibrary } from '@/features/skills/useSkillLibrary';
import { errorMessage } from '@/features/teams/api';
import type { Agent } from '@/types/generated/Agent';
import type { AgentSkill } from '@/types/generated/AgentSkill';
import type { SkillEntry } from '@/types/generated/SkillEntry';

interface SkillsTabProps {
  agent: Agent;
  running: boolean;
}

/**
 * Aba Skills do inspetor (docs/09, T6): as skills do agente em ordem de injeção, com
 * liga/desliga, e o que dá para adicionar da biblioteca. A biblioteca acompanha o disco
 * ao vivo (F04-02); skill não se aplica a quente, então mudar pede reinício (docs/06).
 */
export function SkillsTab({ agent, running }: SkillsTabProps) {
  const { library, problem: libraryProblem } = useSkillLibrary();
  const [assigned, setAssigned] = useState<AgentSkill[] | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [changed, setChanged] = useState(false);

  useEffect(() => {
    setAssigned(null);
    setChanged(false);
    skillsApi
      .ofAgent(agent.id)
      .then(setAssigned)
      .catch((e: unknown) => setProblem(errorMessage(e)));
  }, [agent.id]);

  const save = (next: AgentSkill[]) => {
    const before = assigned;
    setAssigned(next);
    setProblem(null);
    skillsApi
      .setForAgent(agent.id, next)
      .then(() => setChanged(true))
      .catch((e: unknown) => {
        setAssigned(before);
        setProblem(errorMessage(e));
      });
  };

  const byId = new Map(library?.skills.map((s) => [s.id, s]));
  const list = assigned ?? [];
  const toAdd = library ? available(library.skills, list) : [];

  return (
    <div className="flex flex-col gap-4">
      {running && changed && (
        <p className="flex items-start gap-1.5 rounded-md border border-subtle px-2.5 py-2 text-caption text-secondary">
          <AlertTriangle size={13} className="mt-0.5 shrink-0 text-awaiting" />
          Skills só mudam quando o agente reinicia.
        </p>
      )}
      {(problem || libraryProblem) && (
        <p className="text-caption text-failed" role="alert">
          {problem ?? libraryProblem}
        </p>
      )}

      <section aria-label="Skills do agente">
        <h4 className="pb-1 text-caption tracking-[0.02em] text-muted uppercase">
          Deste agente, em ordem
        </h4>
        {assigned && list.length === 0 && (
          <p className="text-caption text-muted">Nenhuma skill atribuída.</p>
        )}
        <ol className="flex flex-col gap-1">
          {list.map((entry, index) => (
            <AssignedRow
              key={entry.skillId}
              entry={entry}
              skill={byId.get(entry.skillId)}
              adapterId={agent.adapterId}
              first={index === 0}
              last={index === list.length - 1}
              onToggle={() => save(toggleSkill(list, entry.skillId))}
              onMove={(delta) => save(moveSkill(list, entry.skillId, delta))}
              onRemove={() => save(removeSkill(list, entry.skillId))}
            />
          ))}
        </ol>
      </section>

      <section aria-label="Adicionar skill">
        <h4 className="pb-1 text-caption tracking-[0.02em] text-muted uppercase">Biblioteca</h4>
        {library && toAdd.length === 0 && (
          <p className="text-caption text-muted">
            Nada para adicionar. Crie skills em{' '}
            <span className="font-mono">~/.aisense/skills/</span>.
          </p>
        )}
        <ul className="flex flex-col gap-1">
          {toAdd.map((skill) => (
            <li key={skill.id} className="flex items-start gap-1.5">
              <IconButton
                label={`Adicionar ${skill.name}`}
                size="sm"
                onClick={() => save(addSkill(list, skill.id))}
              >
                <Plus size={12} />
              </IconButton>
              <SkillText skill={skill} adapterId={agent.adapterId} />
            </li>
          ))}
        </ul>
      </section>

      {library && library.problems.length > 0 && (
        <section aria-label="Skills que não carregaram">
          <h4 className="pb-1 text-caption tracking-[0.02em] text-muted uppercase">
            Não carregaram
          </h4>
          <ul className="flex flex-col gap-1">
            {library.problems.map((p) => (
              <li key={`${p.path}:${p.line}`} className="text-caption text-failed">
                <span className="font-mono break-all">
                  {p.path}
                  {p.line !== null && `:${p.line}`}
                </span>
                {' — '}
                {p.message}
              </li>
            ))}
          </ul>
        </section>
      )}
    </div>
  );
}

function AssignedRow({
  entry,
  skill,
  adapterId,
  first,
  last,
  onToggle,
  onMove,
  onRemove,
}: {
  entry: AgentSkill;
  skill?: SkillEntry;
  adapterId: string;
  first: boolean;
  last: boolean;
  onToggle: () => void;
  onMove: (delta: -1 | 1) => void;
  onRemove: () => void;
}) {
  const name = skill?.name ?? '(skill desconhecida)';
  return (
    <li className="flex items-start gap-1.5">
      <input
        type="checkbox"
        checked={entry.enabled}
        onChange={onToggle}
        aria-label={`Usar ${name}`}
        className="mt-1"
      />
      {skill ? (
        <SkillText skill={skill} adapterId={adapterId} />
      ) : (
        <span className="flex-1 text-caption text-muted">{name}</span>
      )}
      <IconButton label={`Subir ${name}`} size="sm" disabled={first} onClick={() => onMove(-1)}>
        <ChevronUp size={12} />
      </IconButton>
      <IconButton label={`Descer ${name}`} size="sm" disabled={last} onClick={() => onMove(1)}>
        <ChevronDown size={12} />
      </IconButton>
      <IconButton label={`Remover ${name}`} size="sm" variant="danger" onClick={onRemove}>
        <X size={12} />
      </IconButton>
    </li>
  );
}

function SkillText({ skill, adapterId }: { skill: SkillEntry; adapterId: string }) {
  const incompatible = !supports(skill, adapterId);
  return (
    <span className="flex min-w-0 flex-1 flex-col">
      <span className="truncate text-body text-primary">
        {skill.name} <span className="text-caption text-muted">v{skill.version}</span>
      </span>
      <span className="line-clamp-2 text-caption text-secondary">{skill.description}</span>
      {skill.source === null && (
        <span className="text-caption text-failed">Não está mais no disco.</span>
      )}
      {incompatible && (
        <span className="text-caption text-awaiting">
          Não roda em {adapterId} (só {skill.targets.join(', ')}).
        </span>
      )}
    </span>
  );
}
