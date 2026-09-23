//! Posição de erros em arquivos TOML de configuração (adaptadores, `aisense.toml`).
//!
//! Erro de configuração que não diz a linha manda o usuário caçar o problema no olho;
//! estes utilitários traduzem o deslocamento do parser e acham a linha de um campo.

/// Converte um deslocamento em bytes para linha e coluna, ambas a partir de 1.
pub(crate) fn line_col(source: &str, offset: usize) -> (u32, u32) {
    let before = source.get(..offset).unwrap_or(source);
    let line = before.matches('\n').count() + 1;
    let column = before
        .rsplit('\n')
        .next()
        .map_or(0, |tail| tail.chars().count())
        + 1;
    (to_u32(line), to_u32(column))
}

/// Linha da primeira atribuição `key = ...` (ou tabela `[key]`). Melhor esforço: se o
/// campo faltou, não há linha para apontar e o aviso sai só com o caminho.
pub(crate) fn find_key_line(source: &str, key: &str) -> Option<u32> {
    source
        .lines()
        .position(|line| {
            let line = line.trim_start();
            let table = line
                .strip_prefix('[')
                .and_then(|rest| rest.strip_suffix(']').or(Some(rest)))
                .is_some_and(|name| name.trim() == key);
            let assignment = line
                .strip_prefix(key)
                .is_some_and(|rest| rest.trim_start().starts_with(['=', '.']));
            table || assignment
        })
        .map(|index| to_u32(index + 1))
}

pub(crate) fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}
