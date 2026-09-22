import { MoreVertical, Terminal, Users } from 'lucide-react';
import { useState } from 'react';
import {
  type AgentState,
  Badge,
  Button,
  Dialog,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
  EmptyState,
  IconButton,
  Input,
  Kbd,
  StatusDot,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
  Tooltip,
} from '@/components/ui';

/**
 * Amostra do design system (F00-03 e F00-06). Só existe em desenvolvimento:
 * abra com `#/dev`. Serve para conferir escala tipográfica, paleta e componentes
 * lado a lado nos dois temas.
 */
export function KitchenSink() {
  const [dialogOpen, setDialogOpen] = useState(false);

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-8 p-8">
      <header>
        <h1 className="text-display">Design system</h1>
        <p className="text-body text-secondary">
          Alterne o tema com <Kbd>⌘⇧D</Kbd> e confira tudo nos dois.
        </p>
      </header>

      <Section title="Tipografia">
        <p className="text-display">Display 28 — título de tela vazia</p>
        <p className="text-title">Title 20 — nome da equipe</p>
        <p className="text-heading">Heading 15 — título de seção</p>
        <p className="text-body">Body 13,5 — texto padrão da interface</p>
        <p className="text-label text-secondary">Label 12 — rótulo de campo</p>
        <p className="text-caption text-muted">Caption 11 — metadados e contadores</p>
        <p className="font-mono text-body">Mono 13 — 0O 1lI {'{}'} =&gt; !== &amp;&amp;</p>
      </Section>

      <Section title="Cores semânticas">
        <div className="flex flex-wrap gap-2">
          {SURFACES.map(({ token, label }) => (
            <div
              key={token}
              className="flex h-16 w-32 flex-col justify-end rounded-lg border border-subtle p-2"
              style={{ background: `var(${token})` }}
            >
              <span className="text-caption text-primary">{label}</span>
            </div>
          ))}
        </div>
      </Section>

      <Section title="Cores de agente">
        <div className="flex flex-wrap gap-2">
          {AGENT_COLORS.map((token) => (
            <div
              key={token}
              className="flex items-center gap-2 rounded-md border border-subtle px-2 py-1.5"
            >
              <span className="size-3 rounded-full" style={{ background: `var(${token})` }} />
              <span className="text-caption text-secondary">{token.replace('--agent-', '')}</span>
            </div>
          ))}
        </div>
      </Section>

      <Section title="Estados do agente">
        <div className="flex flex-wrap gap-4">
          {AGENT_STATES.map((state) => (
            <StatusDot key={state} state={state} withLabel />
          ))}
        </div>
      </Section>

      <Section title="Botões">
        <div className="flex flex-wrap items-center gap-2">
          <Button variant="primary">Primário</Button>
          <Button variant="secondary">Secundário</Button>
          <Button variant="ghost">Fantasma</Button>
          <Button variant="danger">Perigo</Button>
          <Button variant="primary" size="sm">
            Pequeno
          </Button>
          <Button disabled>Desabilitado</Button>
          <Tooltip content="Ações do agente">
            <IconButton label="Ações do agente">
              <MoreVertical size={15} />
            </IconButton>
          </Tooltip>
        </div>
      </Section>

      <Section title="Campos">
        <div className="grid max-w-md gap-3">
          <Input label="Nome do agente" placeholder="Backend" />
          <Input
            label="Endereço"
            defaultValue="@backend"
            hint="Usado no barramento, sem espaços."
          />
          <Input
            label="Endereço"
            defaultValue="@back end"
            error="Use apenas letras minúsculas, números e hífen."
          />
        </div>
      </Section>

      <Section title="Etiquetas e atalhos">
        <div className="flex flex-wrap items-center gap-2">
          <Badge>neutro</Badge>
          <Badge variant="accent">accent</Badge>
          <Badge variant="outline">contorno</Badge>
          <Badge variant="danger">erro</Badge>
          <Kbd>⌘K</Kbd>
          <Kbd>⌘⇧D</Kbd>
          <Kbd>Esc</Kbd>
        </div>
      </Section>

      <Section title="Sobreposições">
        <div className="flex flex-wrap items-center gap-2">
          <Button onClick={() => setDialogOpen(true)}>Abrir diálogo</Button>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="secondary">Menu</Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent>
              <DropdownMenuItem shortcut="⌘R">Reiniciar</DropdownMenuItem>
              <DropdownMenuItem>Limpar terminal</DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem danger>Parar agente</DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
        <Dialog
          open={dialogOpen}
          onOpenChange={setDialogOpen}
          title="Parar a equipe?"
          description="Os 4 agentes serão encerrados. O quadro e as mensagens são preservados."
          footer={
            <>
              <Button onClick={() => setDialogOpen(false)}>Cancelar</Button>
              <Button variant="danger" onClick={() => setDialogOpen(false)}>
                Parar tudo
              </Button>
            </>
          }
        />
      </Section>

      <Section title="Abas">
        <Tabs defaultValue="visao">
          <TabsList>
            <TabsTrigger value="visao">Visão</TabsTrigger>
            <TabsTrigger value="skills">Skills</TabsTrigger>
            <TabsTrigger value="caixa">Caixa</TabsTrigger>
          </TabsList>
          <TabsContent value="visao">
            <p className="text-body text-secondary">Estado, tempo ativo e últimos eventos.</p>
          </TabsContent>
          <TabsContent value="skills">
            <p className="text-body text-secondary">Skills ativas neste agente.</p>
          </TabsContent>
          <TabsContent value="caixa">
            <p className="text-body text-secondary">Mensagens pendentes.</p>
          </TabsContent>
        </Tabs>
      </Section>

      <Section title="Estado vazio">
        <div className="h-64 rounded-lg border border-subtle">
          <EmptyState
            icon={<Users size={22} />}
            title="Nenhum agente ainda"
            description="Agentes são terminais reais que conversam entre si e compartilham o quadro."
            action={
              <Button variant="primary">
                <Terminal size={14} /> Criar agente
              </Button>
            }
            note="⌘T"
          />
        </div>
      </Section>
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="flex flex-col gap-3">
      <h2 className="text-caption tracking-[0.02em] text-muted uppercase">{title}</h2>
      {children}
    </section>
  );
}

const SURFACES = [
  { token: '--bg-base', label: 'base' },
  { token: '--bg-surface', label: 'surface' },
  { token: '--bg-raised', label: 'raised' },
  { token: '--bg-hover', label: 'hover' },
  { token: '--bg-active', label: 'active' },
  { token: '--bg-terminal', label: 'terminal' },
];

const AGENT_COLORS = [
  '--agent-violet',
  '--agent-cyan',
  '--agent-emerald',
  '--agent-amber',
  '--agent-rose',
  '--agent-indigo',
  '--agent-teal',
  '--agent-fuchsia',
];

const AGENT_STATES: AgentState[] = [
  'idle',
  'busy',
  'awaiting_input',
  'failed',
  'stopped',
  'starting',
];
