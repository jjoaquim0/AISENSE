//! Cliente do barramento: o que a CLI `aisense` e o `aisense-mcp` usam. Conecta, faz o
//! `hello` e manda operações — uma conexão por comando, ou longa para `wait`/`ask`.

use tokio::io::BufReader;

use crate::frame::{read_frame, write_frame, FrameError, MAX_FRAME};
use crate::protocol::{Request, Response};
use crate::transport::{connect, Io};

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// O app não está aberto (ou o socket é de outro usuário).
    #[error("could not reach AISENSE at {endpoint}: {source}")]
    Connect {
        endpoint: String,
        source: std::io::Error,
    },
    /// O `hello` foi recusado: token inválido, expirado ou de sessão encerrada.
    #[error("{}", .0.message.clone().unwrap_or_default())]
    Refused(Response),
    #[error("the connection to AISENSE closed unexpectedly")]
    Closed,
    #[error("invalid answer from AISENSE: {0}")]
    Protocol(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub struct Client {
    io: BufReader<Box<dyn Io>>,
    /// A resposta do `hello`: identidade e equipe.
    pub hello: Response,
}

impl Client {
    /// Conecta e autentica.
    pub async fn connect(endpoint: &str, token: &str) -> Result<Self, ClientError> {
        let io = connect(endpoint)
            .await
            .map_err(|source| ClientError::Connect {
                endpoint: endpoint.to_owned(),
                source,
            })?;
        let mut client = Self {
            io: BufReader::new(io),
            hello: Response::ok(()),
        };
        let hello = client
            .call(&Request::Hello {
                token: token.to_owned(),
                v: crate::protocol::PROTOCOL_VERSION,
            })
            .await?;
        if !hello.ok {
            return Err(ClientError::Refused(hello));
        }
        client.hello = hello;
        Ok(client)
    }

    /// Uma operação e a resposta dela (que pode ser `ok: false`).
    pub async fn call(&mut self, request: &Request) -> Result<Response, ClientError> {
        write_frame(&mut self.io, request).await?;
        match read_frame(&mut self.io, MAX_FRAME).await {
            Ok(Some(frame)) => {
                serde_json::from_slice(&frame).map_err(|e| ClientError::Protocol(e.to_string()))
            }
            Ok(None) => Err(ClientError::Closed),
            Err(FrameError::TooLarge) => Err(ClientError::Protocol("answer too large".into())),
            Err(FrameError::Io(e)) => Err(e.into()),
        }
    }
}
