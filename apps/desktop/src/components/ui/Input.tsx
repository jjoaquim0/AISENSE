import type { InputHTMLAttributes } from 'react';
import { useId } from 'react';
import { cn } from '@/lib/cn';

interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  /** Mensagem de erro. Quando presente, o campo é marcado como inválido. */
  error?: string;
  hint?: string;
}

export function Input({ label, error, hint, className, id, ...props }: InputProps) {
  const generatedId = useId();
  const inputId = id ?? generatedId;
  const describedBy = error ? `${inputId}-error` : hint ? `${inputId}-hint` : undefined;

  return (
    <div className="flex flex-col gap-1">
      {label && (
        <label htmlFor={inputId} className="text-label text-secondary">
          {label}
        </label>
      )}
      <input
        id={inputId}
        aria-invalid={error ? true : undefined}
        aria-describedby={describedBy}
        className={cn(
          'h-8 rounded-md border bg-surface px-2.5 text-body text-primary',
          'placeholder:text-muted',
          'transition-colors duration-100 ease-out',
          'disabled:opacity-50',
          error ? 'border-failed' : 'border-strong focus:border-accent',
          className,
        )}
        {...props}
      />
      {error && (
        <p id={`${inputId}-error`} className="text-caption text-failed">
          {error}
        </p>
      )}
      {!error && hint && (
        <p id={`${inputId}-hint`} className="text-caption text-muted">
          {hint}
        </p>
      )}
    </div>
  );
}
