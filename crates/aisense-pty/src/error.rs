use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum PtyError {
    #[error("não foi possível abrir um terminal: {0}")]
    OpenPty(String),

    #[error("comando '{command}' não pôde ser executado: {source}")]
    Spawn {
        command: String,
        #[source]
        source: std::io::Error,
    },

    #[error("diretório de trabalho não existe: {0}")]
    MissingWorkdir(PathBuf),

    #[error("falha ao escrever no terminal: {0}")]
    Write(#[source] std::io::Error),

    #[error("falha ao redimensionar o terminal: {0}")]
    Resize(String),

    #[error("a sessão do terminal já foi encerrada")]
    Closed,

    #[error("o agente {0} já tem uma sessão em execução")]
    AlreadyRunning(String),

    #[error("nenhuma sessão de terminal para o agente {0}")]
    UnknownAgent(String),
}

impl PtyError {
    /// Dica acionável para a interface. Erro que só diz "falhou" faz o usuário
    /// abrir um chamado; erro que diz o que fazer resolve sozinho.
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            Self::Spawn { .. } => {
                Some("Verifique se o runtime está instalado e acessível no PATH.")
            }
            Self::MissingWorkdir(_) => {
                Some("Escolha outro diretório de trabalho para a equipe ou para o agente.")
            }
            Self::Closed => Some("Inicie o agente novamente."),
            Self::AlreadyRunning(_) => Some("Pare o agente antes de iniciá-lo de novo."),
            Self::UnknownAgent(_) => Some("Inicie o agente para abrir o terminal dele."),
            _ => None,
        }
    }
}
