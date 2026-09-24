import { Hash, Trash2 } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { Button, Dialog, IconButton, Input } from '@/components/ui';
import { busApi } from '@/features/bus/api';
import { errorMessage } from '@/features/teams/api';
import type { Agent } from '@/types/generated/Agent';
import type { ChannelInfo } from '@/types/generated/ChannelInfo';
import type { TeamId } from '@/types/generated/TeamId';

/**
 * Canais da equipe (F07-05): criar, mudar tópico e inscritos, apagar. Canal sem inscritos é
 * aberto — a mensagem vai para a equipe toda; com inscritos, só para eles.
 */
export function ChannelsDialog({
  teamId,
  agents,
  open,
  onOpenChange,
  onChanged,
}: {
  teamId: TeamId;
  agents: Agent[];
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onChanged: (channels: ChannelInfo[]) => void;
}) {
  const [channels, setChannels] = useState<ChannelInfo[]>([]);
  const [slug, setSlug] = useState('');
  const [topic, setTopic] = useState('');
  const [members, setMembers] = useState<string[]>([]);
  const [problem, setProblem] = useState<string | null>(null);
  const [deleting, setDeleting] = useState<string | null>(null);

  const reload = useCallback(
    () =>
      busApi
        .channels(teamId)
        .then((list) => {
          setChannels(list);
          onChanged(list);
        })
        .catch((e: unknown) => setProblem(errorMessage(e))),
    [teamId, onChanged],
  );

  useEffect(() => {
    if (open) void reload();
  }, [open, reload]);

  const edit = (info: ChannelInfo | null) => {
    setSlug(info?.channel.slug ?? '');
    setTopic(info?.channel.topic ?? '');
    setMembers(info?.members ?? []);
    setProblem(null);
  };

  const save = async () => {
    setProblem(null);
    try {
      await busApi.saveChannel(
        teamId,
        slug,
        topic,
        members.map((h) => `@${h}`),
      );
      edit(null);
      await reload();
    } catch (e: unknown) {
      setProblem(errorMessage(e));
    }
  };

  const remove = async (target: string) => {
    setDeleting(null);
    try {
      await busApi.deleteChannel(teamId, target);
      if (target === slug) edit(null);
      await reload();
    } catch (e: unknown) {
      setProblem(errorMessage(e));
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      size="lg"
      title="Canais"
      description="Canal sem inscritos vai para a equipe toda; com inscritos, só para eles. Agentes entram com aisense join #canal."
    >
      <div className="grid grid-cols-[1fr_1.2fr] gap-4">
        <ul className="flex flex-col gap-1" aria-label="Canais da equipe">
          {channels.length === 0 && (
            <li className="text-caption text-muted">Nenhum canal ainda.</li>
          )}
          {channels.map((info) => (
            <li key={info.channel.id} className="flex items-center gap-1">
              <button
                type="button"
                onClick={() => edit(info)}
                className="flex min-w-0 flex-1 flex-col rounded-md px-2 py-1 text-left hover:bg-hover"
              >
                <span className="flex items-center gap-1 text-body text-primary">
                  <Hash size={12} />
                  {info.channel.slug}
                </span>
                <span className="truncate text-caption text-muted">
                  {info.members.length === 0
                    ? 'aberto (equipe toda)'
                    : info.members.map((h) => `@${h}`).join(', ')}
                </span>
              </button>
              {deleting === info.channel.slug ? (
                <Button size="sm" variant="danger" onClick={() => void remove(info.channel.slug)}>
                  Apagar com as mensagens
                </Button>
              ) : (
                <IconButton
                  label={`Apagar #${info.channel.slug}`}
                  size="sm"
                  onClick={() => setDeleting(info.channel.slug)}
                >
                  <Trash2 size={12} />
                </IconButton>
              )}
            </li>
          ))}
        </ul>
        <form
          className="flex flex-col gap-2"
          aria-label="Editar canal"
          onSubmit={(e) => {
            e.preventDefault();
            void save();
          }}
        >
          <Input
            label="Nome"
            value={slug}
            placeholder="pesquisa"
            hint="Minúsculas, dígitos e hífen"
            onChange={(e) => setSlug(e.target.value.replace(/^#/, ''))}
          />
          <Input label="Tópico" value={topic} onChange={(e) => setTopic(e.target.value)} />
          <fieldset className="flex flex-col gap-1">
            <legend className="mb-1 text-label text-secondary">Inscritos</legend>
            {agents.map((a) => (
              <label key={a.id} className="flex items-center gap-2 text-body text-primary">
                <input
                  type="checkbox"
                  checked={members.includes(a.handle)}
                  onChange={(e) =>
                    setMembers((list) =>
                      e.target.checked ? [...list, a.handle] : list.filter((h) => h !== a.handle),
                    )
                  }
                />
                @{a.handle}
              </label>
            ))}
          </fieldset>
          {problem && (
            <p role="alert" className="text-caption text-failed">
              {problem}
            </p>
          )}
          <div className="flex justify-end gap-2">
            <Button variant="ghost" onClick={() => edit(null)}>
              Novo
            </Button>
            <Button type="submit" variant="primary" disabled={!slug.trim()}>
              Salvar canal
            </Button>
          </div>
        </form>
      </div>
    </Dialog>
  );
}
