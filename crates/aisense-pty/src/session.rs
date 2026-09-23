//! Uma sessão de PTY: processo vivo, leitura contínua e histórico.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize};
use tokio::sync::{broadcast, watch};

use crate::error::PtyError;
use crate::log::SessionLog;
use crate::ring::RingBuffer;

/// Quantos chunks ficam represados para quem assina a saída antes de haver perda.
/// Perder aqui não perde histórico: o ring buffer continua completo.
const BROADCAST_CAPACITY: usize = 512;
const READ_CHUNK: usize = 8 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSize {
    pub rows: u16,
    pub cols: u16,
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self { rows: 24, cols: 80 }
    }
}

impl From<TerminalSize> for PtySize {
    fn from(size: TerminalSize) -> Self {
        Self {
            rows: size.rows.max(1),
            cols: size.cols.max(1),
            pixel_width: 0,
            pixel_height: 0,
        }
    }
}

/// Tudo que é preciso para subir um processo de agente.
#[derive(Debug, Clone)]
pub struct PtySpawn {
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(String, String)>,
    pub size: TerminalSize,
    /// Onde gravar a transcrição completa. `None` guarda só o ring buffer.
    pub log_path: Option<PathBuf>,
}

impl PtySpawn {
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            args: Vec::new(),
            cwd: None,
            env: Vec::new(),
            size: TerminalSize::default(),
            log_path: None,
        }
    }

    pub fn log_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.log_path = Some(path.into());
        self
    }

    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    pub fn size(mut self, size: TerminalSize) -> Self {
        self.size = size;
        self
    }
}

type Master = Arc<Mutex<Option<Box<dyn MasterPty + Send>>>>;

pub struct PtySession {
    /// `None` depois que o processo termina no Windows — ver `close_console_on_exit`.
    master: Master,
    writer: Mutex<Box<dyn Write + Send>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    output: broadcast::Sender<Bytes>,
    /// Receptor criado **antes** da thread de leitura começar, entregue uma única vez
    /// ao consumidor principal. Sem isto há perda de saída: um canal broadcast
    /// descarta o que é enviado sem nenhum receptor inscrito, e um processo rápido
    /// (`echo oi`) termina antes de alguém conseguir se inscrever.
    primary: Mutex<Option<broadcast::Receiver<Bytes>>>,
    ring: Arc<Mutex<RingBuffer>>,
    exit: watch::Receiver<Option<i32>>,
    /// Vira `true` quando a leitura do PTY encontra EOF, ou seja, quando **toda** a
    /// saída já foi entregue. É o sinal que permite ordenar o evento de término
    /// depois da última linha, sem depender de tempo.
    reader_done: watch::Receiver<bool>,
    running: Arc<AtomicBool>,
    pid: Option<u32>,
}

impl std::fmt::Debug for PtySession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PtySession")
            .field("running", &self.is_running())
            .finish()
    }
}

impl PtySession {
    /// Sobe o processo num PTY e começa a bombear a saída para o ring buffer e para
    /// quem estiver assinando.
    pub fn spawn(spec: PtySpawn) -> Result<Self, PtyError> {
        Self::spawn_with_ring(spec, RingBuffer::default())
    }

    pub fn spawn_with_ring(spec: PtySpawn, ring: RingBuffer) -> Result<Self, PtyError> {
        if let Some(cwd) = &spec.cwd {
            if !cwd.is_dir() {
                return Err(PtyError::MissingWorkdir(cwd.clone()));
            }
        }

        let pty_system = portable_pty::native_pty_system();
        let pair = pty_system
            .openpty(spec.size.into())
            .map_err(|error| PtyError::OpenPty(error.to_string()))?;

        let mut command = CommandBuilder::new(&spec.command);
        for arg in &spec.args {
            command.arg(arg);
        }
        if let Some(cwd) = &spec.cwd {
            command.cwd(cwd);
        }
        for (key, value) in &spec.env {
            command.env(key, value);
        }

        let mut child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| PtyError::Spawn {
                command: spec.command.clone(),
                source: std::io::Error::other(error.to_string()),
            })?;

