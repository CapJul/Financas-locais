//! Senha de acesso ao app. Guardamos só o hash Argon2 (formato PHC), nunca a senha.

use password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;

use crate::erro::{invalido, Erro, Resultado};

pub const TAMANHO_MINIMO: usize = 4;

pub fn gerar_hash(senha: &str) -> Resultado<String> {
    if senha.chars().count() < TAMANHO_MINIMO {
        return invalido(format!("a senha precisa ter pelo menos {TAMANHO_MINIMO} caracteres"));
    }
    let sal = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(senha.as_bytes(), &sal)
        .map(|h| h.to_string())
        .map_err(|e| Erro::Validacao(format!("não foi possível gerar a senha: {e}")))
}

pub fn conferir(senha: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .map(|h| Argon2::default().verify_password(senha.as_bytes(), &h).is_ok())
        .unwrap_or(false)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn confere_senha_certa_e_recusa_errada() {
        let h = gerar_hash("minha-senha").unwrap();
        assert!(h.starts_with("$argon2"));
        assert!(conferir("minha-senha", &h));
        assert!(!conferir("outra", &h));
    }

    #[test]
    fn recusa_senha_curta() {
        assert!(gerar_hash("abc").is_err());
    }
}
