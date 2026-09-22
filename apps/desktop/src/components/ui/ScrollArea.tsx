import * as Radix from '@radix-ui/react-scroll-area';
import type { ReactNode } from 'react';
import { cn } from '@/lib/cn';

export function ScrollArea({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <Radix.Root className={cn('overflow-hidden', className)} type="hover">
      <Radix.Viewport className="size-full">{children}</Radix.Viewport>
      <Radix.Scrollbar orientation="vertical" className="flex w-2 touch-none p-0.5 select-none">
        <Radix.Thumb className="flex-1 rounded-full bg-strong opacity-60" />
      </Radix.Scrollbar>
    </Radix.Root>
  );
}
