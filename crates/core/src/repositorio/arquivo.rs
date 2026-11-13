//! O arquivo do banco: abrir (com ou sem senha), criptografar com SQLCipher,
//! trocar/remover a senha, fazer e restaurar backups.
//!
//! Com senha, o arquivo inteiro fica cifrado (AES-256, chave derivada da senha
//! pelo próprio SQLCipher). Sem a senha certa, o arquivo é ilegível.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::Local;
use rusqlite::{params, Connection, ErrorCode, OptionalExtension};
use serde::Serialize;

use super::{Repositorio, MIGRACOES};
use crate::erro::{invalido, Erro, Resultado};
use crate::seguranca;

/// Prefixo dos arquivos de backup automático (a rotação só mexe nesses).
const PREFIXO_BACKUP: &str = "financas-locais-backup-";
/// Quantos backups automáticos manter na pasta.
const BACKUPS_MANTIDOS: usize = 10;
/// Intervalo mínimo entre backups automáticos.
const HORAS_ENTRE_BACKUPS: i64 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SituacaoArquivo {
    /// Ainda não existe: será criado sem senha.
    Novo,
    /// Existe e não tem senha.
    Aberto,
    /// Existe e está cifrado: precisa da senha para abrir.
    Cifrado,
    /// Banco da v0.1 com senha só de tela: abre com a senha e é cifrado na hora.
    LegadoComSenha,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoBackup {
    pub pasta: Option<String>,
    /// Data e hora do último backup automático (AAAA-MM-DD HH:MM).
    pub ultimo: Option<String>,
    /// Backups saem cifrados quando o banco tem senha.
    pub cifrado: bool,
}

fn eh_arquivo_cifrado(e: &rusqlite::Error) -> bool {
    matches!(e, rusqlite::Error::SqliteFailure(f, _) if f.code == ErrorCode::NotADatabase)
}

/// Abre a conexão e, se houver senha, aplica a chave e confere se ela abre o arquivo.
fn conectar(caminho: &Path, chave: Option<&str>) -> Resultado<Connection> {
    let conn = Connection::open(caminho)?;
    if let Some(k) = chave {
        conn.pragma_update(None, "key", k)?;
    }
    match conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0)) {
        Ok(_) => Ok(conn),
        Err(e) if eh_arquivo_cifrado(&e) => Err(Erro::SenhaIncorreta),
        Err(e) => Err(e.into()),
    }
}

/// Caminho temporário ao lado do arquivo (mesma pasta = `rename` atômico).
fn ao_lado(caminho: &Path, sufixo: &str) -> PathBuf {
    let mut nome = caminho.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    nome.push(sufixo);
    caminho.with_file_name(nome)
}

