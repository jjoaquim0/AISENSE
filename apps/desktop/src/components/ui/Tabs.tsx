import * as Radix from '@radix-ui/react-tabs';
import type { ReactNode } from 'react';
import { cn } from '@/lib/cn';

export const Tabs = Radix.Root;

export function TabsList({ children }: { children: ReactNode }) {
  return (
    <Radix.List className="flex shrink-0 gap-1 border-b border-subtle px-2">{children}</Radix.List>
  );
}

export function TabsTrigger({ value, children }: { value: string; children: ReactNode }) {
  return (
    <Radix.Trigger
      value={value}
      className={cn(
        'relative -mb-px border-b-2 border-transparent px-2 py-1.5 text-label text-secondary',
        'transition-colors duration-100 hover:text-primary',
        'data-[state=active]:border-emphasis data-[state=active]:text-primary',
      )}
    >
      {children}
    </Radix.Trigger>
  );
}

export function TabsContent({ value, children }: { value: string; children: ReactNode }) {
  return (
    <Radix.Content value={value} className="min-h-0 flex-1 overflow-auto p-3">
      {children}
    </Radix.Content>
  );
}
