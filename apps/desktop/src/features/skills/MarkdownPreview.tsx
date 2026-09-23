import Markdown, { type Components } from 'react-markdown';
import remarkGfm from 'remark-gfm';

/**
 * Preview do corpo de uma skill. `react-markdown` não interpreta HTML cru: um `<script>`
 * no `SKILL.md` aparece como texto, nunca executa. Links não navegam — a janela do app
 * não é navegador.
 */
const components: Components = {
  h1: ({ children }) => (
    <h1 className="mt-4 mb-2 text-display text-primary first:mt-0">{children}</h1>
  ),
  h2: ({ children }) => <h2 className="mt-4 mb-1.5 text-heading text-primary">{children}</h2>,
  h3: ({ children }) => <h3 className="mt-3 mb-1 text-label text-primary">{children}</h3>,
  p: ({ children }) => <p className="my-2 text-body text-primary">{children}</p>,
  ul: ({ children }) => <ul className="my-2 list-disc space-y-0.5 pl-5 text-body">{children}</ul>,
  ol: ({ children }) => (
    <ol className="my-2 list-decimal space-y-0.5 pl-5 text-body">{children}</ol>
  ),
  a: ({ children }) => <span className="text-accent underline">{children}</span>,
  blockquote: ({ children }) => (
    <blockquote className="my-2 border-l-2 border-strong pl-3 text-secondary">
      {children}
    </blockquote>
  ),
  code: ({ children, className }) =>
    className ? (
      <code className={className}>{children}</code>
    ) : (
      <code className="rounded-sm bg-hover px-1 font-mono text-[0.92em]">{children}</code>
    ),
  pre: ({ children }) => (
    <pre className="my-2 overflow-x-auto rounded-md border border-subtle bg-surface p-2.5 font-mono text-caption">
      {children}
    </pre>
  ),
  table: ({ children }) => (
    <div className="my-2 overflow-x-auto">
      <table className="w-full border-collapse text-caption">{children}</table>
    </div>
  ),
  th: ({ children }) => (
    <th className="border border-subtle bg-surface px-2 py-1 text-left font-medium">{children}</th>
  ),
  td: ({ children }) => <td className="border border-subtle px-2 py-1 align-top">{children}</td>,
  hr: () => <hr className="my-3 border-subtle" />,
};

export function MarkdownPreview({ markdown }: { markdown: string }) {
  return (
    <div className="text-primary">
      <Markdown remarkPlugins={[remarkGfm]} components={components}>
        {markdown}
      </Markdown>
    </div>
  );
}
