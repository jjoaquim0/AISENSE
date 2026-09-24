import { type ReactNode, useEffect, useId, useState } from 'react';
import { cn } from '@/lib/cn';

/** Bloco de uma seção: título, frase de contexto e os campos. */
export function Group({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: ReactNode;
}) {
  const id = useId();
  return (
    <section aria-labelledby={id} className="flex flex-col gap-3 border-b border-subtle pb-5">
      <header>
        <h3 id={id} className="text-heading text-primary">
          {title}
        </h3>
        {description && <p className="text-caption text-muted">{description}</p>}
      </header>
      {children}
    </section>
  );
}

/** Radios em linha (tema, densidade...). */
export function Radios<T extends string>({
  legend,
  value,
  options,
  onChange,
}: {
  legend: string;
  value: T;
  options: [T, string][];
  onChange: (value: T) => void;
}) {
  const name = useId();
  return (
    <fieldset className="flex flex-wrap items-center gap-x-4 gap-y-1">
      <legend className="mb-1 text-label text-secondary">{legend}</legend>
      {options.map(([id, label]) => (
        <label key={id} className="flex items-center gap-1.5 text-body text-primary">
          <input
            type="radio"
            name={name}
            value={id}
            checked={value === id}
            onChange={() => onChange(id)}
          />
          {label}
        </label>
      ))}
    </fieldset>
  );
}

export function Toggle({
  label,
  hint,
  checked,
  onChange,
  disabled,
}: {
  label: string;
  hint?: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}) {
  const id = useId();
  return (
    <div className={cn('flex items-start gap-2', disabled && 'opacity-50')}>
      <input
        id={id}
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange(e.target.checked)}
        aria-describedby={hint ? `${id}-hint` : undefined}
        className="mt-1"
      />
      <label htmlFor={id} className="flex flex-col">
        <span className="text-body text-primary">{label}</span>
        {hint && (
          <span id={`${id}-hint`} className="text-caption text-muted">
            {hint}
          </span>
        )}
      </label>
    </div>
  );
}

/**
 * Número que grava ao sair do campo (ou Enter), não a cada tecla: apagar "30" para
 * digitar "45" passaria por "", "4"... e cada um viraria uma gravação.
 */
export function NumberField({
  label,
  hint,
  value,
  min,
  max,
  suffix,
  onCommit,
}: {
  label: string;
  hint?: string;
  value: number;
  min: number;
  max: number;
  suffix?: string;
  onCommit: (value: number) => void;
}) {
  const id = useId();
  const [text, setText] = useState(String(value));
  useEffect(() => setText(String(value)), [value]);
  const parsed = Number(text);
  const invalid = text.trim() === '' || !Number.isInteger(parsed) || parsed < min || parsed > max;
  const commit = () => {
    if (invalid) {
      setText(String(value));
      return;
    }
    if (parsed !== value) onCommit(parsed);
  };
  return (
    <div className="flex flex-col gap-1">
      <label htmlFor={id} className="text-label text-secondary">
        {label}
      </label>
      <div className="flex items-center gap-2">
        <input
          id={id}
          inputMode="numeric"
          value={text}
          onChange={(e) => setText(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === 'Enter') commit();
          }}
          aria-invalid={invalid || undefined}
          aria-describedby={`${id}-hint`}
          className={cn(
            'h-8 w-24 rounded-md border bg-surface px-2.5 text-body text-primary',
            invalid ? 'border-failed' : 'border-strong focus:border-emphasis',
          )}
        />
        {suffix && <span className="text-caption text-muted">{suffix}</span>}
      </div>
      <p id={`${id}-hint`} className={cn('text-caption', invalid ? 'text-failed' : 'text-muted')}>
        {invalid ? `Use um número inteiro entre ${min} e ${max}.` : hint}
      </p>
    </div>
  );
}

/** Caminho em fonte mono, selecionável para copiar. */
export function PathLine({ label, path }: { label: string; path: string }) {
  return (
    <div className="flex flex-col gap-0.5">
      <span className="text-label text-secondary">{label}</span>
      <code className="font-mono text-caption break-all text-primary select-all">{path}</code>
    </div>
  );
}
