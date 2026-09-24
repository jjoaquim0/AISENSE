#!/usr/bin/env node
// Compila os sidecars `aisense` e `aisense-mcp` e os põe onde o empacotador do Tauri os
// procura (F09-01): `crates/aisense-app/binaries/<nome>-<target triple>[.exe]`. O
// `tauri build` copia cada um para o lado do executável do app, sem o sufixo.
//
// Uso: node scripts/sidecars.mjs [--target <triple>] [--debug]
//   --target universal-apple-darwin  compila arm64 e x86_64 e junta com `lipo`.
import { execFileSync } from 'node:child_process';
import { chmodSync, copyFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const out = join(root, 'crates', 'aisense-app', 'binaries');
const BINS = ['aisense', 'aisense-mcp'];

const argv = process.argv.slice(2);
const flag = (name) => {
  const i = argv.indexOf(name);
  return i >= 0 ? argv[i + 1] : undefined;
};
const debug = argv.includes('--debug');
const profile = debug ? 'debug' : 'release';

function run(cmd, args) {
  console.log(`$ ${cmd} ${args.join(' ')}`);
  execFileSync(cmd, args, { cwd: root, stdio: 'inherit' });
}

function hostTriple() {
  const info = execFileSync('rustc', ['-vV'], { encoding: 'utf8' });
  const line = info.split('\n').find((l) => l.startsWith('host: '));
  if (!line) throw new Error('rustc -vV não informou o host');
  return line.slice('host: '.length).trim();
}

/** Compila para `triple` e devolve a pasta dos binários. */
function build(triple, explicit) {
  const args = ['build', '--locked', '-p', 'aisense-cli', '-p', 'aisense-mcp'];
  if (!debug) args.push('--release');
  if (explicit) args.push('--target', triple);
  run('cargo', args);
  return explicit ? join(root, 'target', triple, profile) : join(root, 'target', profile);
}

const target = flag('--target');
const triple = target ?? hostTriple();
const exe = triple.includes('windows') ? '.exe' : '';
mkdirSync(out, { recursive: true });

if (triple === 'universal-apple-darwin') {
  const arm = build('aarch64-apple-darwin', true);
  const intel = build('x86_64-apple-darwin', true);
  for (const bin of BINS) {
    const dest = join(out, `${bin}-${triple}`);
    run('lipo', ['-create', '-output', dest, join(arm, bin), join(intel, bin)]);
    chmodSync(dest, 0o755);
  }
} else {
  const dir = build(triple, Boolean(target));
  for (const bin of BINS) {
    const dest = join(out, `${bin}-${triple}${exe}`);
    copyFileSync(join(dir, `${bin}${exe}`), dest);
    // O risco da fase: sidecar sem permissão de execução depois de empacotado.
    chmodSync(dest, 0o755);
    console.log(`→ ${dest}`);
  }
}
