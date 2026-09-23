//! Registro de sessões de PTY e bombeamento da saída até a interface.
//!
//! Mora aqui, e não no crate do Tauri, por dois motivos: a regra de dependência
//! (`docs/02-arquitetura.md`) mantém o app como camada fina, e assim toda esta lógica
//! — que é a parte com estado e concorrência — fica testável sem abrir uma janela.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use bytes::Bytes;
use tokio::sync::broadcast::error::{RecvError, TryRecvError};

use crate::batch::Batcher;
use crate::error::PtyError;
use crate::ring::RingBuffer;
use crate::session::{PtySession, PtySpawn, TerminalSize};

/// Quanto a tarefa de bombeamento dorme quando não há nada pendente.
const IDLE_POLL: Duration = Duration::from_millis(250);

/// Para onde vai a saída já coalescida. O app implementa emitindo eventos Tauri;
/// o teste implementa gravando numa lista.
pub trait OutputSink: Send + Sync + 'static {
    fn data(&self, agent_id: &str, chunk: Vec<u8>);
    fn exit(&self, agent_id: &str, code: i32);
}

struct Managed {
    session: Arc<PtySession>,
    batcher: Arc<Mutex<Batcher>>,
}

/// Todas as sessões vivas, endereçadas pelo id do agente.
#[derive(Default)]
pub struct PtyManager {
    sessions: RwLock<HashMap<String, Managed>>,
}

impl std::fmt::Debug for PtyManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PtyManager")
            .field("sessions", &self.len())
            .finish()
    }
}

