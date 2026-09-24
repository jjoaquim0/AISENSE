//! Notas da equipe (`docs/15`, F04-09): Markdown em `<workdir>/.aisense/notes/<slug>.md`,
//! a memória compartilhada que sobrevive aos reinícios dos agentes.
//!
//! Escrita concorrente é o caso normal. Duas garantias:
//! - `append` abre com `O_APPEND` e escreve **uma vez só**: dois agentes anexando ao mesmo
//!   tempo nunca perdem conteúdo um do outro.
//! - `write` substitui o arquivo, mas exige o hash lido: se a nota mudou desde a leitura,
//!   falha com `stale_note` e devolve o diff. Sem hash, só cria nota nova.

use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::skill::AISENSE_DIR;
use crate::time::Millis;

pub const NOTES_DIR: &str = "notes";
const EXT: &str = "md";
/// Acima disso é documento, não nota (`docs/15`, "Limites").
pub const NOTE_MAX_BYTES: usize = 256 * 1024;
pub const NOTES_MAX: usize = 200;
/// Linhas do índice no `BOOT.md`.
pub const NOTES_BOOT_INDEX: usize = 20;
const SLUG_MAX: usize = 64;
const TMP_PREFIX: &str = ".tmp-";

/// Uma linha da lista de notas e do índice do `BOOT.md` — sem o conteúdo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct NoteSummary {
    pub slug: String,
    /// O primeiro `# ` do arquivo; sem ele, o slug.
    pub title: String,
    /// Epoch ms da última modificação no disco.
    #[ts(type = "number")]
    pub updated_at: Millis,
    #[ts(type = "number")]
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct Note {
    pub slug: String,
    pub title: String,
    pub content: String,
    /// O que `write` exige de volta para substituir a nota.
    pub hash: String,
}

/// Uma ocorrência de `search`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub struct NoteMatch {
    pub slug: String,
    pub line: u32,
    pub text: String,
}

/// Uma linha do diff de `stale_note`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", content = "text", rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub enum DiffLine {
    Same(String),
    /// Está no disco e sairia com a sua versão.
    Removed(String),
    /// Está na sua versão e não no disco.
    Added(String),
}

/// Resultado de salvar pela UI: a trava otimista não é erro para quem edita — é a hora
/// de mostrar o diff e decidir.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/desktop/src/types/generated/")]
pub enum NoteSave {
    Saved {
        note: Note,
    },
    /// Alguém mudou a nota depois da sua leitura. `diff` vai do disco para a sua versão.
    #[serde(rename_all = "camelCase")]
    Stale {
        current_hash: String,
        diff: Vec<DiffLine>,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum NoteError {
    #[error("invalid note name {0:?}: use lowercase letters, digits and hyphens")]
    InvalidSlug(String),
    #[error("note {0:?} does not exist")]
    NotFound(String),
    #[error("note {0:?} already exists")]
    Exists(String),
    #[error("note {slug:?} changed since you read it")]
    Stale {
        slug: String,
        current_hash: String,
        diff: Vec<DiffLine>,
    },
    #[error("note {0:?} already exists: pass the hash you read to replace it")]
    HashRequired(String),
    #[error("a note is limited to {} KB", NOTE_MAX_BYTES / 1024)]
    TooLarge,
    #[error("a team is limited to {NOTES_MAX} notes")]
    TooMany,
    #[error("section {0:?} not found")]
    SectionNotFound(String),
    #[error("could not access {path}: {source}")]
    Io { path: String, source: io::Error },
}

impl NoteError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidSlug(_) => "invalid_note",
            Self::NotFound(_) => "note_not_found",
            Self::Exists(_) => "note_exists",
            Self::Stale { .. } => "stale_note",
            Self::HashRequired(_) => "note_hash_required",
            Self::TooLarge => "note_too_large",
            Self::TooMany => "too_many_notes",
            Self::SectionNotFound(_) => "note_section_not_found",
            Self::Io { .. } => "note_io",
        }
    }

    pub fn to_command_error(&self) -> crate::CommandError {
        crate::CommandError::new(self.code(), self.to_string(), self.hint())
    }

    pub fn hint(&self) -> Option<String> {
        match self {
            Self::Stale { .. } => Some(
                "Releia a nota, junte as mudanças e grave de novo — ou use append, que nunca conflita."
                    .into(),
            ),
            Self::NotFound(slug) => Some(format!("Crie com: aisense notes new {slug}")),
            Self::HashRequired(_) => Some("Leia a nota antes e mande o hash dela.".into()),
            _ => None,
        }
    }
}

