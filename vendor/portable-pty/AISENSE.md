# portable-pty 0.9.0 — cópia com correções para o Windows

Cópia de [`portable-pty` 0.9.0](https://crates.io/crates/portable-pty) (MIT, `LICENSE.md`),
ligada ao workspace por `[patch.crates-io]` no `Cargo.toml` da raiz. Só o código do Windows
foi alterado; em Linux e macOS o comportamento é o do original.

Todas as mudanças estão marcadas com `AISENSE:` no código.

## 1. Flags do `CreatePseudoConsole` (`src/win/psuedocon.rs`)

O original cria o pseudo-console com `PSEUDOCONSOLE_INHERIT_CURSOR | RESIZE_QUIRK |
WIN32_INPUT_MODE`. No conhost nativo do Windows 10 (verificado no 19045):

- `INHERIT_CURSOR` faz o ConPTY abrir a sessão pedindo a posição do cursor (`ESC[6n`) e não
  entregar nada até alguém responder. Mesmo respondendo, a saída passa a sair ~1 linha a cada
  500 ms (o tique do cursor), e um `for` de 300 linhas leva minutos.
- `WIN32_INPUT_MODE` só faz sentido para um terminal que fala o protocolo de entrada win32
  do Windows Terminal. O xterm.js não fala.

Ficou só `RESIZE_QUIRK`. VS Code/node-pty também não usam `INHERIT_CURSOR` por padrão.

## 2. `kill` com resultado invertido (`src/win/mod.rs`)

`TerminateProcess` devolve **diferente de zero em caso de sucesso**. O original tratava isso
como erro (lendo um `GetLastError` antigo, tipicamente "Identificador inválido") e tratava a
falha real como sucesso. Corrigido nos dois lugares (`do_kill` e `WinChildKiller::kill`).

## Quando remover esta cópia

Quando uma versão publicada do `portable-pty` corrigir os dois pontos (ou permitir escolher as
flags), apague esta pasta e o `[patch.crates-io]`. Os testes de `aisense-pty` rodam no Windows
e pegam a regressão.
