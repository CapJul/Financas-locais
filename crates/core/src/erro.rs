use thiserror::Error;

#[derive(Debug, Error)]
pub enum Erro {
    #[error("{0}")]
    Validacao(String),

    #[error("registro não encontrado: {0}")]
    NaoEncontrado(String),

    #[error("erro no banco de dados: {0}")]
    Banco(#[from] rusqlite::Error),
}

pub type Resultado<T> = Result<T, Erro>;

/// Atalho para erros de validação.
pub fn invalido<T>(msg: impl Into<String>) -> Resultado<T> {
    Err(Erro::Validacao(msg.into()))
}
