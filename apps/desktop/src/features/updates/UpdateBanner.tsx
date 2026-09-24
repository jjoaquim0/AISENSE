import { Download } from 'lucide-react';
import { useEffect, useState } from 'react';
import { Button, Dialog } from '@/components/ui';
import { useSettings } from '@/features/settings/store';
import { isDesktop } from '@/lib/api';
import { useUpdates } from './store';

/** Espera o app assentar antes de ir à rede: a subida não disputa com a verificação. */
const CHECK_DELAY_MS = 4000;

/** Procura atualização uma vez ao abrir, se a pessoa não desligou (F09-03). */
export function useUpdateCheckOnStart() {
  const enabled = useSettings((s) => s.view?.settings.advanced.checkUpdates ?? false);
  const check = useUpdates((s) => s.check);
  useEffect(() => {
    if (!enabled || !isDesktop()) return;
    const timer = window.setTimeout(() => void check(), CHECK_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [enabled, check]);
}

/** Faixa abaixo da barra de título quando há versão nova. */
export function UpdateBanner() {
  const phase = useUpdates((s) => s.phase);
  const dismissed = useUpdates((s) => s.dismissed);
  const install = useUpdates((s) => s.install);
  const dismiss = useUpdates((s) => s.dismiss);
  const [notesOpen, setNotesOpen] = useState(false);

  const update =
    phase.kind === 'available' || phase.kind === 'installing'
      ? phase.update
      : phase.kind === 'error'
        ? phase.update
        : null;
  if (!update || (dismissed && phase.kind === 'available')) return null;

  return (
    <div
      role="status"
      className="flex shrink-0 flex-wrap items-center gap-x-3 gap-y-1 border-b border-subtle bg-surface px-4 py-1.5 text-body"
    >
      <Download size={14} aria-hidden className="shrink-0 text-accent" />
      <span className="mr-auto text-primary">
        {phase.kind === 'installing'
          ? `Instalando o AISENSE ${update.version}${phase.percent === null ? '…' : ` — ${phase.percent}%`}. O app reinicia sozinho ao terminar.`
          : phase.kind === 'error'
            ? `A atualização para ${update.version} falhou: ${phase.message}`
            : `AISENSE ${update.version} disponível (você tem a ${update.currentVersion}).`}
      </span>
      {phase.kind !== 'installing' && (
        <>
          {update.notes && (
            <Button size="sm" variant="ghost" onClick={() => setNotesOpen(true)}>
              Novidades
            </Button>
          )}
          <Button size="sm" variant="primary" onClick={() => void install()}>
            {phase.kind === 'error' ? 'Tentar de novo' : 'Instalar e reiniciar'}
          </Button>
          {phase.kind === 'available' && (
            <Button size="sm" variant="ghost" onClick={dismiss}>
              Depois
            </Button>
          )}
        </>
      )}
      <Dialog
        open={notesOpen}
        onOpenChange={setNotesOpen}
        size="lg"
        title={`Novidades do AISENSE ${update.version}`}
        description="Os agentes rodando são parados antes de reiniciar; com “religar os agentes” ligado, eles voltam sozinhos."
      >
        <pre className="max-h-96 overflow-auto font-sans text-body break-words whitespace-pre-wrap text-secondary">
          {update.notes}
        </pre>
      </Dialog>
    </div>
  );
}