fn io_err(path: &Path) -> impl FnOnce(io::Error) -> NoteError + '_ {
    move |source| NoteError::Io {
        path: path.display().to_string(),
        source,
    }
}

pub type NoteResult<T> = Result<T, NoteError>;

/// `<workdir>/.aisense/notes/`.
pub fn notes_dir(workdir: &Path) -> PathBuf {
    workdir.join(AISENSE_DIR).join(NOTES_DIR)
}

/// `^[a-z0-9][a-z0-9-]*$`, até 64: vira nome de arquivo sem surpresa em nenhum SO.
pub fn validate_slug(slug: &str) -> NoteResult<()> {
    let ok = !slug.is_empty()
        && slug.len() <= SLUG_MAX
        && slug.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        && slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if ok {
        Ok(())
    } else {
        Err(NoteError::InvalidSlug(slug.to_owned()))
    }
}

/// Hash do conteúdo (FNV-1a 64): trava otimista, não segurança. Sem dependência nova.
pub fn content_hash(content: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in content.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// O primeiro `# Título` do Markdown.
pub fn note_title(content: &str) -> Option<String> {
    content
        .lines()
        .find_map(|l| l.strip_prefix("# "))
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty())
}

/// Um trava por pasta de notas: `write` lê, compara o hash e troca o arquivo sem que um
/// `append` do mesmo processo entre no meio. Entre processos vale o `O_APPEND` e o rename.
fn dir_lock(dir: &Path) -> Arc<Mutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();
    let mut locks = LOCKS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    Arc::clone(locks.entry(dir.to_path_buf()).or_default())
}

/// As notas de uma equipe.
#[derive(Debug, Clone)]
pub struct TeamNotes {
    dir: PathBuf,
}

