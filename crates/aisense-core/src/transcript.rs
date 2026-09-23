//! Transcrição legível de uma sessão (`docs/09`, T6, aba Logs — F03-09).
//!
//! O log em disco é a saída crua do PTY: cores, movimento de cursor, `\r` de barras de
//! progresso. Para ler, buscar e exportar, isso vira texto: escapes somem, `\r` volta ao
//! começo da linha e sobrescreve (a barra de progresso fica só no estado final), `\b`
//! recua. Não é um emulador — TUIs que redesenham a tela inteira ficam repetitivas —,
//! mas saída de linha (builds, testes, a conversa com o agente) fica fiel.
//!
//! O log é um arquivo por agente com as sessões em sequência; cada sessão guarda onde
//! começou (`SessionRecord::log_offset`) e termina onde a seguinte começa.

use std::fs::File;
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ids::SessionId;
use crate::repo::SessionRecord;
use crate::time::Millis;

/// Quanto da transcrição vai para a tela de uma vez. O resto fica para a exportação.
pub const TRANSCRIPT_VIEW_BYTES: u64 = 1024 * 1024;

/// Uma sessão na lista da aba Logs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct SessionSummary {
    pub id: SessionId,
    pub pid: Option<u32>,
    #[ts(type = "number")]
    pub started_at: Millis,
    #[ts(type = "number | null")]
    pub ended_at: Option<Millis>,
    pub exit_code: Option<i32>,
    /// `false` quando o trecho desta sessão não está mais no log (rotacionado, ou
    /// gravada antes de as sessões marcarem onde começam).
    pub available: bool,
}

/// O texto de uma sessão, pronto para ler e buscar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct Transcript {
    pub session_id: SessionId,
    pub text: String,
    /// Só o final coube na tela; a exportação leva tudo.
    pub truncated: bool,
    /// Tamanho do trecho cru no log, em bytes.
    #[ts(type = "number")]
    pub bytes: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum TranscriptError {
    #[error("session {0} not found")]
    SessionNotFound(SessionId),
    #[error("the transcript of session {0} is no longer in the log")]
    Unavailable(SessionId),
    #[error("export path must be absolute: {0}")]
    RelativePath(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl TranscriptError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::SessionNotFound(_) => "session_not_found",
            Self::Unavailable(_) => "transcript_unavailable",
            Self::RelativePath(_) => "invalid_path",
            Self::Io(_) => "io_error",
        }
    }
}

/// Onde uma sessão está no log: `[start, end)`; `end = None` vai até o fim do arquivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogRange {
    pub path: String,
    pub start: u64,
    pub end: Option<u64>,
}

/// Trecho de cada sessão, a partir da lista do repositório (mais recente primeiro).
/// A sessão termina onde a seguinte, no mesmo arquivo, começa. Se a seguinte começa
/// *antes* (o log rotacionou no meio) ou o arquivo encolheu, o trecho não existe mais.
pub fn session_range(sessions: &[SessionRecord], index: usize, file_len: u64) -> Option<LogRange> {
    let session = sessions.get(index)?;
    let start = session.log_offset?;
    if start > file_len {
        return None;
    }
    let newer = index
        .checked_sub(1)
        .and_then(|i| sessions.get(i))
        .filter(|s| s.log_path == session.log_path);
    let end = match newer {
        None => None,
        Some(next) => match next.log_offset {
            Some(next_start) if next_start >= start => Some(next_start),
            _ => return None,
        },
    };
    Some(LogRange {
        path: session.log_path.clone(),
        start,
        end,
    })
}

fn file_len(path: &str) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

/// A lista da aba Logs, mais recente primeiro.
pub fn summarize_sessions(sessions: &[SessionRecord]) -> Vec<SessionSummary> {
    sessions
        .iter()
        .enumerate()
        .map(|(i, s)| SessionSummary {
            id: s.id.clone(),
            pid: s.pid,
            started_at: s.started_at,
            ended_at: s.ended_at,
            exit_code: s.exit_code,
            available: session_range(sessions, i, file_len(&s.log_path)).is_some(),
        })
        .collect()
}

fn locate(sessions: &[SessionRecord], id: &SessionId) -> Result<LogRange, TranscriptError> {
    let index = sessions
        .iter()
        .position(|s| &s.id == id)
        .ok_or_else(|| TranscriptError::SessionNotFound(id.clone()))?;
    let path = &sessions[index].log_path;
    session_range(sessions, index, file_len(path))
        .ok_or_else(|| TranscriptError::Unavailable(id.clone()))
}

