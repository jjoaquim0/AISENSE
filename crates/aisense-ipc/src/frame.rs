//! NDJSON com teto: uma linha de até [`MAX_FRAME`] bytes. Quem manda mais que isso recebe
//! `frame_too_large` e a conexão fecha — o servidor segue atendendo os outros.

use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};

/// 1 MiB por frame (`docs/07`).
pub const MAX_FRAME: usize = 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("frame larger than {} bytes", MAX_FRAME)]
    TooLarge,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// A próxima linha sem o `\n` (e sem `\r` no fim). `None` no fim da conexão. Nunca guarda
/// mais que `max` bytes: um cliente mandando lixo sem quebra de linha não enche a memória.
pub async fn read_frame<R: AsyncBufRead + Unpin>(
    reader: &mut R,
    max: usize,
) -> Result<Option<Vec<u8>>, FrameError> {
    let mut line = Vec::new();
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            // EOF: linha incompleta no fim também vale (clientes que não mandam `\n`).
            return Ok((!line.is_empty()).then_some(line));
        }
        match available.iter().position(|b| *b == b'\n') {
            Some(at) => {
                if line.len() + at > max {
                    return Err(FrameError::TooLarge);
                }
                line.extend_from_slice(&available[..at]);
                reader.consume(at + 1);
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                return Ok(Some(line));
            }
            None => {
                let n = available.len();
                if line.len() + n > max {
                    return Err(FrameError::TooLarge);
                }
                line.extend_from_slice(available);
                reader.consume(n);
            }
        }
    }
}

/// Serializa em uma linha e manda.
pub async fn write_frame<W: AsyncWrite + Unpin, T: serde::Serialize>(
    writer: &mut W,
    value: &T,
) -> std::io::Result<()> {
    let mut bytes = serde_json::to_vec(value).map_err(std::io::Error::other)?;
    bytes.push(b'\n');
    writer.write_all(&bytes).await?;
    writer.flush().await
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use tokio::io::BufReader;

    #[tokio::test]
    async fn le_linhas_e_recusa_frame_grande_sem_guardar_tudo() {
        let data = b"{\"a\":1}\r\n{\"b\":2}\nsem-quebra".to_vec();
        let mut reader = BufReader::with_capacity(4, &data[..]);
        assert_eq!(
            read_frame(&mut reader, 64).await.unwrap().unwrap(),
            b"{\"a\":1}"
        );
        assert_eq!(
            read_frame(&mut reader, 64).await.unwrap().unwrap(),
            b"{\"b\":2}"
        );
        assert_eq!(
            read_frame(&mut reader, 64).await.unwrap().unwrap(),
            b"sem-quebra"
        );
        assert!(read_frame(&mut reader, 64).await.unwrap().is_none());

        let big = [b'x'; 100];
        let mut reader = BufReader::with_capacity(8, &big[..]);
        assert!(matches!(
            read_frame(&mut reader, 64).await,
            Err(FrameError::TooLarge)
        ));
    }
}