impl TeamNotes {
    pub fn new(workdir: &Path) -> Self {
        Self {
            dir: notes_dir(workdir),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn path(&self, slug: &str) -> NoteResult<PathBuf> {
        validate_slug(slug)?;
        Ok(self.dir.join(format!("{slug}.{EXT}")))
    }

    fn read_file(&self, slug: &str) -> NoteResult<Option<String>> {
        let path = self.path(slug)?;
        match fs::read_to_string(&path) {
            Ok(content) => Ok(Some(content)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(io_err(&path)(e)),
        }
    }

    /// Mais recente primeiro. Pasta inexistente = nenhuma nota.
    pub fn list(&self) -> NoteResult<Vec<NoteSummary>> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(io_err(&self.dir)(e)),
        };
        let mut notes = Vec::new();
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            let Some(slug) = path
                .file_stem()
                .and_then(|s| s.to_str())
                .filter(|_| path.extension().is_some_and(|e| e == EXT))
            else {
                continue;
            };
            if validate_slug(slug).is_err() {
                continue;
            }
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if !meta.is_file() {
                continue;
            }
            let content = fs::read_to_string(&path).unwrap_or_default();
            notes.push(NoteSummary {
                slug: slug.to_owned(),
                title: note_title(&content).unwrap_or_else(|| slug.to_owned()),
                updated_at: modified_ms(&meta),
                bytes: meta.len(),
            });
        }
        notes.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then_with(|| a.slug.cmp(&b.slug))
        });
        Ok(notes)
    }

    pub fn read(&self, slug: &str) -> NoteResult<Note> {
        let content = self
            .read_file(slug)?
            .ok_or_else(|| NoteError::NotFound(slug.to_owned()))?;
        Ok(Note {
            slug: slug.to_owned(),
            title: note_title(&content).unwrap_or_else(|| slug.to_owned()),
            hash: content_hash(&content),
            content,
        })
    }

    /// Só a seção com esse título (`## Autenticação`), até o próximo título do mesmo
    /// nível ou acima. Casa sem caixa.
    pub fn read_section(&self, slug: &str, heading: &str) -> NoteResult<String> {
        let note = self.read(slug)?;
        section(&note.content, heading).ok_or_else(|| NoteError::SectionNotFound(heading.into()))
    }

    /// Cria com `# título`. Recusa nota que já existe.
    pub fn create(&self, slug: &str, title: &str) -> NoteResult<Note> {
        let title = if title.trim().is_empty() {
            slug
        } else {
            title.trim()
        };
        self.write(slug, &format!("# {title}\n"), None)
    }

    /// Anexa um bloco ao fim, numa escrita só com `O_APPEND` — atômico entre agentes.
    /// Garante a quebra de linha no fim do bloco.
    pub fn append(&self, slug: &str, text: &str) -> NoteResult<Note> {
        let path = self.path(slug)?;
        let mut block = text.trim_end_matches(['\r', '\n']).to_owned();
        block.push('\n');
        let lock = dir_lock(&self.dir);
        let _guard = lock.lock().unwrap_or_else(PoisonError::into_inner);
        let meta = fs::metadata(&path).map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => NoteError::NotFound(slug.to_owned()),
            _ => io_err(&path)(e),
        })?;
        if meta.len() as usize + block.len() > NOTE_MAX_BYTES {
            return Err(NoteError::TooLarge);
        }
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .map_err(io_err(&path))?;
        file.write_all(block.as_bytes()).map_err(io_err(&path))?;
        drop(file);
        self.read(slug)
    }

    /// Substitui a nota. Existe → `expect_hash` tem de ser o do conteúdo atual; senão
    /// `Stale` com o diff (disco → sua versão). Não existe → só sem `expect_hash`.
    pub fn write(&self, slug: &str, content: &str, expect_hash: Option<&str>) -> NoteResult<Note> {
        let path = self.path(slug)?;
        if content.len() > NOTE_MAX_BYTES {
            return Err(NoteError::TooLarge);
        }
        let lock = dir_lock(&self.dir);
        let _guard = lock.lock().unwrap_or_else(PoisonError::into_inner);
        match (self.read_file(slug)?, expect_hash) {
            (Some(current), Some(expected)) => {
                let current_hash = content_hash(&current);
                if current_hash != expected {
                    return Err(NoteError::Stale {
                        slug: slug.to_owned(),
                        current_hash,
                        diff: line_diff(&current, content),
                    });
                }
            }
            (Some(_), None) => return Err(NoteError::HashRequired(slug.to_owned())),
            (None, Some(_)) => return Err(NoteError::NotFound(slug.to_owned())),
            (None, None) => {
                if self.list()?.len() >= NOTES_MAX {
                    return Err(NoteError::TooMany);
                }
            }
        }
        fs::create_dir_all(&self.dir).map_err(io_err(&self.dir))?;
        let tmp = self
            .dir
            .join(format!("{TMP_PREFIX}{slug}-{}", ulid::Ulid::new()));
        fs::write(&tmp, content).map_err(io_err(&tmp))?;
        if let Err(e) = fs::rename(&tmp, &path) {
            let _ = fs::remove_file(&tmp);
            return Err(io_err(&path)(e));
        }
        Ok(Note {
            slug: slug.to_owned(),
            title: note_title(content).unwrap_or_else(|| slug.to_owned()),
            hash: content_hash(content),
            content: content.to_owned(),
        })
    }

    /// `write` para a UI: o conflito volta como [`NoteSave::Stale`], não como erro.
    pub fn save(
        &self,
        slug: &str,
        content: &str,
        expect_hash: Option<&str>,
    ) -> NoteResult<NoteSave> {
        match self.write(slug, content, expect_hash) {
            Ok(note) => Ok(NoteSave::Saved { note }),
            Err(NoteError::Stale {
                current_hash, diff, ..
            }) => Ok(NoteSave::Stale { current_hash, diff }),
            Err(error) => Err(error),
        }
    }

    /// Apaga a nota (pela UI; os agentes não têm este comando).
    pub fn delete(&self, slug: &str) -> NoteResult<()> {
        let path = self.path(slug)?;
        fs::remove_file(&path).map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => NoteError::NotFound(slug.to_owned()),
            _ => io_err(&path)(e),
        })
    }

    /// Busca literal, sem caixa, com a linha de cada ocorrência.
    pub fn search(&self, query: &str) -> NoteResult<Vec<NoteMatch>> {
        let needle = query.to_lowercase();
        if needle.trim().is_empty() {
            return Ok(Vec::new());
        }
        let mut found = Vec::new();
        let mut notes = self.list()?;
        notes.sort_by(|a, b| a.slug.cmp(&b.slug));
        for summary in notes {
            let Some(content) = self.read_file(&summary.slug)? else {
                continue;
            };
            for (i, line) in content.lines().enumerate() {
                if line.to_lowercase().contains(&needle) {
                    found.push(NoteMatch {
                        slug: summary.slug.clone(),
                        line: u32::try_from(i + 1).unwrap_or(u32::MAX),
                        text: line.trim().to_owned(),
                    });
                }
            }
        }
        Ok(found)
    }
}