fn remover_se_existe(caminho: &Path) -> Resultado<()> {
    match fs::remove_file(caminho) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

impl Repositorio {
    /// Diz como o arquivo está, sem precisar de senha.
    pub fn situacao(caminho: impl AsRef<Path>) -> Resultado<SituacaoArquivo> {
        let caminho = caminho.as_ref();
        if !caminho.exists() || fs::metadata(caminho)?.len() == 0 {
            return Ok(SituacaoArquivo::Novo);
        }
        let conn = match conectar(caminho, None) {
            Ok(c) => c,
            Err(Erro::SenhaIncorreta) => return Ok(SituacaoArquivo::Cifrado),
            Err(e) => return Err(e),
        };
        let tem_config: bool = conn
            .query_row("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'config'", [], |_| Ok(()))
            .optional()?
            .is_some();
        let legado = tem_config
            && conn
                .query_row("SELECT 1 FROM config WHERE chave = 'senha_hash'", [], |_| Ok(()))
                .optional()?
                .is_some();
        Ok(if legado { SituacaoArquivo::LegadoComSenha } else { SituacaoArquivo::Aberto })
    }

    /// Abre (ou cria) o banco. `senha` só é usada se o arquivo estiver cifrado.
    pub fn abrir(caminho: impl AsRef<Path>, senha: Option<&str>) -> Resultado<Self> {
        let caminho = caminho.as_ref().to_path_buf();
        let chave = senha.filter(|s| !s.is_empty()).map(str::to_string);
        let conn = conectar(&caminho, chave.as_deref())?;
        Self::preparar(conn, Some(caminho), chave)
    }

    /// Abre um banco da v0.1 que tinha senha só de tela: confere a senha antiga
    /// e já cifra o arquivo com ela.
    pub fn abrir_legado(caminho: impl AsRef<Path>, senha: &str) -> Resultado<Self> {
        let mut repo = Self::abrir(caminho, None)?;
        let hash = repo.ler_config("senha_hash")?.ok_or(Erro::SenhaIncorreta)?;
        if !seguranca::conferir(senha, &hash) {
            return Err(Erro::SenhaIncorreta);
        }
        repo.gravar_config("senha_hash", None)?;
        repo.cifrar(senha)?;
        Ok(repo)
    }

    pub fn cifrado(&self) -> bool {
        self.chave.is_some()
    }

    fn versao(&self) -> Resultado<usize> {
        Ok(self.conn.query_row("PRAGMA user_version", [], |r| r.get(0))?)
    }

    /// Copia o banco inteiro para `destino`, cifrado com `chave` (None = sem criptografia).
    fn exportar_para(&self, destino: &Path, chave: Option<&str>) -> Resultado<()> {
        remover_se_existe(destino)?;
        let versao = self.versao()?;
        let destino_txt = destino.to_string_lossy().to_string();
        self.conn.execute(
            "ATTACH DATABASE ?1 AS destino KEY ?2",
            params![destino_txt, chave.unwrap_or("")],
        )?;
        let resultado = self
            .conn
            .query_row("SELECT sqlcipher_export('destino')", [], |_| Ok(()))
            .and_then(|_| self.conn.execute_batch(&format!("PRAGMA destino.user_version = {versao};")));
        self.conn.execute_batch("DETACH DATABASE destino")?;
        resultado?;
        Ok(())
    }

    /// Substitui o arquivo atual por `novo` e reabre com `chave`.
    /// O arquivo antigo só é apagado depois que o novo abre sem erro.
    fn trocar_arquivo(&mut self, novo: &Path, chave: Option<String>) -> Resultado<()> {
        let caminho = match &self.caminho {
            Some(c) => c.clone(),
            None => return invalido("banco em memória não tem arquivo"),
        };
        let antigo = ao_lado(&caminho, ".anterior");
        let conn_velha = std::mem::replace(&mut self.conn, Connection::open_in_memory()?);
        conn_velha.close().map_err(|(_, e)| e)?;

        remover_se_existe(&antigo)?;
        fs::rename(&caminho, &antigo)?;
        if let Err(e) = fs::rename(novo, &caminho) {
            fs::rename(&antigo, &caminho)?;
            self.conn = conectar(&caminho, self.chave.as_deref())?;
            return Err(e.into());
        }
        match conectar(&caminho, chave.as_deref()) {
            Ok(conn) => {
                conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = DELETE;")?;
                self.conn = conn;
                self.chave = chave;
                remover_se_existe(&antigo)?;
                self.migrar()
            }
            Err(e) => {
                // Volta para o arquivo de antes.
                fs::rename(&caminho, novo)?;
                fs::rename(&antigo, &caminho)?;
                self.conn = conectar(&caminho, self.chave.as_deref())?;
                Err(e)
            }
        }
    }

    fn cifrar(&mut self, senha: &str) -> Resultado<()> {
        let caminho = self.caminho.clone().ok_or_else(|| Erro::Validacao("sem arquivo".into()))?;
        let tmp = ao_lado(&caminho, ".cifrando");
        self.exportar_para(&tmp, Some(senha))?;
        self.trocar_arquivo(&tmp, Some(senha.to_string()))
    }

    fn decifrar(&mut self) -> Resultado<()> {
        let caminho = self.caminho.clone().ok_or_else(|| Erro::Validacao("sem arquivo".into()))?;
        let tmp = ao_lado(&caminho, ".decifrando");
        self.exportar_para(&tmp, None)?;
        self.trocar_arquivo(&tmp, None)
    }

    /// Define, troca ou remove (`nova = None`) a senha que cifra o arquivo.
    /// Se já houver senha, exige a atual.
    pub fn definir_senha(&mut self, atual: Option<&str>, nova: Option<&str>) -> Resultado<()> {
        if let Some(k) = &self.chave {
            if atual != Some(k.as_str()) {
                return invalido("senha atual incorreta");
            }
        }
        if let Some(n) = nova {
            if n.chars().count() < seguranca::TAMANHO_MINIMO {
                return invalido(format!(
                    "a senha precisa ter pelo menos {} caracteres",
                    seguranca::TAMANHO_MINIMO
                ));
            }
        }
        match (self.chave.is_some(), nova) {
            (true, Some(n)) => {
                self.conn.pragma_update(None, "rekey", n)?;
                self.chave = Some(n.to_string());
                Ok(())
            }
            (true, None) => self.decifrar(),
            (false, Some(n)) => self.cifrar(n),
            (false, None) => Ok(()),
        }
    }

    // ---------- Backup ----------

    /// Grava uma cópia completa em `destino`. Com senha, a cópia sai cifrada com ela.
    pub fn fazer_backup(&self, destino: impl AsRef<Path>) -> Resultado<()> {
        let destino = destino.as_ref();
        let tmp = ao_lado(destino, ".parcial");
        self.exportar_para(&tmp, self.chave.as_deref())?;
        remover_se_existe(destino)?;
        fs::rename(&tmp, destino)?;
        Ok(())
    }

    /// Troca todos os dados pelos do backup. `senha_backup` é a senha que o app tinha
    /// quando o backup foi feito. Depois de restaurado, o banco continua com a senha atual.
    pub fn restaurar_backup(&mut self, origem: impl AsRef<Path>, senha_backup: Option<&str>) -> Resultado<()> {
        let origem = origem.as_ref();
        if !origem.exists() {
            return invalido("arquivo de backup não encontrado");
        }
        let chave_backup = senha_backup.filter(|s| !s.is_empty());
        let conn = conectar(origem, chave_backup).map_err(|e| match e {
            Erro::SenhaIncorreta => Erro::Validacao(
                "não consegui abrir o backup: ele tem senha e a informada não confere".into(),
            ),
            outro => outro,
        })?;
        let versao: usize = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        let tem_transacoes = conn
            .query_row("SELECT 1 FROM sqlite_master WHERE name = 'transacao'", [], |_| Ok(()))
            .optional()?
            .is_some();
        if versao == 0 || !tem_transacoes {
            return invalido("esse arquivo não é um backup do Finanças Locais");
        }
        if versao > MIGRACOES.len() {
            return invalido("esse backup é de uma versão mais nova do app; atualize o app primeiro");
        }

        let caminho = self.caminho.clone().ok_or_else(|| Erro::Validacao("sem arquivo".into()))?;
        let tmp = ao_lado(&caminho, ".restaurando");
        let origem_repo = Repositorio { conn, caminho: None, chave: None };
        origem_repo.exportar_para(&tmp, self.chave.as_deref())?;
        drop(origem_repo);
        let chave = self.chave.clone();
        self.trocar_arquivo(&tmp, chave)?;
        // Senha de tela da v0.1 que tenha vindo no backup não vale mais.
        self.gravar_config("senha_hash", None)
    }

    pub fn info_backup(&self) -> Resultado<InfoBackup> {
        Ok(InfoBackup {
            pasta: self.ler_config("pasta_backup")?,
            ultimo: self.ler_config("ultimo_backup")?,
            cifrado: self.cifrado(),
        })
    }

    /// Pasta onde o backup automático é salvo (ex.: dentro do OneDrive ou Google Drive).
    pub fn definir_pasta_backup(&self, pasta: Option<&str>) -> Resultado<()> {
        if let Some(p) = pasta {
            if !Path::new(p).is_dir() {
                return invalido("essa pasta não existe");
            }
        }
        self.gravar_config("pasta_backup", pasta)
    }

    /// Faz o backup automático se houver pasta definida e o último tiver mais de 20 horas
    /// (ou se `forcar`). Mantém só os 10 mais recentes. Devolve o arquivo criado.
    pub fn backup_automatico(&self, forcar: bool) -> Resultado<Option<PathBuf>> {
        let Some(pasta) = self.ler_config("pasta_backup")? else {
            return Ok(None);
        };
        let pasta = PathBuf::from(pasta);
        if !pasta.is_dir() {
            return invalido(format!("a pasta de backup não está acessível: {}", pasta.display()));
        }
        let agora = Local::now().naive_local();
        if !forcar {
            if let Some(ultimo) = self.ler_config("ultimo_backup")? {
                if let Ok(u) = chrono::NaiveDateTime::parse_from_str(&ultimo, "%Y-%m-%d %H:%M") {
                    if (agora - u).num_hours() < HORAS_ENTRE_BACKUPS {
                        return Ok(None);
                    }
                }
            }
        }
        let arquivo = pasta.join(format!("{PREFIXO_BACKUP}{}.db", agora.format("%Y-%m-%d_%H%M%S")));
        self.fazer_backup(&arquivo)?;
        self.gravar_config("ultimo_backup", Some(&agora.format("%Y-%m-%d %H:%M").to_string()))?;

        // Rotação: apaga os mais antigos, só entre os arquivos criados por este app.
        let mut nossos: Vec<PathBuf> = fs::read_dir(&pasta)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with(PREFIXO_BACKUP) && n.ends_with(".db"))
            })
            .collect();
        nossos.sort();
        let excedentes = nossos.len().saturating_sub(BACKUPS_MANTIDOS);
        for antigo in nossos.into_iter().take(excedentes) {
            let _ = fs::remove_file(antigo);
        }
        Ok(Some(arquivo))
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::modelos::*;

    struct Pasta(PathBuf);
    impl Pasta {
        fn nova(nome: &str) -> Self {
            let p = std::env::temp_dir().join(format!("financas-{nome}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            Pasta(p)
        }
        fn arq(&self, n: &str) -> PathBuf {
            self.0.join(n)
        }
    }
    impl Drop for Pasta {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn com_dados(repo: &Repositorio) {
        repo.criar_pessoa("Mãe").unwrap();
    }

    fn pessoas(repo: &Repositorio) -> usize {
        repo.listar_pessoas().unwrap().len()
    }

    #[test]
    fn arquivo_novo_abre_sem_senha_e_reabre_sem_repetir_migracao() {
        let d = Pasta::nova("novo");
        let db = d.arq("f.db");
        assert_eq!(Repositorio::situacao(&db).unwrap(), SituacaoArquivo::Novo);
        com_dados(&Repositorio::abrir(&db, None).unwrap());
        assert_eq!(Repositorio::situacao(&db).unwrap(), SituacaoArquivo::Aberto);
        assert_eq!(pessoas(&Repositorio::abrir(&db, None).unwrap()), 2);
    }

    #[test]
    fn senha_cifra_o_arquivo_troca_e_remove() {
        let d = Pasta::nova("cifra");
        let db = d.arq("f.db");
        let mut repo = Repositorio::abrir(&db, None).unwrap();
        com_dados(&repo);
        repo.definir_senha(None, Some("1234")).unwrap();
        assert!(repo.cifrado());
        assert_eq!(pessoas(&repo), 2, "dados continuam acessíveis depois de cifrar");
        drop(repo);

        // O arquivo não é mais legível sem senha, nem mesmo o cabeçalho do SQLite.
        assert_eq!(Repositorio::situacao(&db).unwrap(), SituacaoArquivo::Cifrado);
        let bytes = fs::read(&db).unwrap();
        assert!(!bytes.starts_with(b"SQLite format 3"));
        assert!(matches!(Repositorio::abrir(&db, Some("errada")), Err(Erro::SenhaIncorreta)));

        let mut repo = Repositorio::abrir(&db, Some("1234")).unwrap();
        assert_eq!(pessoas(&repo), 2);
        assert!(repo.definir_senha(Some("errada"), Some("5678")).is_err());
        repo.definir_senha(Some("1234"), Some("5678")).unwrap();
        drop(repo);
        assert!(Repositorio::abrir(&db, Some("1234")).is_err());

        let mut repo = Repositorio::abrir(&db, Some("5678")).unwrap();
        repo.definir_senha(Some("5678"), None).unwrap();
        drop(repo);
        assert_eq!(Repositorio::situacao(&db).unwrap(), SituacaoArquivo::Aberto);
        assert_eq!(pessoas(&Repositorio::abrir(&db, None).unwrap()), 2);
    }

    #[test]
    fn banco_da_v01_com_senha_de_tela_vira_cifrado() {
        let d = Pasta::nova("legado");
        let db = d.arq("f.db");
        let repo = Repositorio::abrir(&db, None).unwrap();
        com_dados(&repo);
        let hash = seguranca::gerar_hash("abcd").unwrap();
        repo.gravar_config("senha_hash", Some(&hash)).unwrap();
        drop(repo);

        assert_eq!(Repositorio::situacao(&db).unwrap(), SituacaoArquivo::LegadoComSenha);
        assert!(Repositorio::abrir_legado(&db, "errada").is_err());
        let repo = Repositorio::abrir_legado(&db, "abcd").unwrap();
        assert!(repo.cifrado());
        drop(repo);
        assert_eq!(Repositorio::situacao(&db).unwrap(), SituacaoArquivo::Cifrado);
        assert_eq!(pessoas(&Repositorio::abrir(&db, Some("abcd")).unwrap()), 2);
    }

    #[test]
    fn backup_e_restauracao_com_e_sem_senha() {
        let d = Pasta::nova("backup");
        let db = d.arq("f.db");
        let mut repo = Repositorio::abrir(&db, None).unwrap();
        com_dados(&repo);
        repo.definir_senha(None, Some("1234")).unwrap();
        let bkp = d.arq("copia.db");
        repo.fazer_backup(&bkp).unwrap();
        assert_eq!(Repositorio::situacao(&bkp).unwrap(), SituacaoArquivo::Cifrado);

        // Mexe nos dados e depois volta o backup.
        repo.criar_pessoa("Avó").unwrap();
        assert_eq!(pessoas(&repo), 3);
        assert!(repo.restaurar_backup(&bkp, Some("errada")).is_err());
        repo.restaurar_backup(&bkp, Some("1234")).unwrap();
        assert_eq!(pessoas(&repo), 2);

        // Backup antigo sem senha restaurado num app com senha continua cifrado.
        let sem_senha = d.arq("antigo.db");
        com_dados(&Repositorio::abrir(&sem_senha, None).unwrap());
        Repositorio::abrir(&sem_senha, None).unwrap().criar_pessoa("Tio").unwrap();
        repo.restaurar_backup(&sem_senha, None).unwrap();
        assert_eq!(pessoas(&repo), 3);
        drop(repo);
        assert_eq!(Repositorio::situacao(&db).unwrap(), SituacaoArquivo::Cifrado);

        // Arquivo qualquer é recusado.
        let lixo = d.arq("lixo.db");
        fs::write(&lixo, b"nada a ver").unwrap();
        let mut repo = Repositorio::abrir(&db, Some("1234")).unwrap();
        assert!(repo.restaurar_backup(&lixo, None).is_err());
        assert_eq!(pessoas(&repo), 3);
    }

    #[test]
    fn backup_automatico_respeita_intervalo_e_rotacao() {
        let d = Pasta::nova("auto");
        let pasta = d.arq("nuvem");
        fs::create_dir_all(&pasta).unwrap();
        let repo = Repositorio::abrir(d.arq("f.db"), None).unwrap();
        assert!(repo.backup_automatico(false).unwrap().is_none(), "sem pasta, não faz nada");
        repo.definir_pasta_backup(Some(pasta.to_str().unwrap())).unwrap();
        assert!(repo.backup_automatico(false).unwrap().is_some());
        assert!(repo.backup_automatico(false).unwrap().is_none(), "menos de 20h depois, não repete");

        // Arquivos antigos do app são rodados; outros arquivos da pasta ficam.
        for i in 0..12 {
            fs::write(pasta.join(format!("{PREFIXO_BACKUP}2020-01-{:02}_000000.db", i + 1)), b"x").unwrap();
        }
        fs::write(pasta.join("meu-arquivo.txt"), b"x").unwrap();
        repo.backup_automatico(true).unwrap();
        let nomes: Vec<String> = fs::read_dir(&pasta)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(nomes.iter().filter(|n| n.starts_with(PREFIXO_BACKUP)).count(), BACKUPS_MANTIDOS);
        assert!(nomes.contains(&"meu-arquivo.txt".to_string()));
        assert!(repo.definir_pasta_backup(Some("/nao/existe/mesmo")).is_err());
        let _ = TipoConta::Cartao; // mantém o import de modelos usado
    }
}
