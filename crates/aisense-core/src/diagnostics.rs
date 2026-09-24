//! "Exportar diagnóstico" (F09-05, `docs/11` → Privacidade).
//!
//! Monta o pacote que a pessoa anexa a um relato de bug: relatório (versões, SO,
//! preferências sem segredos, adaptadores) e o log do app, tudo **redigido** — valores
//! de segredos conhecidos, formatos comuns de chave de API, `AISENSE_TOKEN` e a pasta
//! home. A UI mostra exatamente estes arquivos antes de salvar, e o `.zip` gravado é
//! o mesmo que foi mostrado.
//!
//! O `.zip` é escrito à mão, sem compressão (método *stored*): são alguns arquivos de
//! texto, e isso evita uma dependência nova só para isto. Efeito colateral útil: o
//! texto aparece literal nos bytes do arquivo, então o teste de aceite procura os
//! segredos direto no pacote gerado.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;
use ts_rs::TS;

use crate::Millis;

/// O que entra no lugar de um segredo.
pub const REDACTED: &str = "‹redigido›";

/// Quanto do fim de cada log entra no pacote.
pub const LOG_TAIL_BYTES: u64 = 1024 * 1024;

/// Log do app, gravado pelo `aisense-app` a cada execução; a anterior vira `.1`.
pub const APP_LOG: &str = "aisense-app.log";

/// Segredos conhecidos mais curtos que isto não são procurados: mascarar "abc" em todo
/// lugar destruiria o log sem proteger nada.
const MIN_KNOWN_SECRET: usize = 6;

// Os `expect` abaixo são de padrões literais, compilados por todo teste deste módulo.

/// `NOME=valor` / `nome: valor` quando o nome diz que é segredo. Mantém o nome.
#[allow(clippy::expect_used)]
static ASSIGNMENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)\b([A-Z0-9_.-]*(?:TOKEN|API[_-]?KEY|SECRET|PASSWORD|PASSWD|PRIVATE[_-]?KEY|ACCESS[_-]?KEY|AUTH)[A-Z0-9_]*)(\s*[=:]\s*)(?:"[^"\r\n]*"|'[^'\r\n]*'|[^\s"',;&]+)"#,
    )
    .expect("valid regex")
});

/// `"apiKey": "valor"` em JSON. Mantém a chave.
#[allow(clippy::expect_used)]
static JSON_FIELD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)("[A-Z0-9_.-]*(?:token|api[_-]?key|secret|password|passwd|private[_-]?key|access[_-]?key)[A-Z0-9_]*"\s*:\s*)"(?:[^"\\]|\\.)*""#,
    )
    .expect("valid regex")
});

/// Formatos que se reconhecem sozinhos, sem nome ao lado.
#[allow(clippy::expect_used)]
static SHAPES: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        // Anthropic, OpenAI e afins.
        r"\bsk-[A-Za-z0-9_-]{16,}",
        // GitHub: tokens clássicos e finos.
        r"\bgh[pousr]_[A-Za-z0-9]{20,}",
        r"\bgithub_pat_[A-Za-z0-9_]{20,}",
        // Slack.
        r"\bxox[abprs]-[A-Za-z0-9-]{10,}",
        // AWS (id da chave) e Google.
        r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b",
        r"\bAIza[0-9A-Za-z_-]{30,}",
        // JWT.
        r"\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}",
        // Cabeçalho de autorização.
        r"(?i)\bbearer\s+[A-Za-z0-9._~+/=-]{8,}",
        // `AISENSE_TOKEN` (64 hex, `supervisor::token`) e hashes do mesmo tamanho.
        r"\b[0-9a-fA-F]{64}\b",
    ]
    .iter()
    .map(|p| Regex::new(p).expect("valid regex"))
    .collect()
});

/// Mascara segredos e a pasta home num texto.
#[derive(Debug, Clone, Default)]
pub struct Redactor {
    /// Valores exatos (do keychain), do mais longo para o mais curto.
    known: Vec<String>,
    /// A pasta home e a mesma com as barras escapadas como no JSON (Windows).
    homes: Vec<String>,
}

