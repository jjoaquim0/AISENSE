import { cn } from '@/lib/cn';

/*
 * Identidade visual do aisense (manual de marca v1.0).
 *
 * Símbolo "Elo": três nós ligados formam uma equipe e um "A" aberto. Os traços e os nós de
 * baixo seguem a cor do texto (`currentColor`); o nó do topo — a IA que coordena o time — é o
 * único ponto verde. Escrita: sempre em minúsculas, Bricolage Grotesque, e o ponto do "i"
 * repete o nó verde.
 *
 * Regras do manual que valem aqui: nada de distorcer, girar, trocar as cores ou pôr sombra;
 * símbolo sozinho com no mínimo 16px; logo completo com no mínimo 88px de largura.
 */

/** O símbolo sozinho. `mono` pinta o nó na cor do texto (sobre fundo verde, impressão). */
export function Elo({
  size = 20,
  mono = false,
  className,
  label,
}: {
  size?: number;
  mono?: boolean;
  className?: string;
  /** Sem rótulo, é decorativo (o texto ao lado já diz "aisense"). */
  label?: string;
}) {
  return (
    <svg
      viewBox="0 0 64 64"
      width={Math.max(16, size)}
      height={Math.max(16, size)}
      className={cn('shrink-0', className)}
      role={label ? 'img' : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
    >
      <path
        d="M32 15.5L13 49.5M32 15.5L51 49.5"
        fill="none"
        stroke="currentColor"
        strokeWidth={7}
        strokeLinecap="round"
      />
      <circle cx={13} cy={49.5} r={9} fill="currentColor" />
      <circle cx={51} cy={49.5} r={9} fill="currentColor" />
      <circle
        cx={32}
        cy={15.5}
        r={10}
        style={{ fill: mono ? 'currentColor' : 'var(--logo-node)' }}
      />
    </svg>
  );
}

/** A escrita "aisense", com o "ı" sem pingo e o ponto verde no lugar. */
export function Wordmark({ size = 18, className }: { size?: number; className?: string }) {
  return (
    <span
      aria-label="aisense"
      role="img"
      className={cn('inline-flex font-display leading-none', className)}
      style={{ fontSize: size, fontWeight: 650, letterSpacing: '-0.04em' }}
    >
      <span aria-hidden>a</span>
      <span aria-hidden className="relative inline-block">
        ı
        <span
          className="absolute left-1/2 -translate-x-1/2 rounded-full"
          style={{
            top: '0.035em',
            width: '0.2em',
            height: '0.2em',
            background: 'var(--logo-node)',
          }}
        />
      </span>
      <span aria-hidden>sense</span>
    </span>
  );
}

/** Logo completo, horizontal (a versão principal). */
export function Logo({ size = 18, className }: { size?: number; className?: string }) {
  return (
    <span className={cn('inline-flex items-center', className)} style={{ gap: size * 0.28 }}>
      <Elo size={Math.round(size * 1.05)} />
      <Wordmark size={size} />
    </span>
  );
}