        // Soltar o lado escravo é o que faz a leitura enxergar EOF quando o processo
        // termina. Sem isto, a thread de leitura fica pendurada para sempre.
        drop(pair.slave);

        let killer = child.clone_killer();
        let pid = child.process_id();
        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| PtyError::OpenPty(error.to_string()))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| PtyError::OpenPty(error.to_string()))?;

        let (output, primary) = broadcast::channel(BROADCAST_CAPACITY);
        let (exit_tx, exit_rx) = watch::channel(None);
        let (reader_tx, reader_rx) = watch::channel(false);
        let ring = Arc::new(Mutex::new(ring));
        let running = Arc::new(AtomicBool::new(true));

        // Falha ao abrir o log não impede o agente de rodar: perder a transcrição é
        // ruim, não deixar o agente subir é pior.
        let log = spec
            .log_path
            .as_ref()
            .and_then(|path| match SessionLog::create(path) {
                Ok(log) => Some(log),
                Err(error) => {
                    tracing::warn!(%error, path = %path.display(), "log da sessão desabilitado");
                    None
                }
            });

        spawn_reader(reader, output.clone(), Arc::clone(&ring), log, reader_tx);

        // A espera pelo processo é bloqueante e mora na própria thread; o `killer` foi
        // clonado antes, então parar o agente não depende desta thread.
        let running_for_waiter = Arc::clone(&running);
        let master: Master = Arc::new(Mutex::new(Some(pair.master)));
        let master_for_waiter = Arc::clone(&master);
        std::thread::Builder::new()
            .name("aisense-pty-wait".into())
            .spawn(move || {
                let code = match child.wait() {
                    Ok(status) => i32::try_from(status.exit_code()).unwrap_or(-1),
                    Err(error) => {
                        tracing::warn!(%error, "falha ao aguardar o processo do agente");
                        -1
                    }
                };
                running_for_waiter.store(false, Ordering::SeqCst);
                close_console_on_exit(&master_for_waiter);
                let _ = exit_tx.send(Some(code));
            })
            .map_err(|error| PtyError::Spawn {
                command: spec.command.clone(),
                source: error,
            })?;

        Ok(Self {
            master,
            primary: Mutex::new(Some(primary)),
            writer: Mutex::new(writer),
            killer: Mutex::new(killer),
            output,
            ring,
            exit: exit_rx,
            reader_done: reader_rx,
            running,
            pid,
        })
    }

    /// Sinaliza quando a leitura do PTY terminou (EOF): a partir daí não vem mais
    /// saída nenhuma. Quem entrega a saída à interface usa isto para só anunciar o
    /// término depois de drenar tudo.
    pub fn reader_done(&self) -> watch::Receiver<bool> {
        self.reader_done.clone()
    }

    /// Receptor principal da saída, garantido sem perda desde o primeiro byte.
    ///
    /// Só pode ser retirado uma vez — é do consumidor que entrega a saída à
    /// interface. Para observadores adicionais, use `subscribe`.
    pub fn take_output(&self) -> Option<broadcast::Receiver<Bytes>> {
        self.primary
            .lock()
            .ok()
            .and_then(|mut primary| primary.take())
    }

    /// Assina a saída como observador. Recebe os chunks **a partir de agora**; o que
    /// veio antes está no `snapshot`.
    pub fn subscribe(&self) -> broadcast::Receiver<Bytes> {
        self.output.subscribe()
    }

    /// Envia bytes para o processo, como se tivessem sido digitados.
    pub fn write(&self, data: &[u8]) -> Result<(), PtyError> {
        if !self.is_running() {
            return Err(PtyError::Closed);
        }
        let mut writer = self.writer.lock().map_err(|_| PtyError::Closed)?;
        writer.write_all(data).map_err(PtyError::Write)?;
        writer.flush().map_err(PtyError::Write)
    }

    pub fn resize(&self, size: TerminalSize) -> Result<(), PtyError> {
        let master = self.master.lock().map_err(|_| PtyError::Closed)?;
        master
            .as_ref()
            .ok_or(PtyError::Closed)?
            .resize(size.into())
            .map_err(|error| PtyError::Resize(error.to_string()))
    }

    /// Todo o histórico retido, para reidratar o terminal na interface.
    pub fn snapshot(&self) -> Vec<u8> {
        self.ring
            .lock()
            .map(|ring| ring.snapshot())
            .unwrap_or_default()
    }

    pub fn dropped_entries(&self) -> u64 {
        self.ring
            .lock()
            .map(|ring| ring.dropped())
            .unwrap_or_default()
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// PID do processo, quando o sistema informa. Vai para a tabela `sessions`.
    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    pub fn exit_code(&self) -> Option<i32> {
        *self.exit.borrow()
    }

    pub fn kill(&self) -> Result<(), PtyError> {
        let mut killer = self.killer.lock().map_err(|_| PtyError::Closed)?;
        killer.kill().map_err(PtyError::Write)
    }

    /// Aguarda o processo terminar e devolve o exit code.
    pub async fn wait(&self) -> i32 {
        let mut exit = self.exit.clone();
        loop {
            if let Some(code) = *exit.borrow() {
                return code;
            }
            if exit.changed().await.is_err() {
                return -1;
            }
        }
    }
}