impl Redactor {
    pub fn new(home: Option<&Path>, known_secrets: impl IntoIterator<Item = String>) -> Self {
        let mut known: Vec<String> = known_secrets
            .into_iter()
            .filter(|s| s.chars().count() >= MIN_KNOWN_SECRET)
            .collect();
        known.sort_by_key(|s| std::cmp::Reverse(s.len()));
        known.dedup();
        let mut homes = Vec::new();
        if let Some(home) = home.map(|h| h.display().to_string()) {
            let home = home.trim_end_matches(['/', '\\']).to_owned();
            // Uma home de um caractere ("/") viraria "~" em todo caminho.
            if home.len() > 1 {
                let escaped = home.replace('\\', "\\\\");
                if escaped != home {
                    homes.push(escaped);
                }
                homes.push(home);
            }
        }
        Self { known, homes }
    }

    /// A pasta home do usuário atual.
    pub fn home_dir() -> Option<PathBuf> {
        ["HOME", "USERPROFILE"]
            .iter()
            .find_map(|k| std::env::var_os(k).filter(|v| !v.is_empty()))
            .map(PathBuf::from)
    }

    pub fn redact(&self, text: &str) -> String {
        let mut out = text.to_owned();
        for secret in &self.known {
            if out.contains(secret.as_str()) {
                out = out.replace(secret.as_str(), REDACTED);
            }
        }
        // Formatos antes dos nomes: em `Authorization: Bearer x`, mascarar só a palavra
        // depois do nome deixaria o `x` para trás.
        for shape in SHAPES.iter() {
            out = shape.replace_all(&out, REDACTED).into_owned();
        }
        out = JSON_FIELD
            .replace_all(&out, format!("${{1}}\"{REDACTED}\""))
            .into_owned();
        out = ASSIGNMENT
            .replace_all(&out, format!("${{1}}${{2}}{REDACTED}"))
            .into_owned();
        for home in &self.homes {
            out = out.replace(home.as_str(), "~");
        }
        out
    }
}

/// Um arquivo do pacote, já redigido: é isto que a prévia mostra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct DiagnosticFile {
    pub name: String,
    pub content: String,
    /// Só o fim do arquivo original entrou.
    pub truncated: bool,
}

/// O pacote inteiro, como a prévia mostra e o `.zip` grava.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct DiagnosticBundle {
    pub files: Vec<DiagnosticFile>,
    /// O que ficou de fora de propósito, em texto para a pessoa ler.
    pub left_out: Vec<String>,
}

impl DiagnosticBundle {
    /// Acrescenta um texto, redigido.
    pub fn add_text(&mut self, redactor: &Redactor, name: &str, content: &str) {
        self.files.push(DiagnosticFile {
            name: name.to_owned(),
            content: redactor.redact(content),
            truncated: false,
        });
    }

    /// Acrescenta o fim de um log (até `max_bytes`), redigido. Arquivo ausente não
    /// entra; ilegível entra como uma linha dizendo o porquê.
    pub fn add_log_tail(&mut self, redactor: &Redactor, name: &str, path: &Path, max_bytes: u64) {
        let (content, truncated) = match read_tail(path, max_bytes) {
            Ok(Some(tail)) => tail,
            Ok(None) => return,
            Err(error) => (
                format!("(não foi possível ler {}: {error})", path.display()),
                false,
            ),
        };
        self.files.push(DiagnosticFile {
            name: name.to_owned(),
            content: redactor.redact(&content),
            truncated,
        });
    }

    /// O `.zip` em memória.
    pub fn to_zip(&self, modified_ms: Millis) -> Vec<u8> {
        let (time, date) = dos_datetime(modified_ms);
        let mut out = Vec::new();
        let mut central = Vec::new();
        for file in &self.files {
            let name = file.name.as_bytes();
            let data = file.content.as_bytes();
            let crc = crc32(data);
            let offset = out.len() as u32;
            let size = data.len() as u32;
            // Cabeçalho local.
            put32(&mut out, 0x0403_4b50);
            put16(&mut out, 20); // versão necessária
            put16(&mut out, 0x0800); // nomes em UTF-8
            put16(&mut out, 0); // sem compressão
            put16(&mut out, time);
            put16(&mut out, date);
            put32(&mut out, crc);
            put32(&mut out, size);
            put32(&mut out, size);
            put16(&mut out, name.len() as u16);
            put16(&mut out, 0);
            out.extend_from_slice(name);
            out.extend_from_slice(data);
            // Entrada do diretório central.
            put32(&mut central, 0x0201_4b50);
            put16(&mut central, 20); // feito por
            put16(&mut central, 20);
            put16(&mut central, 0x0800);
            put16(&mut central, 0);
            put16(&mut central, time);
            put16(&mut central, date);
            put32(&mut central, crc);
            put32(&mut central, size);
            put32(&mut central, size);
            put16(&mut central, name.len() as u16);
            put16(&mut central, 0); // extra
            put16(&mut central, 0); // comentário
            put16(&mut central, 0); // disco
            put16(&mut central, 0); // atributos internos
            put32(&mut central, 0); // atributos externos
            put32(&mut central, offset);
            central.extend_from_slice(name);
        }
        let central_offset = out.len() as u32;
        let central_size = central.len() as u32;
        out.extend_from_slice(&central);
        let count = self.files.len() as u16;
        put32(&mut out, 0x0605_4b50);
        put16(&mut out, 0);
        put16(&mut out, 0);
        put16(&mut out, count);
        put16(&mut out, count);
        put32(&mut out, central_size);
        put32(&mut out, central_offset);
        put16(&mut out, 0);
        out
    }

