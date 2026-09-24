import { save as saveDialog } from '@tauri-apps/plugin-dialog';
import { useEffect, useState } from 'react';
import {
  Button,
  Dialog,
  ErrorNotice,
  SkeletonList,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from '@/components/ui';
import { errorMessage } from '@/features/teams/api';
import type { DiagnosticBundle } from '@/types/generated/DiagnosticBundle';
import { settingsApi } from './api';

/**
 * "Exportar diagnóstico" (F09-05): mostra cada arquivo do pacote, já redigido, antes de
 * salvar. O `.zip` gravado é exatamente o que está na tela.
 */
export function DiagnosticsDialog({
  open,
  onOpenChange,
  onSaved,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSaved: (message: string) => void;
}) {
  const [bundle, setBundle] = useState<DiagnosticBundle | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [selected, setSelected] = useState(0);
  const [attempt, setAttempt] = useState(0);

  // biome-ignore lint/correctness/useExhaustiveDependencies: `attempt` refaz a prévia no "Tentar de novo".
  useEffect(() => {
    if (!open) return;
    let alive = true;
    setBundle(null);
    setProblem(null);
    setSelected(0);
    settingsApi
      .diagnosticsPreview()
      .then((b) => alive && setBundle(b))
      .catch((e: unknown) => alive && setProblem(errorMessage(e)));
    return () => {
      alive = false;
    };
  }, [open, attempt]);

  const saveZip = async () => {
    try {
      const path = await saveDialog({
        title: 'Salvar diagnóstico',
        defaultPath: await settingsApi.diagnosticsFileName(),
        filters: [{ name: 'Zip', extensions: ['zip'] }],
      });
      if (typeof path !== 'string') return;
      const bytes = await settingsApi.diagnosticsSave(path);
      onOpenChange(false);
      onSaved(
        `Diagnóstico salvo em ${path} (${Math.max(1, Math.round(bytes / 1024))} KB). Anexe-o ao relato do problema.`,
      );
    } catch (e: unknown) {
      setProblem(errorMessage(e));
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      size="lg"
      title="Exportar diagnóstico"
      description="Confira o que vai no pacote. Chaves, tokens e a sua pasta pessoal já estão mascarados."
      footer={
        <>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            Cancelar
          </Button>
          <Button variant="primary" disabled={!bundle} onClick={() => void saveZip()}>
            Salvar .zip…
          </Button>
        </>
      }
    >
      {problem && (
        <ErrorNotice
          title="Não foi possível montar o diagnóstico"
          message={problem}
          onRetry={() => setAttempt((n) => n + 1)}
        />
      )}
      {!bundle && !problem && <SkeletonList rows={3} label="Montando o diagnóstico" />}
      {bundle && (
        <div className="flex flex-col gap-3">
          <Tabs value={String(selected)} onValueChange={(v) => setSelected(Number(v))}>
            <TabsList>
              {bundle.files.map((f, i) => (
                <TabsTrigger key={f.name} value={String(i)}>
                  <span className="font-mono">{f.name}</span>
                </TabsTrigger>
              ))}
            </TabsList>
            {bundle.files.map((f, i) => (
              <TabsContent key={f.name} value={String(i)}>
                {f.truncated && (
                  <p className="mb-1 text-caption text-muted">
                    Só o fim do arquivo entra no pacote (o último 1 MB).
                  </p>
                )}
                <section aria-label={`Conteúdo de ${f.name}`}>
                  <pre
                    // biome-ignore lint/a11y/noNoninteractiveTabindex: rolar o conteúdo pelo teclado.
                    tabIndex={0}
                    className="max-h-80 overflow-auto rounded-md border border-subtle bg-surface p-2 font-mono text-caption break-all whitespace-pre-wrap text-secondary"
                  >
                    {f.content || '(vazio)'}
                  </pre>
                </section>
              </TabsContent>
            ))}
          </Tabs>
          {bundle.leftOut.length > 0 && (
            <div className="text-caption text-secondary">
              <p className="text-muted">Fica de fora:</p>
              <ul className="list-disc pl-5">
                {bundle.leftOut.map((item) => (
                  <li key={item}>{item}</li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}
    </Dialog>
  );
}
