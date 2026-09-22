import { cva, type VariantProps } from 'class-variance-authority';
import type { ButtonHTMLAttributes, ReactNode } from 'react';
import { cn } from '@/lib/cn';

const button = cva(
  [
    'inline-flex items-center justify-center gap-1.5 rounded-md font-medium whitespace-nowrap',
    'transition-colors duration-100 ease-out',
    'disabled:pointer-events-none disabled:opacity-50',
  ],
  {
    variants: {
      variant: {
        primary: 'bg-accent text-accent-fg hover:opacity-90',
        secondary: 'border border-strong bg-surface text-primary hover:bg-hover',
        ghost: 'text-secondary hover:bg-hover hover:text-primary',
        danger: 'bg-failed text-accent-fg hover:opacity-90',
      },
      size: {
        sm: 'h-7 px-2.5 text-label',
        md: 'h-8 px-3 text-body',
      },
    },
    defaultVariants: { variant: 'secondary', size: 'md' },
  },
);

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement>, VariantProps<typeof button> {
  children: ReactNode;
}

export function Button({ variant, size, className, children, ...props }: ButtonProps) {
  return (
    <button type="button" className={cn(button({ variant, size }), className)} {...props}>
      {children}
    </button>
  );
}
