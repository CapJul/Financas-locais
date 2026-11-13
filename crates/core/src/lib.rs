//! Núcleo do Finanças Locais: regras de fatura, parcelas e divisão entre pessoas,
//! mais a persistência em SQLite. Não depende do Tauri, então pode ser testado
//! com `cargo test -p financas-core` em qualquer máquina.

pub mod competencia;
pub mod dominio;
pub mod erro;
pub mod modelos;
pub mod repositorio;
pub mod seguranca;

pub use competencia::Competencia;
pub use erro::{Erro, Resultado};
pub use repositorio::Repositorio;
