import * as Radix from '@radix-ui/react-dropdown-menu';
import type { ReactNode } from 'react';
import { cn } from '@/lib/cn';

export const DropdownMenu = Radix.Root;
export const DropdownMenuTrigger = Radix.Trigger;

export function DropdownMenuContent({
  children,
  align = 'end',
}: {
  children: ReactNode;
  align?: 'start' | 'center' | 'end';
}) {
  return (
    <Radix.Portal>
      <Radix.Content
        align={align}
        sideOffset={4}
        className="z-50 min-w-44 rounded-lg border border-subtle bg-raised p-1 shadow-md"
      >
        {children}
      </Radix.Content>
    </Radix.Portal>
  );
}

export function DropdownMenuItem({
  children,
  onSelect,
  danger = false,
  shortcut,
}: {
  children: ReactNode;
  onSelect?: () => void;
  danger?: boolean;
  shortcut?: string;
}) {
  return (
    <Radix.Item
      onSelect={onSelect}
      className={cn(
        'flex cursor-default items-center justify-between gap-4 rounded-md px-2 py-1.5 text-body outline-none',
        'data-[highlighted]:bg-hover',
        danger ? 'text-failed' : 'text-primary',
      )}
    >
      {children}
      {shortcut && <span className="text-caption text-muted">{shortcut}</span>}
    </Radix.Item>
  );
}

export function DropdownMenuSeparator() {
  return <Radix.Separator className="my-1 h-px bg-subtle" />;
}
