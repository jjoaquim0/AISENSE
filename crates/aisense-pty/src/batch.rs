//! Coalescência da saída do PTY antes de chegar na interface.
//!
//! Um agente falando muito emite dezenas de milhares de bytes por segundo. Emitir um
//! evento por leitura derruba a UI. O `Batcher` acumula e libera no máximo um lote por
//! janela (16 ms ≈ 60 fps), e quando o painel está invisível **não libera nada** — o
//! histórico continua no ring buffer e o front reidrata ao voltar.
//!
//! A lógica é pura e recebe o instante por parâmetro: assim o teste é determinístico,
//! sem `sleep` e sem depender do relógio da máquina.

use std::time::{Duration, Instant};

/// Janela padrão de coalescência (~60 fps).
pub const DEFAULT_WINDOW: Duration = Duration::from_millis(16);

#[derive(Debug)]
pub struct Batcher {
    window: Duration,
    pending: Vec<u8>,
    last_flush: Option<Instant>,
    visible: bool,
    suppressed: u64,
}

impl Batcher {
    pub fn new(window: Duration) -> Self {
        Self {
            window,
            pending: Vec::new(),
            last_flush: None,
            visible: true,
            suppressed: 0,
        }
    }

    pub fn with_default_window() -> Self {
        Self::new(DEFAULT_WINDOW)
    }

