//! Transcrição completa da sessão em arquivo.
//!
//! O ring buffer guarda as últimas N linhas para reidratar a tela; o log guarda
//! **tudo**, para você abrir depois e entender o que o agente fez. São coisas
//! diferentes e de propósito: um é memória, o outro é histórico.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Limite por agente antes de rotacionar. Ver retenção em `docs/04-modelo-de-dados.md`.
pub const DEFAULT_MAX_LOG_BYTES: u64 = 200 * 1024 * 1024;

#[derive(Debug)]
pub struct SessionLog {
    path: PathBuf,
    file: File,
    written: u64,
    max_bytes: u64,
}

impl SessionLog {
    pub fn create(path: impl Into<PathBuf>) -> std::io::Result<Self> {
        Self::with_limit(path, DEFAULT_MAX_LOG_BYTES)
    }

    pub fn with_limit(path: impl Into<PathBuf>, max_bytes: u64) -> std::io::Result<Self> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let written = file.metadata().map(|meta| meta.len()).unwrap_or(0);
        Ok(Self {
            path,
            file,
            written,
            max_bytes: max_bytes.max(1),
        })
    }

    pub fn append(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.file.write_all(data)?;
        self.written += data.len() as u64;
        if self.written >= self.max_bytes {
            self.rotate()?;
        }
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn written(&self) -> u64 {
        self.written
    }

    /// Caminho do arquivo anterior, criado pela rotação.
    pub fn previous_path(&self) -> PathBuf {
        self.path.with_extension("1.log")
    }

    /// Move o atual para `.1.log` e recomeça. Guardamos uma geração: duas já cobrem
    /// "o que acabou de acontecer" sem encher o disco do usuário.
    fn rotate(&mut self) -> std::io::Result<()> {
        self.file.flush()?;
        let previous = self.previous_path();
        std::fs::rename(&self.path, &previous)?;
        self.file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        self.written = 0;
        tracing::debug!(path = %self.path.display(), "log da sessão rotacionado");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn grava_o_que_recebe() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agente.log");

        let mut log = SessionLog::create(&path).unwrap();
        log.append(b"primeira linha\n").unwrap();
        log.append(b"segunda linha\n").unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "primeira linha\nsegunda linha\n");
    }

    #[test]
    fn cria_o_diretorio_se_nao_existir() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("logs").join("fundo").join("agente.log");

        let mut log = SessionLog::create(&path).unwrap();
        log.append(b"ok\n").unwrap();

        assert!(path.exists());
    }

    #[test]
    fn continua_um_arquivo_existente_sem_apagar() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agente.log");
        std::fs::write(&path, b"sessao anterior\n").unwrap();

        let mut log = SessionLog::create(&path).unwrap();
        assert_eq!(log.written(), 16);
        log.append(b"sessao nova\n").unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "sessao anterior\nsessao nova\n");
    }

    #[test]
    fn rotaciona_ao_atingir_o_limite_sem_perder_o_anterior() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agente.log");

        let mut log = SessionLog::with_limit(&path, 32).unwrap();
        log.append(&[b'a'; 40]).unwrap();
        log.append(b"depois da rotacao\n").unwrap();

        let atual = std::fs::read_to_string(&path).unwrap();
        let anterior = std::fs::read_to_string(log.previous_path()).unwrap();

        assert_eq!(atual, "depois da rotacao\n");
        assert_eq!(anterior.len(), 40, "o conteúdo antigo é preservado");
    }

    #[test]
    fn o_tamanho_nao_cresce_sem_limite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agente.log");

        let mut log = SessionLog::with_limit(&path, 1024).unwrap();
        for _ in 0..500 {
            log.append(&[b'x'; 100]).unwrap();
        }

        // No máximo o arquivo atual mais uma geração anterior.
        let atual = std::fs::metadata(&path).unwrap().len();
        let anterior = std::fs::metadata(log.previous_path())
            .map(|m| m.len())
            .unwrap_or(0);
        assert!(
            atual + anterior <= 1024 * 2 + 200,
            "atual={atual} anterior={anterior}"
        );
    }
}
