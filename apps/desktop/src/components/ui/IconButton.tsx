import { cva, type VariantProps } from 'class-variance-authority';
import type { ButtonHTMLAttributes, ReactNode } from 'react';
import { cn } from '@/lib/cn';

const iconButton = cva(
  [
    'inline-flex shrink-0 items-center justify-center rounded-md',
    'transition-colors duration-100 ease-out',
    'disabled:pointer-events-none disabled:opacity-50',
  ],
  {
    variants: {
      variant: {
        ghost: 'text-secondary hover:bg-hover hover:text-primary',
        secondary: 'border border-strong bg-surface text-primary hover:bg-hover',
        danger: 'text-secondary hover:bg-hover hover:text-failed',
      },
      size: { sm: 'size-6', md: 'size-7', lg: 'size-8' },
    },
    defaultVariants: { variant: 'ghost', size: 'md' },
  },
);

interface IconButtonProps
  extends ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof iconButton> {
  /** Obrigatório: botão só de ícone precisa de nome acessível. */
  label: string;
  children: ReactNode;
}

export function IconButton({
  label,
  variant,
  size,
  className,
  children,
  ...props
}: IconButtonProps) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      className={cn(iconButton({ variant, size }), className)}
      {...props}
    >
      {children}
    </button>
  );
}
