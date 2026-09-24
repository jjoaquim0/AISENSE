//! Socket local: Unix domain socket (macOS/Linux) ou named pipe (Windows), pelo próprio
//! `tokio`. Nada escuta em rede (ADR 0004).

use std::io;
use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncWrite, BufReader};
use tokio_util::sync::CancellationToken;

use crate::frame::{read_frame, write_frame, FrameError, MAX_FRAME};
use crate::protocol::{codes, Request, Response};

/// Um lado de conexão qualquer (socket ou pipe).
pub trait Io: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}

/// Quem atende as operações. O transporte só cuida de frame, `hello` e erro de protocolo.
pub trait Handler: Send + Sync + 'static {
    /// Valida o token do `hello`; `Err` é a resposta de recusa (a conexão fecha depois).
    fn hello(
        &self,
        token: &str,
    ) -> impl std::future::Future<Output = Result<Response, Response>> + Send;
    /// Uma operação de uma conexão já autenticada com `token`.
    fn handle(
        &self,
        token: &str,
        request: Request,
    ) -> impl std::future::Future<Output = Response> + Send;
}

/// Atende uma conexão até ela fechar.
pub async fn serve_connection<IO: Io, H: Handler>(io: IO, handler: Arc<H>) {
    let mut io = BufReader::new(io);
    let token = match read_frame(&mut io, MAX_FRAME).await {
        Ok(Some(frame)) => match serde_json::from_slice::<Request>(&frame) {
            Ok(Request::Hello { token, .. }) => token,
            _ => {
                let reply = Response::error(
                    codes::UNAUTHORIZED,
                    "the first frame must be hello with AISENSE_TOKEN",
                    None,
                );
                let _ = write_frame(&mut io, &reply).await;
                return;
            }
        },
        Ok(None) | Err(FrameError::Io(_)) => return,
        Err(FrameError::TooLarge) => {
            let _ = write_frame(&mut io, &too_large()).await;
            return;
        }
    };
    match handler.hello(&token).await {
        Ok(reply) => {
            if write_frame(&mut io, &reply).await.is_err() {
                return;
            }
        }
        Err(refusal) => {
            let _ = write_frame(&mut io, &refusal).await;
            return;
        }
    }
    loop {
        let frame = match read_frame(&mut io, MAX_FRAME).await {
            Ok(Some(frame)) => frame,
            Ok(None) | Err(FrameError::Io(_)) => return,
            Err(FrameError::TooLarge) => {
                // Sem como achar o começo do próximo frame: responde e fecha.
                let _ = write_frame(&mut io, &too_large()).await;
                return;
            }
        };
        if frame.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let reply = match serde_json::from_slice::<Request>(&frame) {
            Ok(Request::Hello { .. }) => Response::error(
                codes::INVALID_REQUEST,
                "this connection is already authenticated",
                None,
            ),
            Ok(request) => {
                tracing::debug!(op = request.op(), "ipc");
                handler.handle(&token, request).await
            }
            Err(e) => Response::error(codes::INVALID_REQUEST, format!("invalid frame: {e}"), None),
        };
        if write_frame(&mut io, &reply).await.is_err() {
            return;
        }
    }
}

fn too_large() -> Response {
    Response::error(
        codes::FRAME_TOO_LARGE,
        format!("frames are limited to {} bytes", MAX_FRAME),
        Some("Grave o conteúdo num arquivo e mande o caminho.".into()),
    )
}

#[derive(Debug, thiserror::Error)]
pub enum BindError {
    #[error("another AISENSE is already listening on {0}")]
    InUse(String),
    #[error("could not listen on {endpoint}: {source}")]
    Io { endpoint: String, source: io::Error },
}

/// Escuta em `endpoint` (caminho do socket ou nome do pipe) até `shutdown`.
pub async fn serve<H: Handler>(
    endpoint: &str,
    handler: Arc<H>,
    shutdown: CancellationToken,
) -> Result<(), BindError> {
    imp::serve(endpoint, handler, shutdown).await
}

