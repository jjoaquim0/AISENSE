# Distribuição — empacotar, assinar, atualizar e lançar

> Para quem mantém o AISENSE. Como os instaladores são feitos (F09-01), assinados (F09-02),
> atualizados (F09-03) e publicados por tag (F09-07). Guia de quem **usa** o app: o
> [README](../README.md) e [`docs/guia/`](guia/).

## O que sai de um release

| SO | Arquivos | Observação |
|---|---|---|
| macOS | `AISENSE_<v>_universal.dmg`, `AISENSE.app.tar.gz` (+ `.sig`) | binário universal (arm64 + x86_64) |
| Windows | `AISENSE_<v>_x64-setup.exe` (NSIS), `AISENSE_<v>_x64_en-US.msi` (+ `.sig`) | o NSIS instala por usuário, sem admin |
| Linux | `AISENSE_<v>_amd64.AppImage` (+ `.sig`), `AISENSE_<v>_amd64.deb`, `.rpm` | compilado no Ubuntu 22.04 (glibc 2.35) |
| todos | `latest.json` | manifesto do updater, gerado pelo `tauri-action` |

Todo pacote leva os sidecars `aisense` e `aisense-mcp` ao lado do executável do app (no `.deb`,
em `/usr/bin`). O supervisor os põe na frente do `PATH` de cada agente.

## Empacotar localmente

```bash
pnpm app:build     # = node scripts/sidecars.mjs && tauri build --config crates/aisense-app/tauri.bundle.conf.json
```

- `scripts/sidecars.mjs` compila `aisense`/`aisense-mcp` em release e os copia para
  `crates/aisense-app/binaries/<nome>-<target triple>` (fora do git), que é onde o `externalBin` do
  Tauri os procura. `--target universal-apple-darwin` compila os dois arcos e junta com `lipo`.
- `tauri.bundle.conf.json` só liga o `externalBin`. Ele fica fora do `tauri.conf.json` de propósito:
  o `tauri-build` exige os sidecars em disco, e o `cargo build -p aisense-app` do dia a dia (e o
  clippy do CI) não deve depender de compilá-los antes.
- `tauri.release.conf.json` é **gerado** por `scripts/release-config.mjs` a partir de
  `AISENSE_UPDATER_PUBKEY` (fora do git): liga `createUpdaterArtifacts` e põe a chave pública em
  `plugins.updater.pubkey`. Exige também a chave privada (`TAURI_SIGNING_PRIVATE_KEY`); só o
  workflow de release usa. Para testar localmente, gere um par com `tauri signer generate`.

**Teste de fumaça do pacote:** `scripts/smoke-install.py <executável instalado>` sobe o app com uma
pasta de dados nova e um agente que roda `aisense whoami` e o `initialize` do `aisense-mcp`, e só
passa se os dois responderem de dentro do terminal do agente. Prova que os sidecars foram
empacotados, executam, estão no `PATH` e falam com o barramento. No Linux sem tela usa `xvfb-run`.

## Lançar uma versão

1. Suba a versão em **dois** lugares, no mesmo commit: `Cargo.toml` (`[workspace.package] version`)
   e `crates/aisense-app/tauri.conf.json` (`version`). O workflow recusa tag que não bate com eles.
2. `git tag v0.2.0 && git push --tags`.
3. O workflow **Release** cria o release como rascunho com as notas geradas dos commits
   (`scripts/release-notes.mjs`, por tipo do Conventional Commit, com o ID da tarefa), compila e
   assina nos 3 SOs, roda o teste de fumaça no pacote de cada um e, só com os três verdes, publica.
   O updater lê `releases/latest/download/latest.json`, então nunca vê um release pela metade.
4. Tag com hífen (`v0.2.0-beta.1`) sai como *pre-release*: o `/latest` do GitHub a ignora, então
   quem está no canal estável não a recebe.

## Segredos (Settings → Secrets and variables → Actions)

Nada disso entra no repositório (R3). Sem os opcionais, o release sai **sem** a assinatura
correspondente e o workflow avisa.

### Updater — obrigatório

```bash
pnpm tauri signer generate -w ~/.tauri/aisense.key   # pede uma senha
```

| Segredo | Valor |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | conteúdo de `~/.tauri/aisense.key` |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | a senha escolhida |
| `AISENSE_UPDATER_PUBKEY` | conteúdo de `~/.tauri/aisense.key.pub` |

A chave pública vai para a config do app no build de release (`scripts/release-config.mjs`): cada
atualização baixada tem a assinatura conferida contra ela — e a versão assinada contra a anunciada
(`requireSignedVersion`) — antes de instalar. Um build sem ela (o de desenvolvimento) não procura
atualizações. **Guarde a chave privada fora da máquina de build**:
perdê-la significa que as instalações existentes não aceitam mais nenhuma atualização.

### macOS — assinatura e notarização

Precisa de uma conta Apple Developer ($99/ano) e de um certificado **Developer ID Application**.

| Segredo | Valor |
|---|---|
| `APPLE_CERTIFICATE` | o `.p12` exportado do Keychain, em base64 (`base64 -i cert.p12`) |
| `APPLE_CERTIFICATE_PASSWORD` | a senha do `.p12` |
| `APPLE_SIGNING_IDENTITY` | `Developer ID Application: Nome (TEAMID)` |
| `APPLE_ID` | e-mail da conta Apple |
| `APPLE_PASSWORD` | senha de app específica (appleid.apple.com → Senhas de app) |
| `APPLE_TEAM_ID` | o Team ID de 10 caracteres |

O `tauri-action` importa o certificado, assina o app e os sidecars com *hardened runtime* e manda
notarizar. Não há entitlements extras: o app só abre processos filhos (os terminais dos agentes), o
que o *hardened runtime* permite.

### Windows — Authenticode

Precisa de um certificado de assinatura de código (OV ou EV) em `.pfx`.

| Segredo | Valor |
|---|---|
| `WINDOWS_CERTIFICATE` | o `.pfx` em base64 (`[Convert]::ToBase64String([IO.File]::ReadAllBytes('cert.pfx'))`) |
| `WINDOWS_CERTIFICATE_PASSWORD` | a senha do `.pfx` |

O workflow importa o `.pfx` no repositório de certificados do runner e passa a impressão digital ao
empacotador (`bundle.windows.certificateThumbprint`), com carimbo de tempo da DigiCert. Certificados
EV em token de hardware não funcionam assim; use `bundle.windows.signCommand` com o serviço de
assinatura do emissor. Um certificado OV novo ainda passa por um período de reputação no
SmartScreen; se o Defender acusar o binário, envie-o para análise em
<https://www.microsoft.com/wdsi/filesubmission>.