/// Lê `[start, end)` do log de `chunk` em `chunk` bytes.
fn read_range(
    range: &LogRange,
    from: u64,
    mut each: impl FnMut(&[u8]) -> std::io::Result<()>,
) -> std::io::Result<u64> {
    let mut file = File::open(&range.path)?;
    let len = file.metadata()?.len();
    let end = range.end.unwrap_or(len).min(len);
    file.seek(SeekFrom::Start(from))?;
    let mut remaining = end.saturating_sub(from);
    let mut buffer = vec![0u8; 64 * 1024];
    while remaining > 0 {
        let want = usize::try_from(remaining.min(buffer.len() as u64)).unwrap_or(buffer.len());
        let read = file.read(&mut buffer[..want])?;
        if read == 0 {
            break;
        }
        each(&buffer[..read])?;
        remaining -= read as u64;
    }
    Ok(end.saturating_sub(range.start))
}

/// O final da transcrição de uma sessão, até [`TRANSCRIPT_VIEW_BYTES`] do log cru.
pub fn read_transcript(
    sessions: &[SessionRecord],
    id: &SessionId,
) -> Result<Transcript, TranscriptError> {
    let range = locate(sessions, id)?;
    let end = range.end.unwrap_or_else(|| file_len(&range.path));
    let from = end.saturating_sub(TRANSCRIPT_VIEW_BYTES).max(range.start);
    let truncated = from > range.start;
    let mut plain = PlainText::default();
    let bytes = read_range(&range, from, |chunk| {
        plain.feed(chunk);
        Ok(())
    })?;
    let mut text = plain.finish();
    if truncated {
        // Cortar no meio de uma linha (ou de um escape) deixa lixo no começo.
        if let Some(newline) = text.find('\n') {
            text.drain(..=newline);
        }
    }
    Ok(Transcript {
        session_id: id.clone(),
        text,
        truncated,
        bytes,
    })
}

/// Grava a transcrição inteira da sessão em `dest`, como texto. Devolve os bytes escritos.
pub fn export_transcript(
    sessions: &[SessionRecord],
    id: &SessionId,
    dest: &Path,
) -> Result<u64, TranscriptError> {
    if !dest.is_absolute() {
        return Err(TranscriptError::RelativePath(dest.display().to_string()));
    }
    let range = locate(sessions, id)?;
    let mut out = BufWriter::new(File::create(dest)?);
    let mut plain = PlainText::default();
    let mut written = 0u64;
    read_range(&range, range.start, |chunk| {
        plain.feed(chunk);
        let ready = plain.take_lines();
        written += ready.len() as u64;
        out.write_all(ready.as_bytes())
    })?;
    let rest = plain.finish();
    written += rest.len() as u64;
    out.write_all(rest.as_bytes())?;
    out.flush()?;
    Ok(written)
}

/// Converte saída crua de terminal em texto, aos pedaços: um escape ou um caractere
/// UTF-8 pode vir partido entre duas leituras.
#[derive(Debug, Default)]
pub struct PlainText {
    /// Bytes de um caractere UTF-8 incompleto no fim do último pedaço.
    pending: Vec<u8>,
    escape: Escape,
    line: Vec<char>,
    cursor: usize,
    /// `\r` visto e ainda não se sabe se vem `\n` (fim de linha) ou texto (sobrescrever).
    carriage: bool,
    done: String,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Escape {
    #[default]
    None,
    /// Logo após `ESC`.
    Start,
    /// `ESC [` ... até o byte final (0x40–0x7E).
    Csi,
    /// `ESC ]` ... até BEL ou `ESC \`.
    Osc,
    /// `ESC` dentro de um OSC: `\` termina.
    OscTerminator,
    /// `ESC (`, `ESC )` e afins: um caractere a mais.
    Charset,
}

impl PlainText {
    pub fn feed(&mut self, bytes: &[u8]) {
        let mut data = std::mem::take(&mut self.pending);
        data.extend_from_slice(bytes);
        let mut rest = data.as_slice();
        loop {
            match std::str::from_utf8(rest) {
                Ok(text) => {
                    self.chars(text);
                    break;
                }
                Err(error) => {
                    let (good, bad) = rest.split_at(error.valid_up_to());
                    // `valid_up_to` garante UTF-8 válido até ali.
                    self.chars(std::str::from_utf8(good).unwrap_or_default());
                    match error.error_len() {
                        // Incompleto no fim: espera o próximo pedaço.
                        None => {
                            self.pending = bad.to_vec();
                            break;
                        }
                        Some(len) => {
                            self.chars("\u{FFFD}");
                            rest = &bad[len..];
                        }
                    }
                }
            }
        }
    }