impl PtyManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sobe uma sessão para o agente. Falha se já existir uma viva com esse id —
    /// subir duas para o mesmo agente deixaria uma órfã, consumindo CPU sem dono.
    pub fn spawn(
        &self,
        agent_id: impl Into<String>,
        spec: PtySpawn,
        sink: Arc<dyn OutputSink>,
    ) -> Result<(), PtyError> {
        self.spawn_with_ring(agent_id, spec, RingBuffer::default(), sink)
    }

    pub fn spawn_with_ring(
        &self,
        agent_id: impl Into<String>,
        spec: PtySpawn,
        ring: RingBuffer,
        sink: Arc<dyn OutputSink>,
    ) -> Result<(), PtyError> {
        let agent_id = agent_id.into();

        {
            let sessions = self.sessions.read().map_err(|_| PtyError::Closed)?;
            if sessions
                .get(&agent_id)
                .is_some_and(|managed| managed.session.is_running())
            {
                return Err(PtyError::AlreadyRunning(agent_id));
            }
        }

        let session = Arc::new(PtySession::spawn_with_ring(spec, ring)?);
        let batcher = Arc::new(Mutex::new(Batcher::with_default_window()));

        // Receptor criado dentro do `spawn`, antes da thread de leitura começar.
        // Inscrever-se aqui seria tarde demais: um `echo` rápido já teria enviado a
        // saída para um canal sem receptores, e ela seria descartada em silêncio.
        let stream = session.take_output().unwrap_or_else(|| session.subscribe());
        tokio::spawn(pump(
            agent_id.clone(),
            Arc::clone(&session),
            stream,
            Arc::clone(&batcher),
            sink,
        ));

        let mut sessions = self.sessions.write().map_err(|_| PtyError::Closed)?;
        sessions.insert(agent_id, Managed { session, batcher });
        Ok(())
    }

    pub fn write(&self, agent_id: &str, data: &[u8]) -> Result<(), PtyError> {
        self.with(agent_id, |managed| managed.session.write(data))?
    }

    pub fn resize(&self, agent_id: &str, size: TerminalSize) -> Result<(), PtyError> {
        self.with(agent_id, |managed| managed.session.resize(size))?
    }

    pub fn kill(&self, agent_id: &str) -> Result<(), PtyError> {
        self.with(agent_id, |managed| managed.session.kill())?
    }

    /// Histórico retido, para reidratar o terminal quando o painel volta a aparecer.
    pub fn snapshot(&self, agent_id: &str) -> Result<Vec<u8>, PtyError> {
        self.with(agent_id, |managed| managed.session.snapshot())
    }

    /// Liga ou desliga a emissão de eventos deste agente. Painel fora da tela não
    /// gera evento nenhum — o histórico continua no ring buffer.
    pub fn set_visible(&self, agent_id: &str, visible: bool) -> Result<(), PtyError> {
        self.with(agent_id, |managed| {
            if let Ok(mut batcher) = managed.batcher.lock() {
                batcher.set_visible(visible);
            }
        })
    }

    pub fn is_running(&self, agent_id: &str) -> bool {
        self.with(agent_id, |managed| managed.session.is_running())
            .unwrap_or(false)
    }

    pub fn exit_code(&self, agent_id: &str) -> Option<i32> {
        self.with(agent_id, |managed| managed.session.exit_code())
            .ok()
            .flatten()
    }

    /// Remove a sessão do registro, encerrando o processo se ainda estiver vivo.
    pub fn remove(&self, agent_id: &str) -> Result<(), PtyError> {
        let managed = {
            let mut sessions = self.sessions.write().map_err(|_| PtyError::Closed)?;
            sessions.remove(agent_id)
        };
        match managed {
            Some(managed) => {
                if managed.session.is_running() {
                    managed.session.kill()?;
                }
                Ok(())
            }
            None => Err(PtyError::UnknownAgent(agent_id.to_owned())),
        }
    }

    pub fn len(&self) -> usize {
        self.sessions
            .read()
            .map(|sessions| sessions.len())
            .unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn agent_ids(&self) -> Vec<String> {
        self.sessions
            .read()
            .map(|sessions| sessions.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Encerra tudo. Chamado ao fechar o app para não deixar processo órfão.
    pub fn shutdown(&self) {
        let sessions = match self.sessions.write() {
            Ok(mut sessions) => std::mem::take(&mut *sessions),
            Err(_) => return,
        };
        for (agent_id, managed) in sessions {
            if managed.session.is_running() {
                if let Err(error) = managed.session.kill() {
                    tracing::warn!(%agent_id, %error, "falha ao encerrar a sessão");
                }
            }
        }
    }

    fn with<T>(&self, agent_id: &str, action: impl FnOnce(&Managed) -> T) -> Result<T, PtyError> {
        let sessions = self.sessions.read().map_err(|_| PtyError::Closed)?;
        sessions
            .get(agent_id)
            .map(action)
            .ok_or_else(|| PtyError::UnknownAgent(agent_id.to_owned()))
    }
}

/// Lê a saída da sessão, agrupa por janela, entrega ao destino e — só então —
/// anuncia o término.
///
/// A ordem entre "última linha" e "processo encerrado" é **causal**, não temporal:
/// esperamos o EOF da leitura e drenamos o que restou antes de emitir a saída. Uma
/// versão anterior dormia 50 ms na esperança de que desse tempo, e a corrida
/// aparecia sob carga — justamente escondendo a mensagem de erro que explica a falha.
async fn pump(
    agent_id: String,
    session: Arc<PtySession>,
    mut stream: tokio::sync::broadcast::Receiver<Bytes>,
    batcher: Arc<Mutex<Batcher>>,
    sink: Arc<dyn OutputSink>,
) {
    let mut reader_done = session.reader_done();
    let mut finished = *reader_done.borrow();

    while !finished {
        // O tempo de espera é o que falta para o próximo lote; sem nada pendente,
        // dorme até chegar algo. Nunca segura o lock atravessando um `await`.
        let delay = match batcher.lock() {
            Ok(batcher) => batcher
                .time_until_ready(Instant::now())
                .unwrap_or(IDLE_POLL),
            Err(_) => return,
        };

        tokio::select! {
            received = stream.recv() => match received {
                Ok(chunk) => push(&batcher, &chunk),
                Err(RecvError::Lagged(skipped)) => {
                    // A interface ficou para trás. O histórico segue íntegro no ring
                    // buffer, e o próximo snapshot corrige a tela.
                    tracing::debug!(%agent_id, skipped, "saída do PTY descartada por atraso");
                }
                Err(RecvError::Closed) => finished = true,
            },
            changed = reader_done.changed() => {
                finished = changed.is_err() || *reader_done.borrow();
            }
            () = tokio::time::sleep(delay) => {}
        }

        emit_ready(&agent_id, &batcher, sink.as_ref());
    }

    // A leitura acabou, mas ainda pode haver chunks no canal. Drena sem bloquear.
    loop {
        match stream.try_recv() {
            Ok(chunk) => push(&batcher, &chunk),
            Err(TryRecvError::Lagged(skipped)) => {
                tracing::debug!(%agent_id, skipped, "saída do PTY descartada por atraso");
            }
            Err(TryRecvError::Empty | TryRecvError::Closed) => break,
        }
    }

    // Tudo entregue — inclusive as últimas linhas, que normalmente são o erro.
    let remaining = match batcher.lock() {
        Ok(mut batcher) => batcher.drain(Instant::now()),
        Err(_) => None,
    };
    if let Some(remaining) = remaining {
        sink.data(&agent_id, remaining);
    }

    sink.exit(&agent_id, session.wait().await);
}

fn push(batcher: &Mutex<Batcher>, chunk: &[u8]) {
    if let Ok(mut batcher) = batcher.lock() {
        batcher.push(chunk);
    }
}

fn emit_ready(agent_id: &str, batcher: &Mutex<Batcher>, sink: &dyn OutputSink) {
    let batch = match batcher.lock() {
        Ok(mut batcher) => batcher.poll(Instant::now()),
        Err(_) => None,
    };
    if let Some(batch) = batch {
        sink.data(agent_id, batch);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Instant;

    use super::*;

    /// Destino de teste: grava o que seria emitido para a interface.
    #[derive(Default)]
    struct Recorder {
        chunks: Mutex<Vec<(String, Vec<u8>)>>,
        exits: Mutex<Vec<(String, i32)>>,
        received: AtomicBool,
    }

    impl Recorder {
        fn text(&self) -> String {
            let chunks = self.chunks.lock().unwrap();
            chunks
                .iter()
                .map(|(_, data)| String::from_utf8_lossy(data).into_owned())
                .collect()
        }

        fn batches(&self) -> usize {
            self.chunks.lock().unwrap().len()
        }

        fn exits(&self) -> Vec<(String, i32)> {
            self.exits.lock().unwrap().clone()
        }
    }

    impl OutputSink for Recorder {
        fn data(&self, agent_id: &str, chunk: Vec<u8>) {
            self.received.store(true, Ordering::SeqCst);
            self.chunks
                .lock()
                .unwrap()
                .push((agent_id.to_owned(), chunk));
        }

        fn exit(&self, agent_id: &str, code: i32) {
            self.exits.lock().unwrap().push((agent_id.to_owned(), code));
        }
    }

    use crate::test_support::*;

    async fn wait_until(mut condition: impl FnMut() -> bool) {
        // Folga para o ConPTY do Windows 10, lento com vários terminais em paralelo.
        let deadline = Instant::now() + Duration::from_secs(30);
        while !condition() {
            assert!(
                Instant::now() < deadline,
                "condição não foi satisfeita a tempo"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    #[tokio::test]
    async fn entrega_a_saida_ao_destino() {
        let manager = PtyManager::new();
        let recorder = Arc::new(Recorder::default());

        manager
            .spawn("agt_1", shell("echo pelo-gerenciador"), recorder.clone())
            .unwrap();
        wait_until(|| recorder.text().contains("pelo-gerenciador")).await;

        let chunks = recorder.chunks.lock().unwrap();
        assert!(
            chunks.iter().all(|(id, _)| id == "agt_1"),
            "o id do agente vai junto"
        );
    }

    #[tokio::test]
    async fn nao_perde_a_saida_de_um_processo_instantaneo() {
        // Regressão: o receptor era criado depois da thread de leitura começar, e um
        // `echo` terminava antes de alguém estar inscrito. Um canal broadcast
        // descarta em silêncio o que é enviado sem receptores, então a saída sumia —
        // de forma intermitente, que é o pior jeito de sumir.
        for rodada in 0..10 {
            let manager = PtyManager::new();
            let recorder = Arc::new(Recorder::default());
            let marca = format!("instantaneo-{rodada}");

            manager
                .spawn(
                    "agt_rapido",
                    shell(&format!("echo {marca}")),
                    recorder.clone(),
                )
                .unwrap();
            wait_until(|| recorder.text().contains(&marca)).await;
        }
    }

    #[tokio::test]
    async fn anuncia_o_termino_com_o_codigo_de_saida() {
        let manager = PtyManager::new();
        let recorder = Arc::new(Recorder::default());

        manager
            .spawn("agt_2", shell("exit 7"), recorder.clone())
            .unwrap();
        wait_until(|| !recorder.exits().is_empty()).await;

        assert_eq!(recorder.exits(), vec![("agt_2".to_owned(), 7)]);
    }

    #[tokio::test]
    async fn as_ultimas_linhas_chegam_antes_do_evento_de_termino() {
        // Se o término chegasse primeiro, a mensagem de erro que explica a falha
        // apareceria depois do painel já ter sido marcado como parado.
        let manager = PtyManager::new();
        let recorder = Arc::new(Recorder::default());

        manager
            .spawn(
                "agt_3",
                echo_then_exit("erro fatal aqui", 1),
                recorder.clone(),
            )
            .unwrap();
        wait_until(|| !recorder.exits().is_empty()).await;

        assert!(
            recorder.text().contains("erro fatal aqui"),
            "saída: {:?}",
            recorder.text()
        );
    }

    #[tokio::test]
    async fn painel_invisivel_nao_emite_evento() {
        let manager = PtyManager::new();
        let recorder = Arc::new(Recorder::default());

        manager
            .spawn("agt_4", echo_stdin(), recorder.clone())
            .unwrap();
        manager.set_visible("agt_4", false).unwrap();
        manager.write("agt_4", b"barulho invisivel\n").unwrap();

        // O histórico continua íntegro mesmo sem evento nenhum.
        wait_until(|| {
            String::from_utf8_lossy(&manager.snapshot("agt_4").unwrap())
                .contains("barulho invisivel")
        })
        .await;
        assert_eq!(recorder.batches(), 0, "nada deveria ter sido emitido");

        manager.remove("agt_4").unwrap();
    }

    #[tokio::test]
    async fn saida_intensa_nao_vira_uma_enxurrada_de_eventos() {
        let manager = PtyManager::new();
        let recorder = Arc::new(Recorder::default());

        manager
            .spawn("agt_5", many_lines(INTENSE_LINES), recorder.clone())
            .unwrap();
        wait_until(|| !recorder.exits().is_empty()).await;

        assert!(
            recorder.text().contains(&format!("linha {INTENSE_LINES}")),
            "a saída precisa chegar inteira"
        );
        // 5000 linhas viram poucas dezenas de eventos, não 5000.
        assert!(
            recorder.batches() < 200,
            "lotes emitidos: {}",
            recorder.batches()
        );
    }

    #[tokio::test]
    async fn recusa_subir_duas_sessoes_para_o_mesmo_agente() {
        let manager = PtyManager::new();
        let recorder = Arc::new(Recorder::default());

        manager
            .spawn("agt_6", echo_stdin(), recorder.clone())
            .unwrap();
        let error = manager
            .spawn("agt_6", echo_stdin(), recorder.clone())
            .unwrap_err();

        assert!(matches!(error, PtyError::AlreadyRunning(_)));
        assert!(error.hint().is_some());
        assert_eq!(manager.len(), 1, "a sessão órfã não pode ficar no registro");

        manager.remove("agt_6").unwrap();
    }

    #[tokio::test]
    async fn permite_reiniciar_um_agente_que_ja_terminou() {
        let manager = PtyManager::new();
        let recorder = Arc::new(Recorder::default());

        manager
            .spawn("agt_7", shell("exit 0"), recorder.clone())
            .unwrap();
        wait_until(|| !manager.is_running("agt_7")).await;

        manager
            .spawn("agt_7", shell("echo reiniciado"), recorder.clone())
            .unwrap();
        wait_until(|| recorder.text().contains("reiniciado")).await;
    }

    #[tokio::test]
    async fn operacoes_em_agente_desconhecido_sao_erro_acionavel() {
        let manager = PtyManager::new();

        let error = manager.write("agt_fantasma", b"oi").unwrap_err();
        assert!(matches!(error, PtyError::UnknownAgent(_)));
        assert!(error.hint().is_some());
        assert!(manager.snapshot("agt_fantasma").is_err());
        assert!(manager.remove("agt_fantasma").is_err());
        assert!(!manager.is_running("agt_fantasma"));
    }

    #[tokio::test]
    async fn shutdown_encerra_tudo_sem_deixar_orfao() {
        let manager = PtyManager::new();
        let recorder = Arc::new(Recorder::default());

        for index in 0..4 {
            manager
                .spawn(format!("agt_{index}"), long_running(), recorder.clone())
                .unwrap();
        }
        assert_eq!(manager.len(), 4);

        manager.shutdown();
        assert!(manager.is_empty(), "o registro precisa ficar vazio");
    }

    #[tokio::test]
    async fn gerencia_varios_agentes_ao_mesmo_tempo() {
        let manager = PtyManager::new();
        let recorder = Arc::new(Recorder::default());

        for index in 0..6 {
            manager
                .spawn(
                    format!("agt_{index}"),
                    shell(&format!("echo sou-o-{index}")),
                    recorder.clone(),
                )
                .unwrap();
        }
        wait_until(|| recorder.exits().len() == 6).await;

        let text = recorder.text();
        for index in 0..6 {
            assert!(
                text.contains(&format!("sou-o-{index}")),
                "faltou o agente {index}"
            );
        }
        assert_eq!(manager.agent_ids().len(), 6);
    }
}
