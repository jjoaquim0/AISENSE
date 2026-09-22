//! Buffer circular da saída de um terminal.
//!
//! Guarda as últimas N linhas em RAM para reidratar o xterm quando o painel volta a
//! ficar visível (`docs/02-arquitetura.md`, fluxo 2). Fica em Rust, e não no heap da
//! webview, justamente para o histórico de 12 agentes não pesar na interface.

use std::collections::VecDeque;

/// Limites padrão. Ver `docs/04-modelo-de-dados.md` (retenção).
pub const DEFAULT_MAX_LINES: usize = 10_000;
pub const DEFAULT_MAX_BYTES: usize = 8 * 1024 * 1024;
/// Acima disto, uma "linha" é fatiada em várias entradas. Não corrompe nada: o
/// snapshot concatena as entradas, então os bytes saem idênticos. Serve só para que
/// uma linha gigante (um `cat` de binário) não vire uma entrada monolítica.
const MAX_ENTRY_BYTES: usize = 64 * 1024;

#[derive(Debug)]
pub struct RingBuffer {
    entries: VecDeque<Vec<u8>>,
    bytes: usize,
    max_lines: usize,
    max_bytes: usize,
    dropped: u64,
}

impl Default for RingBuffer {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_LINES, DEFAULT_MAX_BYTES)
    }
}

impl RingBuffer {
    pub fn new(max_lines: usize, max_bytes: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            bytes: 0,
            max_lines: max_lines.max(1),
            max_bytes: max_bytes.max(1),
            dropped: 0,
        }
    }

    /// Acrescenta bytes crus vindos do PTY. Pode conter linhas parciais e ANSI.
    pub fn push(&mut self, mut data: &[u8]) {
        while !data.is_empty() {
            // O pedaço vai até a próxima quebra de linha, mas nunca passa do tamanho
            // máximo de entrada — vale também quando a entrada é nova, senão um único
            // `push` grande (um `cat` de binário) vira uma entrada monolítica.
            let line_end = data
                .iter()
                .position(|&b| b == b'\n')
                .map_or(data.len(), |index| index + 1);
            let (chunk, rest) = data.split_at(line_end.min(MAX_ENTRY_BYTES));

            match self.entries.back_mut() {
                // A última entrada está "aberta" (sem \n) e ainda cabe: continua nela.
                Some(last)
                    if !last.ends_with(b"\n") && last.len() + chunk.len() <= MAX_ENTRY_BYTES =>
                {
                    last.extend_from_slice(chunk);
                }
                _ => self.entries.push_back(chunk.to_vec()),
            }
            self.bytes += chunk.len();
            data = rest;
        }
        self.evict();
    }

    /// Todo o conteúdo retido, na ordem. É o que o front escreve de uma vez no xterm.
    pub fn snapshot(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.bytes);
        for entry in &self.entries {
            out.extend_from_slice(entry);
        }
        out
    }

    pub fn bytes(&self) -> usize {
        self.bytes
    }

    pub fn entries(&self) -> usize {
        self.entries.len()
    }

    /// Quantas entradas foram descartadas por limite. A UI usa para avisar que o
    /// histórico foi truncado em vez de fingir que aquilo é o começo da sessão.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.bytes = 0;
    }

    fn evict(&mut self) {
        while self.entries.len() > self.max_lines || self.bytes > self.max_bytes {
            match self.entries.pop_front() {
                Some(removed) => {
                    self.bytes -= removed.len();
                    self.dropped += 1;
                }
                // Só acontece se a deque esvaziar: nada mais a remover.
                None => break,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(buffer: &RingBuffer) -> String {
        String::from_utf8_lossy(&buffer.snapshot()).into_owned()
    }

    #[test]
    fn reconstroi_os_bytes_exatamente() {
        let mut buffer = RingBuffer::default();
        buffer.push(b"primeira\nsegunda\n");
        assert_eq!(text(&buffer), "primeira\nsegunda\n");
    }

    #[test]
    fn junta_linha_partida_entre_dois_chunks() {
        // O PTY entrega pedaços arbitrários: uma linha pode chegar em 3 leituras.
        let mut buffer = RingBuffer::default();
        buffer.push(b"come");
        buffer.push(b"cou aqui");
        buffer.push(b" e terminou\n");
        assert_eq!(text(&buffer), "comecou aqui e terminou\n");
        assert_eq!(buffer.entries(), 1);
    }

    #[test]
    fn preserva_sequencias_ansi_e_utf8() {
        let mut buffer = RingBuffer::default();
        buffer.push("\x1b[31mvermelho\x1b[0m ção 🚀\n".as_bytes());
        assert_eq!(text(&buffer), "\u{1b}[31mvermelho\u{1b}[0m ção 🚀\n");
    }

    #[test]
    fn descarta_as_linhas_mais_antigas_ao_estourar_o_limite() {
        let mut buffer = RingBuffer::new(3, DEFAULT_MAX_BYTES);
        for i in 1..=5 {
            buffer.push(format!("linha {i}\n").as_bytes());
        }
        assert_eq!(text(&buffer), "linha 3\nlinha 4\nlinha 5\n");
        assert_eq!(buffer.dropped(), 2);
    }

    #[test]
    fn respeita_o_limite_de_bytes() {
        let mut buffer = RingBuffer::new(DEFAULT_MAX_LINES, 32);
        for i in 0..100 {
            buffer.push(format!("{i:08}\n").as_bytes());
        }
        assert!(buffer.bytes() <= 32, "bytes retidos: {}", buffer.bytes());
        assert!(buffer.entries() > 0, "não pode esvaziar completamente");
    }

    #[test]
    fn uma_linha_gigante_nao_vira_uma_entrada_monolitica() {
        let mut buffer = RingBuffer::default();
        let huge = vec![b'x'; MAX_ENTRY_BYTES * 3];
        buffer.push(&huge);
        assert!(buffer.entries() >= 3, "entradas: {}", buffer.entries());
        // Mesmo fatiada, a reconstrução é byte a byte idêntica.
        assert_eq!(buffer.snapshot(), huge);
    }

    #[test]
    fn memoria_fica_estavel_com_muita_saida() {
        let mut buffer = RingBuffer::new(1_000, DEFAULT_MAX_BYTES);
        for i in 0..100_000 {
            buffer.push(format!("linha de log número {i}\n").as_bytes());
        }
        assert_eq!(buffer.entries(), 1_000);
        assert!(
            buffer.bytes() < 64 * 1024,
            "bytes retidos: {}",
            buffer.bytes()
        );
        assert!(text(&buffer).ends_with("linha de log número 99999\n"));
    }

    #[test]
    fn clear_zera_o_conteudo_mas_mantem_o_contador_de_descarte() {
        let mut buffer = RingBuffer::new(1, DEFAULT_MAX_BYTES);
        buffer.push(b"a\nb\n");
        let dropped = buffer.dropped();
        buffer.clear();
        assert_eq!(buffer.snapshot(), b"");
        assert_eq!(buffer.bytes(), 0);
        assert_eq!(buffer.dropped(), dropped);
    }
}
