# ADR 0001 — Tauri 2 em vez de Electron

- **Status:** aceito
- **Data:** 2026-09-22
- **Fase:** 00

## Contexto

O AISENSE é um aplicativo de desktop que mantém 6–12 processos PTY vivos por horas. Ele é
**infraestrutura**: fica aberto o dia inteiro ao lado do editor e do navegador. O custo de memória
do próprio app compete diretamente com a memória que as ferramentas do usuário precisam.

Medição de referência: um Electron vazio consome ~250–300 MB de RSS; com uma UI real e alguns
xterm.js, passa fácil de 500 MB. Somando a isso os processos dos agentes (cada `claude`/`codex` é um
Node ou Python de centenas de MB), o app vira o vilão da máquina.

## Opções consideradas

| Opção | Prós | Contras |
|---|---|---|
| **Tauri 2** | ~40 MB de base; backend Rust nativo para PTY; bundle ~10 MB; sidecars; IPC tipada; updater assinado | Webview varia por SO (WebKitGTK no Linux é o elo fraco); ecossistema menor; debugging de webview pior |
| Electron | Chromium idêntico nos 3 SOs; `node-pty` maduríssimo; devtools excelentes; ecossistema gigante | 6–10× mais memória; bundle 15× maior; PTY em Node gasta CPU em `Buffer`/GC no caminho quente |
| Web + daemon local | Acesso remoto de graça; dev web puro | Instalação em duas partes; UX de app pior; problema de segurança de origem; atalhos de SO limitados |
| Nativo por SO | Melhor performance e integração possível | Três interfaces para manter — inviável para o tamanho da equipe |

## Decisão

**Tauri 2**, com core em Rust e front em React.

## Consequências

**Positivas**
- Orçamento de 150 MB com 6 agentes ociosos vira atingível.
- O pump de PTY roda em Rust, longe do event loop do JS — sem engasgo de terminal.
- Os sidecars `aisense` e `aisense-mcp` são binários Rust minúsculos, distribuídos junto.
- Instalador de ~25 MB, download rápido, atualização barata.

**Negativas / custos aceitos**
- Diferenças de webview: WebKitGTK atrasa em CSS moderno. Mitigação: CI com screenshots nos 3 SOs
  desde a Fase 0 e regra de não usar CSS de ponta sem fallback.
- Menos respostas prontas no Stack Overflow. Mitigação: ADRs e documentação interna densa.
- Debug de webview no Linux é pior. Mitigação: desenvolver primariamente em macOS/Windows e
  validar no Linux em CI e antes de cada release.

**O que passa a ser proibido**
- Depender de APIs exclusivas do Chromium (ex.: File System Access API).
- Colocar lógica pesada no JS "porque é mais rápido de escrever".

## Quando revisitar

Se o WebKitGTK causar mais de três bugs bloqueantes de renderização em um ciclo, ou se o consumo
de memória do Tauri com 12 xterm.js ultrapassar 600 MB (o que anularia a vantagem principal).