    /// Grava o `.zip` em `path` (temporário + rename: nunca um pacote pela metade).
    /// Devolve o tamanho gravado.
    pub fn write_zip(&self, path: &Path, modified_ms: Millis) -> std::io::Result<u64> {
        let bytes = self.to_zip(modified_ms);
        let mut tmp = path.as_os_str().to_os_string();
        tmp.push(".tmp");
        let tmp = PathBuf::from(tmp);
        std::fs::write(&tmp, &bytes)?;
        std::fs::rename(&tmp, path).inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp);
        })?;
        Ok(bytes.len() as u64)
    }
}

/// Nome sugerido para o pacote.
pub fn bundle_file_name(now_ms: Millis) -> String {
    let (y, m, d) = civil_date(now_ms);
    format!("aisense-diagnostico-{y:04}-{m:02}-{d:02}.zip")
}

/// Renomeia o log da execução anterior para `.1` (substituindo o mais antigo). Chamado
/// uma vez na subida, antes de abrir o log novo.
pub fn rotate_app_log(dir: &Path) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let current = dir.join(APP_LOG);
    if current.exists() {
        std::fs::rename(&current, dir.join(format!("{APP_LOG}.1")))?;
    }
    Ok(current)
}

/// Até `max` bytes do fim do arquivo, começando numa linha inteira quando corta.
fn read_tail(path: &Path, max: u64) -> std::io::Result<Option<(String, bool)>> {
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let len = file.metadata()?.len();
    let truncated = len > max;
    if truncated {
        file.seek(SeekFrom::Start(len - max))?;
    }
    let mut bytes = Vec::new();
    file.take(max).read_to_end(&mut bytes)?;
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    if truncated {
        if let Some(newline) = text.find('\n') {
            text.drain(..=newline);
        }
    }
    Ok(Some((text, truncated)))
}

fn put16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

/// CRC-32 (IEEE), o do formato zip.
fn crc32(data: &[u8]) -> u32 {
    static TABLE: LazyLock<[u32; 256]> = LazyLock::new(|| {
        let mut table = [0u32; 256];
        for (i, slot) in table.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 {
                c = if c & 1 == 1 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
            *slot = c;
        }
        table
    });
    !data.iter().fold(!0u32, |crc, &b| {
        TABLE[((crc ^ u32::from(b)) & 0xFF) as usize] ^ (crc >> 8)
    })
}