    fn chars(&mut self, text: &str) {
        for c in text.chars() {
            self.char(c);
        }
    }

    fn char(&mut self, c: char) {
        match self.escape {
            Escape::Start => {
                self.escape = match c {
                    '[' => Escape::Csi,
                    ']' => Escape::Osc,
                    '(' | ')' | '*' | '+' | '#' | '%' => Escape::Charset,
                    _ => Escape::None,
                };
                return;
            }
            Escape::Csi => {
                if ('\u{40}'..='\u{7e}').contains(&c) {
                    self.escape = Escape::None;
                }
                return;
            }
            Escape::Osc => {
                match c {
                    '\u{7}' => self.escape = Escape::None,
                    '\u{1b}' => self.escape = Escape::OscTerminator,
                    _ => {}
                }
                return;
            }
            Escape::OscTerminator => {
                self.escape = if c == '\\' { Escape::None } else { Escape::Osc };
                return;
            }
            Escape::Charset => {
                self.escape = Escape::None;
                return;
            }
            Escape::None => {}
        }

        if self.carriage && c != '\n' {
            // `\r` sozinho: volta ao começo e o que vier sobrescreve.
            self.carriage = false;
            self.cursor = 0;
        }
        match c {
            '\u{1b}' => self.escape = Escape::Start,
            '\r' => self.carriage = true,
            '\n' => {
                self.carriage = false;
                self.end_line();
            }
            '\u{8}' => self.cursor = self.cursor.saturating_sub(1),
            '\t' => self.put('\t'),
            c if c.is_control() => {}
            c => self.put(c),
        }
    }

    fn put(&mut self, c: char) {
        if self.cursor < self.line.len() {
            self.line[self.cursor] = c;
        } else {
            self.line.push(c);
        }
        self.cursor += 1;
    }

    fn end_line(&mut self) {
        let line: String = self.line.drain(..).collect();
        self.done.push_str(line.trim_end());
        self.done.push('\n');
        self.cursor = 0;
    }

    /// As linhas já fechadas, para quem grava aos pedaços.
    pub fn take_lines(&mut self) -> String {
        std::mem::take(&mut self.done)
    }

