import type { ReactNode } from 'react';

interface EmptyStateProps {
  icon: ReactNode;
  title: string;
  description: string;
  action?: ReactNode;
  /** Nota secundária — versão, dica de atalho, contexto. */
  note?: string;
}

export function EmptyState({ icon, title, description, action, note }: EmptyStateProps) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 px-6 text-center">
      <div className="flex size-12 items-center justify-center rounded-xl border border-subtle bg-surface text-muted">
        {icon}
      </div>
      <h2 className="text-display text-primary">{title}</h2>
      <p className="max-w-sm text-body text-secondary">{description}</p>
      {action}
      {note && <p className="text-caption text-muted">{note}</p>}
    </div>
  );
}