/// Abre uma conexão de cliente.
pub async fn connect(endpoint: &str) -> io::Result<Box<dyn Io>> {
    imp::connect(endpoint).await
}

#[cfg(unix)]
mod imp {
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    use tokio::net::{UnixListener, UnixStream};

    use super::*;

    pub async fn serve<H: Handler>(
        endpoint: &str,
        handler: Arc<H>,
        shutdown: CancellationToken,
    ) -> Result<(), BindError> {
        let io_err = |source| BindError::Io {
            endpoint: endpoint.to_owned(),
            source,
        };
        let path = Path::new(endpoint);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(io_err)?;
            // A pasta só do usuário: o socket nasce protegido, antes mesmo do chmod.
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
                .map_err(io_err)?;
        }
        if path.exists() {
            if UnixStream::connect(path).await.is_ok() {
                return Err(BindError::InUse(endpoint.to_owned()));
            }
            // Sobra de uma execução que caiu.
            std::fs::remove_file(path).map_err(io_err)?;
        }
        let listener = UnixListener::bind(path).map_err(io_err)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).map_err(io_err)?;
        tracing::info!(socket = %endpoint, "barramento escutando");
        loop {
            tokio::select! {
                () = shutdown.cancelled() => break,
                accepted = listener.accept() => match accepted {
                    Ok((stream, _)) => {
                        tokio::spawn(serve_connection(stream, Arc::clone(&handler)));
                    }
                    Err(error) => tracing::warn!(%error, "conexão recusada no barramento"),
                },
            }
        }
        let _ = std::fs::remove_file(path);
        Ok(())
    }

    pub async fn connect(endpoint: &str) -> io::Result<Box<dyn Io>> {
        Ok(Box::new(UnixStream::connect(endpoint).await?))
    }
}

#[cfg(windows)]
mod imp {
    use std::time::Duration;

    use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};

    use super::*;

    /// `ERROR_PIPE_BUSY`: todas as instâncias ocupadas; tenta de novo em seguida.
    const PIPE_BUSY: i32 = 231;

    pub async fn serve<H: Handler>(
        endpoint: &str,
        handler: Arc<H>,
        shutdown: CancellationToken,
    ) -> Result<(), BindError> {
        let io_err = |source| BindError::Io {
            endpoint: endpoint.to_owned(),
            source,
        };
        // `first_pipe_instance`: se outro processo já criou o pipe com este nome (outro
        // AISENSE, ou alguém tentando se passar por ele), falha em vez de dividir o nome.
        let mut server = ServerOptions::new()
            .first_pipe_instance(true)
            .reject_remote_clients(true)
            .create(endpoint)
            .map_err(|e| {
                if e.kind() == io::ErrorKind::PermissionDenied {
                    BindError::InUse(endpoint.to_owned())
                } else {
                    io_err(e)
                }
            })?;
        tracing::info!(pipe = %endpoint, "barramento escutando");
        loop {
            tokio::select! {
                () = shutdown.cancelled() => break,
                connected = server.connect() => {
                    if let Err(error) = connected {
                        tracing::warn!(%error, "conexão recusada no barramento");
                        continue;
                    }
                    let next = ServerOptions::new()
                        .reject_remote_clients(true)
                        .create(endpoint)
                        .map_err(io_err)?;
                    let client = std::mem::replace(&mut server, next);
                    tokio::spawn(serve_connection(client, Arc::clone(&handler)));
                }
            }
        }
        Ok(())
    }

    pub async fn connect(endpoint: &str) -> io::Result<Box<dyn Io>> {
        for _ in 0..50 {
            match ClientOptions::new().open(endpoint) {
                Ok(client) => return Ok(Box::new(client)),
                Err(e) if e.raw_os_error() == Some(PIPE_BUSY) => {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                Err(e) => return Err(e),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "the bus pipe stayed busy",
        ))
    }
}