/// Data (ano, mês, dia) em UTC de um instante em ms.
fn civil_date(ms: Millis) -> (i64, u32, u32) {
    // Algoritmo "days from civil" invertido (Howard Hinnant).
    let days = ms.div_euclid(86_400_000) + 719_468;
    let era = days.div_euclid(146_097);
    let doe = days.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// Hora e data no formato do MS-DOS que o zip usa (UTC; 1980 é o mínimo).
fn dos_datetime(ms: Millis) -> (u16, u16) {
    let (y, m, d) = civil_date(ms);
    if y < 1980 {
        return (0, (1 << 5) | 1);
    }
    let secs = ms.div_euclid(1000).rem_euclid(86_400);
    let time = ((secs / 3600) << 11) | (((secs / 60) % 60) << 5) | ((secs % 60) / 2);
    let date = (((y - 1980) as u32) << 9) | (m << 5) | d;
    (time as u16, date as u16)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    /// Lê de volta um zip *stored*: (nome, conteúdo) pelo diretório central.
    fn unzip(bytes: &[u8]) -> Vec<(String, String)> {
        let u16_at = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]) as usize;
        let u32_at = |i: usize| {
            u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]) as usize
        };
        let end = bytes.len() - 22;
        assert_eq!(u32_at(end), 0x0605_4b50, "end of central directory");
        let count = u16_at(end + 10);
        let mut at = u32_at(end + 16);
        let mut files = Vec::new();
        for _ in 0..count {
            assert_eq!(u32_at(at), 0x0201_4b50);
            let crc = u32_at(at + 16) as u32;
            let size = u32_at(at + 24);
            let name_len = u16_at(at + 28);
            let offset = u32_at(at + 42);
            let name = String::from_utf8(bytes[at + 46..at + 46 + name_len].to_vec()).unwrap();
            assert_eq!(u32_at(offset), 0x0403_4b50);
            let data_at = offset + 30 + u16_at(offset + 26) + u16_at(offset + 28);
            let data = &bytes[data_at..data_at + size];
            assert_eq!(crc32(data), crc, "crc of {name}");
            files.push((name, String::from_utf8(data.to_vec()).unwrap()));
            at += 46 + name_len;
        }
        files
    }

    #[test]
    fn crc32_matches_the_reference_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn dates_convert_to_civil_and_dos() {
        // 2026-09-24 13:45:30 UTC
        let ms = 1_790_257_530_000;
        assert_eq!(civil_date(ms), (2026, 9, 24));
        let (time, date) = dos_datetime(ms);
        assert_eq!(date, ((2026 - 1980) << 9) | (9 << 5) | 24);
        assert_eq!(time, (13 << 11) | (45 << 5) | 15);
        assert_eq!(bundle_file_name(ms), "aisense-diagnostico-2026-09-24.zip");
        assert_eq!(civil_date(0), (1970, 1, 1));
        assert_eq!(civil_date(951_782_400_000), (2000, 2, 29));
    }

    #[test]
    fn redacts_names_that_say_secret_and_keeps_the_name() {
        let r = Redactor::default();
        assert_eq!(
            r.redact("ANTHROPIC_API_KEY=abc123xyz rest"),
            format!("ANTHROPIC_API_KEY={REDACTED} rest")
        );
        assert_eq!(
            r.redact("AISENSE_TOKEN: \"quoted value\""),
            format!("AISENSE_TOKEN: {REDACTED}")
        );
        assert_eq!(
            r.redact(r#"{"apiKey": "v\"x", "name": "ok"}"#),
            format!(r#"{{"apiKey": "{REDACTED}", "name": "ok"}}"#)
        );
        // Nome do segredo sem valor, como as preferências guardam, fica.
        assert_eq!(
            r.redact(r#"{"envName": "OPENAI_API_KEY", "masked": "sk-…abcd"}"#),
            r#"{"envName": "OPENAI_API_KEY", "masked": "sk-…abcd"}"#
        );
    }

    #[test]
    fn replaces_the_home_folder_with_a_tilde() {
        let r = Redactor::new(Some(Path::new("/home/maria/")), []);
        assert_eq!(
            r.redact("db at /home/maria/.aisense/aisense.db"),
            "db at ~/.aisense/aisense.db"
        );
        let w = Redactor::new(Some(Path::new(r"C:\Users\Maria")), []);
        assert_eq!(
            w.redact(r#"{"dataDir": "C:\\Users\\Maria\\AppData"} C:\Users\Maria\x"#),
            r#"{"dataDir": "~\\AppData"} ~\x"#
        );
        // Uma home "/" não apaga todas as barras.
        let root = Redactor::new(Some(Path::new("/")), []);
        assert_eq!(root.redact("/usr/bin"), "/usr/bin");
    }

    #[test]
    fn reads_only_the_tail_starting_at_a_whole_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.log");
        std::fs::write(&path, "first line\nsecond line\nthird\n").unwrap();
        let (tail, truncated) = read_tail(&path, 15).unwrap().unwrap();
        assert!(truncated);
        assert_eq!(tail, "third\n");
        let (all, truncated) = read_tail(&path, 1000).unwrap().unwrap();
        assert!(!truncated);
        assert_eq!(all, "first line\nsecond line\nthird\n");
        assert!(read_tail(&dir.path().join("missing"), 10)
            .unwrap()
            .is_none());
    }

    #[test]
    fn rotation_keeps_one_previous_run() {
        let dir = tempfile::tempdir().unwrap();
        let logs = dir.path().join("logs");
        let current = rotate_app_log(&logs).unwrap();
        std::fs::write(&current, "run 1").unwrap();
        rotate_app_log(&logs).unwrap();
        std::fs::write(&current, "run 2").unwrap();
        rotate_app_log(&logs).unwrap();
        let previous = std::fs::read_to_string(logs.join(format!("{APP_LOG}.1"))).unwrap();
        assert_eq!(previous, "run 2");
        assert!(!current.exists());
    }

    /// Aceite da F09-05: nenhum token ou chave aparece no pacote gerado.
    #[test]
    fn no_token_or_key_reaches_the_generated_package() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home").join("maria");
        let keychain_value = "minha-senha-do-keychain-9f8e7d";
        let bus_token = "a3f1".repeat(16);
        let secrets = [
            keychain_value.to_owned(),
            bus_token.clone(),
            "sk-ant-api03-AbCdEfGhIjKlMnOpQrStUvWxYz0123456789".to_owned(),
            "sk-proj-abcdefghijklmnopqrstuvwx".to_owned(),
            "ghp_0123456789abcdefghijABCDEFGHIJ".to_owned(),
            "github_pat_11ABCDEFG0123456789_abcdefghijklmnop".to_owned(),
            "xoxb-1234567890-abcdefghij".to_owned(),
            "AKIAIOSFODNN7EXAMPLE".to_owned(),
            "AIzaSyA1234567890abcdefghijklmnopqrstuv".to_owned(),
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U".to_owned(),
            "senha-no-env-qwe".to_owned(),
            "bearer-valor-zzz".to_owned(),
        ];
        let log = format!(
            "INFO aisense: iniciando dir={home}/.aisense\n\
             DEBUG supervisor: env AISENSE_TOKEN={bus_token} agent=agt_1\n\
             WARN runtime: claude saiu: Invalid API key {s2}\n\
             ERROR codex: 401 with key {s3} in /home/other/file\n\
             INFO git: remote https://x:{s4}@github.com/a/b\n\
             INFO pat {s5} slack {s6}\n\
             INFO aws {s7} google {s8}\n\
             INFO jwt {s9}\n\
             INFO DB_PASSWORD={s10} ok\n\
             INFO Authorization: Bearer {s11}\n\
             INFO keychain devolveu {kc}\n\
             INFO fim da execução\n",
            home = home.display(),
            s2 = secrets[2],
            s3 = secrets[3],
            s4 = secrets[4],
            s5 = secrets[5],
            s6 = secrets[6],
            s7 = secrets[7],
            s8 = secrets[8],
            s9 = secrets[9],
            s10 = secrets[10],
            s11 = secrets[11],
            kc = keychain_value,
        );
        let log_path = dir.path().join(APP_LOG);
        std::fs::write(&log_path, &log).unwrap();
        let report = serde_json::json!({
            "dataDir": format!("{}/.aisense", home.display()),
            "settings": { "secrets": [], "apiKey": keychain_value },
        });

        let redactor = Redactor::new(Some(&home), [keychain_value.to_owned()]);
        let mut bundle = DiagnosticBundle::default();
        bundle.add_text(
            &redactor,
            "relatorio.json",
            &serde_json::to_string_pretty(&report).unwrap(),
        );
        bundle.add_log_tail(&redactor, APP_LOG, &log_path, LOG_TAIL_BYTES);
        let zip_path = dir.path().join("pacote.zip");
        bundle.write_zip(&zip_path, 1_790_257_530_000).unwrap();

        let bytes = std::fs::read(&zip_path).unwrap();
        let raw = String::from_utf8_lossy(&bytes);
        for secret in &secrets {
            assert!(
                !raw.contains(secret.as_str()),
                "{secret} leaked into the zip"
            );
        }
        assert!(
            !raw.contains(&home.display().to_string()),
            "home folder leaked"
        );
        // O resto do log chegou, e o zip se lê de volta.
        let files = unzip(&bytes);
        let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["relatorio.json", APP_LOG]);
        let app_log = &files[1].1;
        for kept in [
            "iniciando dir=~/.aisense",
            "agent=agt_1",
            "Invalid API key",
            "in /home/other/file",
            "fim da execução",
        ] {
            assert!(app_log.contains(kept), "{kept:?} missing from:\n{app_log}");
        }
        assert!(
            files[0].1.contains("\"dataDir\": \"~/.aisense\""),
            "{}",
            files[0].1
        );
        // O que a prévia mostra é o que foi gravado.
        assert_eq!(files[1].1, bundle.files[1].content);
    }
}