    /// Todo o texto, incluindo a última linha sem `\n`.
    pub fn finish(mut self) -> String {
        if !self.pending.is_empty() {
            self.chars("\u{FFFD}");
        }
        if !self.line.is_empty() {
            let line: String = self.line.drain(..).collect();
            self.done.push_str(line.trim_end());
        }
        self.done
    }
}

/// Atalho para um pedaço inteiro de uma vez.
pub fn plain_text(bytes: &[u8]) -> String {
    let mut plain = PlainText::default();
    plain.feed(bytes);
    plain.finish()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::ids::AgentId;

    #[test]
    fn tira_cores_e_titulos_e_junta_crlf() {
        let raw = b"\x1b[1;32mok\x1b[0m teste\r\n\x1b]0;titulo\x07depois\r\n";
        assert_eq!(plain_text(raw), "ok teste\ndepois\n");
    }

    #[test]
    fn barra_de_progresso_fica_so_no_estado_final() {
        let raw = b"baixando  10%\rbaixando  55%\rbaixando 100%\r\npronto";
        assert_eq!(plain_text(raw), "baixando 100%\npronto");
    }

    #[test]
    fn backspace_recua_e_sobrescreve() {
        assert_eq!(plain_text(b"lsx\x08 \x08\r\n"), "ls\n");
    }

    #[test]
    fn escape_e_utf8_partidos_entre_leituras() {
        let raw = "\x1b[31mação\x1b[0m\r\n".as_bytes();
        for cut in 0..raw.len() {
            let mut plain = PlainText::default();
            plain.feed(&raw[..cut]);
            plain.feed(&raw[cut..]);
            assert_eq!(plain.finish(), "ação\n", "cortado em {cut}");
        }
    }

    #[test]
    fn osc_terminado_por_st() {
        assert_eq!(
            plain_text(b"\x1b]8;;http://x\x1b\\link\x1b]8;;\x1b\\\n"),
            "link\n"
        );
    }

    fn record(id: &str, started_at: Millis, offset: Option<u64>, path: &str) -> SessionRecord {
        SessionRecord {
            id: SessionId::from_raw(id),
            agent_id: AgentId::from_raw("agt_1"),
            pid: Some(1),
            started_at,
            ended_at: None,
            exit_code: None,
            log_path: path.to_owned(),
            log_offset: offset,
        }
    }

    #[test]
    fn cada_sessao_vai_ate_onde_a_seguinte_comeca() {
        let s = [
            record("c", 3, Some(90), "a.log"),
            record("b", 2, Some(40), "a.log"),
            record("a", 1, Some(0), "a.log"),
        ];
        let range = |i| session_range(&s, i, 100).map(|r| (r.start, r.end));
        assert_eq!(range(0), Some((90, None)));
        assert_eq!(range(1), Some((40, Some(90))));
        assert_eq!(range(2), Some((0, Some(40))));
    }

    #[test]
    fn rotacao_e_sessoes_antigas_ficam_indisponiveis() {
        // "b" começou em 500; o log rotacionou e "c" começou do zero no arquivo novo.
        let s = [
            record("c", 3, Some(0), "a.log"),
            record("b", 2, Some(500), "a.log"),
            record("a", 1, None, "a.log"),
        ];
        assert!(session_range(&s, 0, 100).is_some());
        assert!(
            session_range(&s, 1, 100).is_none(),
            "trecho foi para o .1.log"
        );
        assert!(session_range(&s, 2, 100).is_none(), "sem offset gravado");
    }

    #[test]
    fn le_e_exporta_so_a_sessao_pedida() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("agt.log");
        let first = b"\x1b[32mprimeira\x1b[0m\r\n";
        let second = b"segunda sessao\r\nfim";
        let mut bytes = first.to_vec();
        bytes.extend_from_slice(second);
        std::fs::write(&log, &bytes).unwrap();
        let path = log.display().to_string();
        let sessions = [
            record("new", 2, Some(first.len() as u64), &path),
            record("old", 1, Some(0), &path),
        ];

        let old = read_transcript(&sessions, &SessionId::from_raw("old")).unwrap();
        assert_eq!(old.text, "primeira\n");
        assert!(!old.truncated);
        let new = read_transcript(&sessions, &SessionId::from_raw("new")).unwrap();
        assert_eq!(new.text, "segunda sessao\nfim");

        let out = dir.path().join("export.txt");
        export_transcript(&sessions, &SessionId::from_raw("new"), &out).unwrap();
        assert_eq!(
            std::fs::read_to_string(&out).unwrap(),
            "segunda sessao\nfim"
        );

        let summary = summarize_sessions(&sessions);
        assert!(summary.iter().all(|s| s.available));
    }

    #[test]
    fn exportacao_exige_caminho_absoluto_e_sessao_conhecida() {
        let sessions = [record("a", 1, Some(0), "/nao/existe.log")];
        let error = export_transcript(&sessions, &SessionId::from_raw("a"), Path::new("x.txt"))
            .unwrap_err();
        assert_eq!(error.code(), "invalid_path");
        let error = read_transcript(&sessions, &SessionId::from_raw("z")).unwrap_err();
        assert_eq!(error.code(), "session_not_found");
    }

    #[test]
    fn transcricao_grande_mostra_so_o_final_a_partir_de_uma_linha_inteira() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("agt.log");
        let line = "0123456789".repeat(10) + "\r\n";
        let lines = usize::try_from(TRANSCRIPT_VIEW_BYTES).unwrap() / line.len() + 50;
        std::fs::write(&log, line.repeat(lines)).unwrap();
        let sessions = [record("a", 1, Some(0), &log.display().to_string())];
        let t = read_transcript(&sessions, &SessionId::from_raw("a")).unwrap();
        assert!(t.truncated);
        assert!(
            t.text.lines().all(|l| l.len() == 100),
            "nenhuma linha cortada"
        );
        assert!(t.text.len() <= usize::try_from(TRANSCRIPT_VIEW_BYTES).unwrap());
    }
}
