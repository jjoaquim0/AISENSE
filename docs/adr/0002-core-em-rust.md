# ADR 0002 — Domínio, PTY e barramento em Rust

- **Status:** aceito
- **Data:** 2026-09-22
- **Fase:** 00

## Contexto

Tauri já exige Rust no processo principal, mas isso não obriga a colocar o **domínio** lá — daria
para fazer uma casca fina em Rust e a lógica toda no TypeScript.

O que decide a questão é o caminho crítico: ler a saída de 12 PTYs simultâneos, manter ring buffers,
rodar detecção de estado por regex e rotear mensagens. É trabalho contínuo, sensível a latência e
totalmente I/O-bound concorrente.

## Opções consideradas

| Opção | Prós | Contras |
|---|---|---|
| **Domínio em Rust** | Sem GC no caminho quente; concorrência estrutural com Tokio; tipos gerados para o TS; core testável sem UI | Curva de aprendizado; iteração mais lenta que TS |
| Domínio no front (TS), Rust só como casca | Iteração rápida; um idioma só | Toda a saída de PTY atravessaria a ponte IPC e passaria pelo event loop do JS — o gargalo exato que queremos evitar; regra de negócio some junto com a UI |
| Domínio em Go com FFI | Boa concorrência | FFI com Tauri é atrito puro; dois toolchains sem ganho |

## Decisão

Todo o domínio (equipes, agentes, skills, barramento, tarefas, políticas) vive em
`aisense-core`, em Rust. O TypeScript cuida **apenas** de apresentação e interação.

## Consequências

**Positivas**
- A saída de PTY nunca cruza a ponte IPC quando o painel não está visível — economia enorme.
- `cargo test -p aisense-core` roda o domínio inteiro sem abrir janela: testes rápidos e determinísticos.
- Um dia é possível um modo daemon headless (D4) sem reescrever nada.
- `ts-rs` mantém os tipos sincronizados automaticamente.

**Negativas / custos aceitos**
- Toda feature que atravessa camadas toca Rust e TS. Compensado por `ts-rs`.
- Contribuidores precisam saber os dois. Mitigado por `docs/10-padroes-de-codigo.md`.

**O que passa a ser proibido**
- Regra de negócio em componente React.
- `aisense-core` importar `tauri`.
- TypeScript escrever tipo de domínio à mão em vez de usar o gerado.

## Quando revisitar

Se em três ciclos seguidos o custo de sincronizar Rust↔TS dominar o tempo de desenvolvimento de
features que não tocam performance.