/// No Windows, a leitura de um ConPTY **não** vê EOF quando o processo termina: o
/// pseudo-console continua aberto e a thread de leitura ficaria parada para sempre —
/// e com ela o evento de término, do qual o supervisor depende para reiniciar
/// agentes. Soltar o master (o escravo já foi solto no spawn) chama
/// `ClosePseudoConsole`, que entrega o que falta da saída e fecha o pipe.
///
/// Em Unix o EOF já vem com a morte do processo; o master fica para quem ainda
/// quiser consultá-lo.
fn close_console_on_exit(master: &Master) {
    if cfg!(windows) {
        if let Ok(mut master) = master.lock() {
            drop(master.take());
        }
    }
}

fn spawn_reader(
    mut reader: Box<dyn Read + Send>,
    output: broadcast::Sender<Bytes>,
    ring: Arc<Mutex<RingBuffer>>,
    mut log: Option<SessionLog>,
    done: watch::Sender<bool>,
) {
    let done_on_failure = done.clone();
    // Leitura de PTY é bloqueante, então mora numa thread própria e não numa task
    // do Tokio (regra R6 / docs/10-padroes-de-codigo.md).
    let spawned = std::thread::Builder::new()
        .name("aisense-pty-read".into())
        .spawn(move || {
            let mut buffer = [0u8; READ_CHUNK];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(count) => {
                        let chunk = &buffer[..count];
                        if let Ok(mut ring) = ring.lock() {
                            ring.push(chunk);
                        }
                        if let Some(log) = log.as_mut() {
                            if let Err(error) = log.append(chunk) {
                                tracing::warn!(%error, "falha ao gravar o log da sessão");
                            }
                        }
                        // Sem assinante, `send` devolve erro — é o caso normal quando o
                        // painel está fechado, e o histórico já foi para o ring buffer.
                        let _ = output.send(Bytes::copy_from_slice(chunk));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(error) => {
                        tracing::debug!(%error, "leitura do PTY encerrada");
                        break;
                    }
                }
            }
            // Não vem mais saída. Precisa valer para todo caminho de saída do laço,
            // inclusive o de erro: quem espera este sinal ficaria preso para sempre.
            let _ = done.send(true);
        });

    if let Err(error) = spawned {
        tracing::error!(%error, "não foi possível iniciar a thread de leitura do PTY");
        // A thread não subiu: ninguém mais sinalizaria o fim da leitura.
        let _ = done_on_failure.send(true);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::time::{Duration, Instant};

    use super::*;
    use crate::ring::DEFAULT_MAX_BYTES;

    /// Espera a leitura do PTY chegar ao EOF: só aí toda a saída está no ring e no
    /// log. A morte do processo não basta — no Windows o ConPTY ainda entrega saída
    /// depois dela, e um `sleep` fixo perdia a última linha no runner do CI.
    async fn wait_reader_done(session: &PtySession) {
        let mut done = session.reader_done();
        tokio::time::timeout(Duration::from_secs(10), done.wait_for(|d| *d))
            .await
            .expect("a leitura do PTY não terminou em 10 s")
            .expect("o sinal de fim de leitura foi descartado");
    }

    use crate::test_support::*;

    /// Procura uma linha exata na saída.
    ///
    /// Um PTY traduz `\n` em `\r\n` (termios `ONLCR`), então procurar por
    /// `"linha 1\n"` **não** casa. E procurar só por `"linha 1"` casaria com
    /// `"linha 10"`. Esta armadilha vale para qualquer código que analise saída de
    /// terminal — incluindo o detector de estado da Fase 03.
    fn has_line(haystack: &str, line: &str) -> bool {
        haystack.contains(&format!("{line}\r\n")) || haystack.contains(&format!("{line}\n"))
    }

    /// Espera o texto aparecer na saída. PTY é assíncrono por natureza: não dá para
    /// ler logo depois do spawn e esperar que já esteja lá.
    async fn wait_for(session: &PtySession, needle: &str) -> String {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let text = String::from_utf8_lossy(&session.snapshot()).into_owned();
            if text.contains(needle) {
                return text;
            }
            assert!(
                Instant::now() < deadline,
                "esperava encontrar {needle:?} na saída, mas veio: {text:?}"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    #[tokio::test]
    async fn captura_a_saida_do_processo() {
        let session = PtySession::spawn(shell("echo ola-do-terminal")).unwrap();
        let output = wait_for(&session, "ola-do-terminal").await;
        assert!(output.contains("ola-do-terminal"), "{output:?}");
        assert_eq!(session.wait().await, 0);
    }

    #[tokio::test]
    async fn propaga_o_codigo_de_saida() {
        let session = PtySession::spawn(shell("exit 3")).unwrap();
        assert_eq!(session.wait().await, 3);
        assert!(!session.is_running());
        assert_eq!(session.exit_code(), Some(3));
    }

    #[tokio::test]
    async fn escreve_no_processo_como_se_fosse_digitado() {
        let session = PtySession::spawn(echo_stdin()).unwrap();
        session.write(b"linha digitada\n").unwrap();
        let output = wait_for(&session, "linha digitada").await;
        assert!(output.contains("linha digitada"), "{output:?}");
        session.kill().unwrap();
    }

    #[tokio::test]
    async fn entrega_a_saida_a_quem_assina() {
        let session = PtySession::spawn(echo_stdin()).unwrap();
        let mut stream = session.subscribe();

        session.write(b"pelo-broadcast\n").unwrap();

        let received = tokio::time::timeout(Duration::from_secs(10), async {
            let mut seen = String::new();
            while let Ok(chunk) = stream.recv().await {
                seen.push_str(&String::from_utf8_lossy(&chunk));
                if seen.contains("pelo-broadcast") {
                    return seen;
                }
            }
            seen
        })
        .await
        .expect("assinante deveria receber a saída");

        assert!(received.contains("pelo-broadcast"), "{received:?}");
        session.kill().unwrap();
    }

    #[tokio::test]
    async fn o_processo_enxerga_o_tamanho_do_terminal() {
        // Se o tamanho não chegasse ao processo, TUIs como htop desenhariam errado.
        let session = PtySession::spawn(print_size().size(TerminalSize {
            rows: 40,
            cols: 132,
        }))
        .unwrap();
        if cfg!(windows) {
            // `mode con` sai localizado ("Linhas: 40", "Colunas: 132").
            let output = wait_for(&session, "132").await;
            assert!(
                output.contains("40"),
                "esperava 40 linhas, veio: {output:?}"
            );
        } else {
            let output = wait_for(&session, "40").await;
            assert!(
                output.contains("40 132"),
                "esperava '40 132', veio: {output:?}"
            );
        }
    }

    #[tokio::test]
    async fn redimensionar_nao_falha_com_a_sessao_viva() {
        let session = PtySession::spawn(echo_stdin()).unwrap();
        session
            .resize(TerminalSize {
                rows: 50,
                cols: 200,
            })
            .unwrap();
        session.kill().unwrap();
    }

    #[tokio::test]
    async fn kill_encerra_um_processo_que_ficaria_parado() {
        let session = PtySession::spawn(long_running()).unwrap();
        assert!(session.is_running());
        session.kill().unwrap();

        let code = tokio::time::timeout(Duration::from_secs(10), session.wait())
            .await
            .expect("kill deveria encerrar o processo");
        assert_ne!(code, 0, "processo morto não sai com sucesso");
        assert!(!session.is_running());
    }

    #[tokio::test]
    async fn escrever_em_sessao_encerrada_devolve_erro_com_dica() {
        let session = PtySession::spawn(shell("exit 0")).unwrap();
        session.wait().await;

        let error = session.write(b"tarde demais\n").unwrap_err();
        assert!(matches!(error, PtyError::Closed));
        assert!(error.hint().is_some(), "erro precisa dizer o que fazer");
    }

    #[tokio::test]
    async fn diretorio_de_trabalho_inexistente_falha_antes_de_subir_o_processo() {
        let error = PtySession::spawn(shell("echo oi").cwd("/nao/existe/mesmo")).unwrap_err();
        assert!(matches!(error, PtyError::MissingWorkdir(_)));
        assert!(error.hint().is_some());
    }

    #[tokio::test]
    async fn comando_inexistente_devolve_erro_acionavel() {
        let spawn = PtySpawn::new("comando-que-nao-existe-aisense");
        let result = PtySession::spawn(spawn);

        // Em alguns sistemas o erro aparece no spawn; em outros, o processo sobe e
        // morre em seguida. Os dois caminhos precisam terminar em falha visível.
        match result {
            Err(error) => assert!(error.hint().is_some(), "erro precisa dizer o que fazer"),
            Ok(session) => assert_ne!(session.wait().await, 0, "deveria falhar ao executar"),
        }
    }

    #[tokio::test]
    async fn o_ambiente_chega_ao_processo() {
        // É assim que AISENSE_TOKEN e AISENSE_AGENT_HANDLE chegam ao agente.
        let session = PtySession::spawn(
            print_env("AISENSE_AGENT_HANDLE", "handle").env("AISENSE_AGENT_HANDLE", "backend"),
        )
        .unwrap();
        let output = wait_for(&session, "handle=backend").await;
        assert!(output.contains("handle=backend"), "{output:?}");
    }

    #[tokio::test]
    async fn grava_a_transcricao_completa_em_arquivo() {
        // O ring buffer só guarda as últimas linhas; o log guarda tudo. Este teste
        // prova que a linha que o ring já descartou continua no arquivo.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agente.log");

        let session = PtySession::spawn_with_ring(
            many_lines(300).log_path(&path),
            RingBuffer::new(10, DEFAULT_MAX_BYTES),
        )
        .unwrap();

        assert_eq!(session.wait().await, 0);
        wait_reader_done(&session).await;

        let transcript = std::fs::read_to_string(&path).unwrap();
        assert!(
            has_line(&transcript, "linha 1"),
            "o começo precisa estar no log"
        );
        assert!(has_line(&transcript, "linha 300"), "o fim também");

        let snapshot = String::from_utf8_lossy(&session.snapshot()).into_owned();
        assert!(
            !has_line(&snapshot, "linha 1"),
            "o ring buffer já descartou o começo"
        );
        assert!(has_line(&snapshot, "linha 300"), "mas manteve o fim");
    }

    #[tokio::test]
    async fn aguenta_saida_volumosa_sem_estourar_a_memoria() {
        let session = PtySession::spawn_with_ring(
            many_lines(VOLUME_LINES),
            RingBuffer::new(500, DEFAULT_MAX_BYTES),
        )
        .unwrap();

        assert_eq!(session.wait().await, 0);
        wait_reader_done(&session).await;

        let snapshot = session.snapshot();
        assert!(
            snapshot.len() < 256 * 1024,
            "reteve {} bytes",
            snapshot.len()
        );
        assert!(
            session.dropped_entries() > 0,
            "deveria ter descartado linhas antigas"
        );
    }
}
