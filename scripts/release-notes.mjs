#!/usr/bin/env node
// Notas do release a partir dos commits desde a tag anterior (F09-07).
// Uso: node scripts/release-notes.mjs v0.2.0 > notes.md
//
// Agrupa pelo tipo do Conventional Commit (`feat(F09-03): ...`) e mantém o ID da tarefa,
// que liga a nota ao documento da fase. Merges e commits de docs/ci/chore ficam de fora.
import { execFileSync } from 'node:child_process';

const tag = process.argv[2];
if (!tag) {
  console.error('uso: release-notes.mjs <tag>');
  process.exit(1);
}

const git = (...args) =>
  execFileSync('git', args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim();

function previousTag() {
  try {
    return git('describe', '--tags', '--abbrev=0', '--match', 'v*', `${tag}^`);
  } catch {
    return null; // primeiro release: todos os commits
  }
}

const since = previousTag();
const range = since ? `${since}..${tag}` : tag;
const log = git('log', '--no-merges', '--format=%s%x1f%h', range);

const SECTIONS = [
  ['feat', 'Novidades'],
  ['fix', 'Correções'],
  ['perf', 'Desempenho'],
  ['refactor', 'Por dentro'],
];
const HIDDEN = new Set(['docs', 'ci', 'chore', 'test', 'style', 'build']);
const groups = new Map(SECTIONS.map(([type]) => [type, []]));
const other = [];

for (const line of log ? log.split('\n') : []) {
  const [subject, hash] = line.split('\x1f');
  const match = /^(\w+)(?:\(([^)]+)\))?!?:\s*(.+)$/.exec(subject);
  if (!match) {
    other.push(`- ${subject} (${hash})`);
    continue;
  }
  const [, type, scope, text] = match;
  if (HIDDEN.has(type)) continue;
  const entry = `- ${scope ? `**${scope}** ` : ''}${text} (${hash})`;
  (groups.get(type) ?? other).push(entry);
}

const out = [];
for (const [type, title] of SECTIONS) {
  const entries = groups.get(type);
  if (entries.length) out.push(`## ${title}\n\n${entries.join('\n')}\n`);
}
if (other.length) out.push(`## Outras mudanças\n\n${other.join('\n')}\n`);
if (!out.length) out.push('Sem mudanças visíveis desde o release anterior.\n');

const repo = process.env.GITHUB_REPOSITORY;
if (since && repo) {
  out.push(`**Todas as mudanças:** https://github.com/${repo}/compare/${since}...${tag}\n`);
}
out.push(
  '## Instalação\n\n' +
    '| Sistema | Arquivo |\n|---|---|\n' +
    '| macOS (Apple Silicon e Intel) | `.dmg` |\n' +
    '| Windows | `-setup.exe` (ou `.msi`) |\n' +
    '| Linux | `.AppImage` ou `.deb` |\n\n' +
    'Quem já tem o AISENSE recebe esta versão pela atualização automática.\n',
);
process.stdout.write(out.join('\n'));
