//! Comandos de teste que se comportam igual nos três sistemas.
//!
//! Os testes antigos usavam `cat`, `sleep`, `seq` e `stty` dentro de `cmd /C` no
//! Windows. Esses comandos não existem lá: o processo morria na hora, a escrita
//! seguinte dava `ERROR_INVALID_HANDLE` e os testes pareciam bugs do PTY.
#![allow(clippy::expect_used)]

use crate::session::PtySpawn;

/// Roda um script no shell do sistema (`sh -c` / `cmd /C`).
pub fn shell(script: &str) -> PtySpawn {
    if cfg!(windows) {
        PtySpawn::new("cmd").arg("/C").arg(script)
    } else {
        PtySpawn::new("sh").arg("-c").arg(script)
    }
}

/// Imprime `text` e sai com `code`.
pub fn echo_then_exit(text: &str, code: i32) -> PtySpawn {
    if cfg!(windows) {
        shell(&format!("echo {text}& exit /b {code}"))
    } else {
        shell(&format!("echo {text}; exit {code}"))
    }
}

/// Devolve o que for digitado, linha a linha, até ser morto (o papel do `cat`).
pub fn echo_stdin() -> PtySpawn {
    if cfg!(windows) {
        // `findstr "^"` casa toda linha e repete o que lê da entrada.
        PtySpawn::new("findstr").arg("^")
    } else {
        PtySpawn::new("cat")
    }
}

/// Fica vivo por ~60 s sem imprimir nada relevante (o papel do `sleep 60`).
pub fn long_running() -> PtySpawn {
    if cfg!(windows) {
        PtySpawn::new("ping").arg("-n").arg("61").arg("127.0.0.1")
    } else {
        PtySpawn::new("sleep").arg("60")
    }
}

/// Imprime `linha 1` .. `linha {count}`.
///
/// No Windows, um `echo` por linha dentro de `for /L` custa ~200 ms cada no ConPTY (é o
/// conhost redesenhando a tela a cada escrita). Um arquivo impresso com `type` sai numa
/// escrita só, que é o que estes testes querem medir.
pub fn many_lines(count: u32) -> PtySpawn {
    if cfg!(windows) {
        let path = std::env::temp_dir().join(format!("aisense-linhas-{count}.txt"));
        let text: String = (1..=count).map(|i| format!("linha {i}\r\n")).collect();
        std::fs::write(&path, text).expect("arquivo de linhas para o teste");
        // Sem aspas: o `portable-pty` as escaparia como `\"`, que o `cmd` não entende. A
        // pasta temporária do usuário não tem espaço.
        shell(&format!("type {}", path.display()))
    } else {
        shell(&format!(
            "for i in $(seq 1 {count}); do echo linha $i; done"
        ))
    }
}

/// Mostra o tamanho do terminal: `"<linhas> <colunas>"` em Unix; no Windows, a saída de
/// `mode con` (localizada, mas com os dois números).
pub fn print_size() -> PtySpawn {
    if cfg!(windows) {
        shell("mode con")
    } else {
        shell("stty size")
    }
}

/// Imprime `"{label}=<valor da variável>"`.
pub fn print_env(var: &str, label: &str) -> PtySpawn {
    if cfg!(windows) {
        shell(&format!("echo {label}=%{var}%"))
    } else {
        shell(&format!("echo {label}=${var}"))
    }
}

/// Volume para o teste de memória do ring buffer (limite de 500 linhas). No Windows 10 o
/// ConPTY nativo rola só ~30 linhas/s (ver `docs/ESTADO.md`, riscos), então lá usamos o
/// mínimo que ainda estoura o limite com folga.
pub const VOLUME_LINES: u32 = if cfg!(windows) { 800 } else { 20_000 };

/// Linhas do teste de coalescência do gerenciador (5 000 fora do Windows, como antes).
pub const INTENSE_LINES: u32 = if cfg!(windows) { 250 } else { 5_000 };