fn modified_ms(meta: &fs::Metadata) -> Millis {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| {
            Millis::try_from(d.as_millis()).unwrap_or(Millis::MAX)
        })
}

fn heading_level(line: &str) -> Option<(usize, &str)> {
    let level = line.chars().take_while(|c| *c == '#').count();
    let rest = line.get(level..)?;
    (level > 0 && rest.starts_with(' ')).then(|| (level, rest.trim()))
}

fn section(content: &str, heading: &str) -> Option<String> {
    let wanted = heading.trim().trim_start_matches('#').trim().to_lowercase();
    let lines: Vec<&str> = content.lines().collect();
    let (start, level) = lines.iter().enumerate().find_map(|(i, l)| {
        heading_level(l)
            .filter(|(_, text)| text.to_lowercase() == wanted)
            .map(|(level, _)| (i, level))
    })?;
    let end = lines[start + 1..]
        .iter()
        .position(|l| heading_level(l).is_some_and(|(lv, _)| lv <= level))
        .map_or(lines.len(), |p| start + 1 + p);
    Some(lines[start..end].join("\n").trim_end().to_owned() + "\n")
}

/// Diff por linhas: prefixo e sufixo comuns, o miolo como removido/adicionado. Não é o
/// menor diff possível, mas é correto e linear — nota de 256 KB não trava a UI.
pub fn line_diff(before: &str, after: &str) -> Vec<DiffLine> {
    let a: Vec<&str> = before.lines().collect();
    let b: Vec<&str> = after.lines().collect();
    let prefix = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let mut diff = Vec::new();
    diff.extend(a[..prefix].iter().map(|l| DiffLine::Same((*l).to_owned())));
    diff.extend(
        a[prefix..a.len() - suffix]
            .iter()
            .map(|l| DiffLine::Removed((*l).to_owned())),
    );
    diff.extend(
        b[prefix..b.len() - suffix]
            .iter()
            .map(|l| DiffLine::Added((*l).to_owned())),
    );
    diff.extend(
        a[a.len() - suffix..]
            .iter()
            .map(|l| DiffLine::Same((*l).to_owned())),
    );
    diff
}

/// "há 5 min", "há 2 h", "há 3 dias" — o índice do `BOOT.md` é texto, não interface.
pub fn relative_age(then: Millis, now: Millis) -> String {
    let minutes = now.saturating_sub(then) / 60_000;
    match minutes {
        0 => "agora".into(),
        1..=59 => format!("há {minutes} min"),
        60..=1439 => format!("há {} h", minutes / 60),
        _ => format!("há {} dias", minutes / 1440),
    }
}

