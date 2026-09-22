import { cva, type VariantProps } from 'class-variance-authority';
import type { ReactNode } from 'react';
import { cn } from '@/lib/cn';

const badge = cva(
  'inline-flex items-center gap-1 rounded-sm px-1.5 py-0.5 text-caption font-medium',
  {
    variants: {
      variant: {
        neutral: 'bg-hover text-secondary',
        accent: 'bg-accent text-accent-fg',
        outline: 'border border-subtle text-secondary',
        danger: 'bg-failed text-accent-fg',
      },
    },
    defaultVariants: { variant: 'neutral' },
  },
);

interface BadgeProps extends VariantProps<typeof badge> {
  children: ReactNode;
  className?: string;
}

export function Badge({ variant, className, children }: BadgeProps) {
  return <span className={cn(badge({ variant }), className)}>{children}</span>;
}
