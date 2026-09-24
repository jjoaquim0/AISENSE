#!/usr/bin/env node
// Config extra do `tauri build` de release (F09-03): liga os artefatos do updater e põe a
// chave pública em `plugins.updater.pubkey` — o CLI a exige para assinar, e o app a lê
// dali para conferir cada atualização. A chave vem de `AISENSE_UPDATER_PUBKEY` (o
// conteúdo do `.key.pub` gerado por `tauri signer generate`); nunca fica no repositório.
// Uso: AISENSE_UPDATER_PUBKEY=... node scripts/release-config.mjs > crates/aisense-app/tauri.release.conf.json
const pubkey = (process.env.AISENSE_UPDATER_PUBKEY ?? '').trim();
if (!pubkey) {
  console.error('AISENSE_UPDATER_PUBKEY vazio: sem a chave pública o release não se atualiza');
  process.exit(1);
}
const config = {
  bundle: { createUpdaterArtifacts: true },
  plugins: { updater: { pubkey } },
};
process.stdout.write(`${JSON.stringify(config, null, 2)}\n`);