/// A seção "Memória da equipe" do `BOOT.md`: as 20 notas mais recentes, sem conteúdo.
/// Vazia quando a equipe não tem notas.
pub fn boot_index(notes: &[NoteSummary], now: Millis, max: usize) -> String {
    if notes.is_empty() {
        return String::new();
    }
    let mut out = String::from(
        "\n## Memória da equipe\nEstas notas são a fonte da verdade compartilhada. Leia a que for \
         relevante antes de decidir algo.\n\n| Nota | Assunto | Atualizada |\n|---|---|---|\n",
    );
    for note in notes.iter().take(max) {
        let title: String = note.title.chars().take(80).collect();
        out.push_str(&format!(
            "| {} | {} | {} |\n",
            note.slug,
            title.replace('|', "\\|"),
            relative_age(note.updated_at, now)
        ));
    }
    if notes.len() > max {
        out.push_str(&format!(
            "\nMais {} nota(s): `aisense notes list`.\n",
            notes.len() - max
        ));
    }
    out.push_str("\nLeia com: `aisense notes read <slug>`\n");
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn notes() -> (tempfile::TempDir, TeamNotes) {
        let dir = tempfile::tempdir().unwrap();
        let notes = TeamNotes::new(dir.path());
        (dir, notes)
    }

    #[test]
    fn cria_le_lista_e_busca() {
        let (_dir, notes) = notes();
        assert!(notes.list().unwrap().is_empty(), "sem pasta = sem notas");
        notes.create("contratos-api", "Contratos da API").unwrap();
        notes
            .append(
                "contratos-api",
                "## Autenticação\nPOST /auth/token devolve refresh_token",
            )
            .unwrap();
        notes
            .append("contratos-api", "## Usuários\nlegacy_id saiu na v2\n")
            .unwrap();

        let list = notes.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "Contratos da API");
        let note = notes.read("contratos-api").unwrap();
        assert!(note.content.ends_with("legacy_id saiu na v2\n"));
        assert_eq!(note.hash, content_hash(&note.content));

        assert_eq!(
            notes.read_section("contratos-api", "autenticação").unwrap(),
            "## Autenticação\nPOST /auth/token devolve refresh_token\n"
        );
        assert!(matches!(
            notes.read_section("contratos-api", "Pagamentos"),
            Err(NoteError::SectionNotFound(_))
        ));
        let hits = notes.search("LEGACY_ID").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!((hits[0].slug.as_str(), hits[0].line), ("contratos-api", 5));

        assert!(matches!(
            notes.create("contratos-api", "de novo"),
            Err(NoteError::HashRequired(_))
        ));
        assert!(matches!(
            notes.append("nao-existe", "x"),
            Err(NoteError::NotFound(_))
        ));
    }

    #[test]
    fn nome_invalido_nunca_vira_caminho() {
        let (_dir, notes) = notes();
        for bad in [
            "",
            "../fora",
            "Maiuscula",
            "com espaço",
            "-hifen",
            "a/b",
            "a.b",
        ] {
            assert!(
                matches!(notes.create(bad, "x"), Err(NoteError::InvalidSlug(_))),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn write_com_hash_desatualizado_falha_com_o_diff() {
        let (_dir, notes) = notes();
        let first = notes
            .write("decisoes", "# Decisões\nusar SQLite\n", None)
            .unwrap();
        // Outro agente mudou a nota depois da nossa leitura.
        notes.append("decisoes", "usar WAL").unwrap();

        let err = notes
            .write("decisoes", "# Decisões\nusar Postgres\n", Some(&first.hash))
            .unwrap_err();
        assert_eq!(err.code(), "stale_note");
        let NoteError::Stale {
            diff, current_hash, ..
        } = err
        else {
            panic!()
        };
        assert_eq!(
            diff,
            [
                DiffLine::Same("# Decisões".into()),
                DiffLine::Removed("usar SQLite".into()),
                DiffLine::Removed("usar WAL".into()),
                DiffLine::Added("usar Postgres".into()),
            ]
        );
        // Nada mudou no disco; com o hash atual, grava.
        assert!(notes.read("decisoes").unwrap().content.contains("usar WAL"));
        let saved = notes
            .write(
                "decisoes",
                "# Decisões\nusar Postgres\n",
                Some(&current_hash),
            )
            .unwrap();
        assert_eq!(notes.read("decisoes").unwrap().hash, saved.hash);
        assert!(matches!(
            notes.write("outra", "x", Some("abc")),
            Err(NoteError::NotFound(_))
        ));
    }

    #[test]
    fn appends_simultaneos_nao_perdem_conteudo() {
        let (_dir, notes) = notes();
        notes.create("log", "Log").unwrap();
        let writers: Vec<_> = (0..8)
            .map(|w| {
                let notes = notes.clone();
                std::thread::spawn(move || {
                    for i in 0..50 {
                        notes
                            .append("log", &format!("agente {w} linha {i}"))
                            .unwrap();
                    }
                })
            })
            .collect();
        for writer in writers {
            writer.join().unwrap();
        }
        let content = notes.read("log").unwrap().content;
        let lines: Vec<_> = content.lines().collect();
        assert_eq!(lines.len(), 1 + 8 * 50);
        for w in 0..8 {
            for i in 0..50 {
                let line = format!("agente {w} linha {i}");
                assert!(lines.contains(&line.as_str()), "perdeu {line}");
            }
        }
    }

    #[test]
    fn appends_de_arquivos_abertos_separados_nao_se_sobrescrevem() {
        // Sem o lock do processo: só o O_APPEND garante (é o caso de dois processos).
        let (_dir, notes) = notes();
        notes.create("raw", "Raw").unwrap();
        let path = notes.dir().join("raw.md");
        let writers: Vec<_> = (0..4)
            .map(|w| {
                let path = path.clone();
                std::thread::spawn(move || {
                    for i in 0..100 {
                        let mut f = fs::OpenOptions::new().append(true).open(&path).unwrap();
                        f.write_all(format!("p{w}-{i}\n").as_bytes()).unwrap();
                    }
                })
            })
            .collect();
        for writer in writers {
            writer.join().unwrap();
        }
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content.lines().count(), 1 + 400);
    }

    #[test]
    fn limites_de_tamanho() {
        let (_dir, notes) = notes();
        let big = "a".repeat(NOTE_MAX_BYTES + 1);
        assert!(matches!(
            notes.write("grande", &big, None),
            Err(NoteError::TooLarge)
        ));
        notes.create("quase", "Q").unwrap();
        let almost = "b".repeat(NOTE_MAX_BYTES - 4);
        assert!(matches!(
            notes.append("quase", &almost),
            Err(NoteError::TooLarge)
        ));
    }

    #[test]
    fn indice_do_boot_mostra_as_mais_recentes_sem_conteudo() {
        let summary = |slug: &str, at: Millis| NoteSummary {
            slug: slug.into(),
            title: format!("Título | {slug}"),
            updated_at: at,
            bytes: 10,
        };
        let now = 10 * 3_600_000;
        let list: Vec<_> = (0..25)
            .map(|i| summary(&format!("n{i}"), now - i * 60_000))
            .collect();
        let index = boot_index(&list, now, NOTES_BOOT_INDEX);
        assert!(index.contains("## Memória da equipe"));
        assert!(index.contains("| n0 | Título \\| n0 | agora |"));
        assert!(index.contains("| n19 |"));
        assert!(!index.contains("| n20 |"));
        assert!(index.contains("Mais 5 nota(s)"));
        assert_eq!(boot_index(&[], now, NOTES_BOOT_INDEX), "");
        assert_eq!(relative_age(now - 2 * 3_600_000, now), "há 2 h");
        assert_eq!(relative_age(now - 3 * 86_400_000, now), "há 3 dias");
    }

    #[test]
    fn diff_de_textos_iguais_e_so_contexto() {
        let diff = line_diff("a\nb\n", "a\nb\n");
        assert!(diff.iter().all(|d| matches!(d, DiffLine::Same(_))));
        assert_eq!(line_diff("", "novo\n"), [DiffLine::Added("novo".into())]);
    }
}