    /// Marca se o painel deste agente está na tela. Invisível não gera evento algum.
    pub fn set_visible(&mut self, visible: bool) {
        if visible == self.visible {
            return;
        }
        self.visible = visible;
        if !visible {
            // O que estava para sair é descartado: ao voltar, o front pede o snapshot
            // inteiro do ring buffer, que é mais barato e não deixa buraco.
            self.pending.clear();
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Quantos chunks foram ignorados por estar invisível. Só para diagnóstico.
    pub fn suppressed(&self) -> u64 {
        self.suppressed
    }

    /// Acumula um chunk vindo do PTY.
    pub fn push(&mut self, data: &[u8]) {
        if !self.visible {
            self.suppressed += 1;
            return;
        }
        self.pending.extend_from_slice(data);
    }

    /// Devolve o lote se a janela fechou. `None` significa "ainda não é hora".
    pub fn poll(&mut self, now: Instant) -> Option<Vec<u8>> {
        if self.pending.is_empty() {
            return None;
        }
        match self.last_flush {
            // Primeiro chunk depois de um período parado sai na hora: sem isso, cada
            // tecla digitada teria 16 ms de atraso visível no eco.
            None => Some(self.flush(now)),
            Some(last) if now.duration_since(last) >= self.window => Some(self.flush(now)),
            Some(_) => None,
        }
    }

    /// Quanto falta para o próximo lote poder sair. Usado para dormir o tempo exato.
    pub fn time_until_ready(&self, now: Instant) -> Option<Duration> {
        if self.pending.is_empty() {
            return None;
        }
        match self.last_flush {
            None => Some(Duration::ZERO),
            Some(last) => Some(self.window.saturating_sub(now.duration_since(last))),
        }
    }

    /// Esvazia o que houver, ignorando a janela. Usado ao encerrar a sessão para não
    /// perder as últimas linhas (tipicamente a mensagem de erro que interessa).
    pub fn drain(&mut self, now: Instant) -> Option<Vec<u8>> {
        if self.pending.is_empty() {
            return None;
        }
        Some(self.flush(now))
    }

    fn flush(&mut self, now: Instant) -> Vec<u8> {
        self.last_flush = Some(now);
        std::mem::take(&mut self.pending)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(base: Instant, millis: u64) -> Instant {
        base + Duration::from_millis(millis)
    }

    #[test]
    fn o_primeiro_chunk_sai_imediatamente() {
        // O eco do que você digita não pode esperar a janela.
        let base = Instant::now();
        let mut batcher = Batcher::with_default_window();
        batcher.push(b"a");
        assert_eq!(batcher.poll(base).as_deref(), Some(&b"a"[..]));
    }

    #[test]
    fn segura_os_chunks_dentro_da_janela_e_junta_tudo() {
        let base = Instant::now();
        let mut batcher = Batcher::with_default_window();

        batcher.push(b"inicio");
        assert!(batcher.poll(base).is_some());

        batcher.push(b"um ");
        batcher.push(b"dois ");
        batcher.push(b"tres");
        assert!(
            batcher.poll(at(base, 5)).is_none(),
            "ainda dentro da janela"
        );
        assert!(
            batcher.poll(at(base, 15)).is_none(),
            "ainda dentro da janela"
        );
        assert_eq!(
            batcher.poll(at(base, 16)).as_deref(),
            Some(&b"um dois tres"[..])
        );
    }

    #[test]
    fn saida_intensa_gera_no_maximo_um_lote_por_janela() {
        let base = Instant::now();
        let mut batcher = Batcher::new(Duration::from_millis(16));
        let mut lotes = 0;

        // 1 segundo de saída, um chunk a cada milissegundo.
        for millis in 0..1_000 {
            batcher.push(b"x");
            if batcher.poll(at(base, millis)).is_some() {
                lotes += 1;
            }
        }

        // ~60 fps: 1000/16 = 62, mais o lote imediato inicial.
        assert!(lotes <= 64, "lotes emitidos em 1s: {lotes}");
        assert!(lotes >= 60, "lotes emitidos em 1s: {lotes}");
    }

    #[test]
    fn painel_invisivel_nao_emite_nada() {
        let base = Instant::now();
        let mut batcher = Batcher::with_default_window();
        batcher.set_visible(false);

        for _ in 0..1_000 {
            batcher.push(b"barulho");
        }

        assert!(batcher.poll(at(base, 10_000)).is_none());
        assert_eq!(batcher.suppressed(), 1_000);
    }

    #[test]
    fn ao_voltar_a_ficar_visivel_nao_despeja_o_atrasado() {
        // O front reidrata pelo snapshot do ring buffer; repetir aqui duplicaria a saída.
        let base = Instant::now();
        let mut batcher = Batcher::with_default_window();

        batcher.push(b"antes");
        batcher.set_visible(false);
        batcher.push(b"durante");
        batcher.set_visible(true);

        assert!(batcher.poll(base).is_none(), "nada represado deve sair");

        batcher.push(b"depois");
        assert_eq!(batcher.poll(base).as_deref(), Some(&b"depois"[..]));
    }

    #[test]
    fn drain_nao_perde_as_ultimas_linhas_no_encerramento() {
        let base = Instant::now();
        let mut batcher = Batcher::with_default_window();

        batcher.push(b"primeiro");
        assert!(batcher.poll(base).is_some());

        batcher.push(b"erro fatal na linha 42");
        assert!(batcher.poll(at(base, 1)).is_none(), "dentro da janela");
        assert_eq!(
            batcher.drain(at(base, 1)).as_deref(),
            Some(&b"erro fatal na linha 42"[..])
        );
        assert!(
            batcher.drain(at(base, 2)).is_none(),
            "drain duas vezes não repete"
        );
    }

    #[test]
    fn diz_quanto_falta_para_o_proximo_lote() {
        let base = Instant::now();
        let mut batcher = Batcher::new(Duration::from_millis(16));

        assert_eq!(batcher.time_until_ready(base), None, "sem nada pendente");

        batcher.push(b"a");
        assert_eq!(batcher.time_until_ready(base), Some(Duration::ZERO));
        batcher.poll(base);

        batcher.push(b"b");
        assert_eq!(
            batcher.time_until_ready(at(base, 6)),
            Some(Duration::from_millis(10))
        );
        assert_eq!(
            batcher.time_until_ready(at(base, 99)),
            Some(Duration::ZERO),
            "atrasado"
        );
    }
}
