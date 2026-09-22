import * as RadixDialog from '@radix-ui/react-dialog';
import { X } from 'lucide-react';
import type { ReactNode } from 'react';
import { IconButton } from './IconButton';

interface DialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  /** Descrição acessível. Obrigatória: leitores de tela anunciam junto com o título. */
  description: string;
  children?: ReactNode;
  footer?: ReactNode;
}

export function Dialog({ open, onOpenChange, title, description, children, footer }: DialogProps) {
  return (
    <RadixDialog.Root open={open} onOpenChange={onOpenChange}>
      <RadixDialog.Portal>
        <RadixDialog.Overlay className="fixed inset-0 z-40 bg-black/40" />
        <RadixDialog.Content className="fixed top-1/2 left-1/2 z-50 w-[min(28rem,calc(100vw-2rem))] -translate-x-1/2 -translate-y-1/2 rounded-xl border border-subtle bg-raised p-4 shadow-lg">
          <div className="mb-3 flex items-start justify-between gap-4">
            <div>
              <RadixDialog.Title className="text-heading text-primary">{title}</RadixDialog.Title>
              <RadixDialog.Description className="mt-0.5 text-body text-secondary">
                {description}
              </RadixDialog.Description>
            </div>
            <RadixDialog.Close asChild>
              <IconButton label="Fechar" size="sm">
                <X size={14} />
              </IconButton>
            </RadixDialog.Close>
          </div>
          {children}
          {footer && <div className="mt-4 flex justify-end gap-2">{footer}</div>}
        </RadixDialog.Content>
      </RadixDialog.Portal>
    </RadixDialog.Root>
  );
}
