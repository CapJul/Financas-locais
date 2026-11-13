//! Persistência em SQLite. Um único arquivo `.db` guarda tudo.

use std::collections::BTreeMap;
use std::path::Path;

use chrono::Datelike;
use rusqlite::{params, Connection, ErrorCode, OptionalExtension};

use crate::competencia::Competencia;
use crate::dominio::{self, ratear};
use crate::erro::{invalido, Erro, Resultado};
use crate::modelos::*;
use crate::seguranca;

/// Pessoa padrão: recebe a compra inteira quando não há divisão.
pub const PESSOA_EU: i64 = 1;

const MIGRACOES: &[&str] = &[
    // v1 — esquema inicial
    r#"
    CREATE TABLE pessoa (
        id   INTEGER PRIMARY KEY,
        nome TEXT NOT NULL UNIQUE
    );
    CREATE TABLE conta (
        id             INTEGER PRIMARY KEY,
        nome           TEXT NOT NULL UNIQUE,
        tipo           TEXT NOT NULL CHECK (tipo IN ('corrente', 'cartao')),
        dia_fechamento INTEGER CHECK (dia_fechamento BETWEEN 1 AND 31),
        dia_vencimento INTEGER CHECK (dia_vencimento BETWEEN 1 AND 31)
    );
    CREATE TABLE categoria (
        id   INTEGER PRIMARY KEY,
        nome TEXT NOT NULL UNIQUE,
        cor  TEXT NOT NULL DEFAULT '#888888'
    );
    CREATE TABLE transacao (
        id              INTEGER PRIMARY KEY,
        conta_id        INTEGER NOT NULL REFERENCES conta(id) ON DELETE RESTRICT,
        descricao       TEXT NOT NULL,
        data_compra     TEXT NOT NULL,
        valor_total     INTEGER NOT NULL CHECK (valor_total > 0),
        num_parcelas    INTEGER NOT NULL CHECK (num_parcelas >= 1),
        categoria_id    INTEGER REFERENCES categoria(id) ON DELETE SET NULL,
        origem          TEXT NOT NULL DEFAULT 'manual' CHECK (origem IN ('manual', 'importado')),
        hash_importacao TEXT UNIQUE
    );
    CREATE TABLE parcela (
        id           INTEGER PRIMARY KEY,
        transacao_id INTEGER NOT NULL REFERENCES transacao(id) ON DELETE CASCADE,
        numero       INTEGER NOT NULL,
        valor        INTEGER NOT NULL,
        competencia  TEXT NOT NULL,
        UNIQUE (transacao_id, numero)
    );
    CREATE INDEX idx_parcela_competencia ON parcela(competencia);
    CREATE TABLE divisao (
        id           INTEGER PRIMARY KEY,
        transacao_id INTEGER NOT NULL REFERENCES transacao(id) ON DELETE CASCADE,
        pessoa_id    INTEGER NOT NULL REFERENCES pessoa(id) ON DELETE RESTRICT,
        valor        INTEGER NOT NULL CHECK (valor > 0),
        UNIQUE (transacao_id, pessoa_id)
    );
    CREATE TABLE assinatura (
        id           INTEGER PRIMARY KEY,
        descricao    TEXT NOT NULL,
        valor        INTEGER NOT NULL CHECK (valor > 0),
        conta_id     INTEGER NOT NULL REFERENCES conta(id) ON DELETE CASCADE,
        dia_cobranca INTEGER NOT NULL CHECK (dia_cobranca BETWEEN 1 AND 31),
        categoria_id INTEGER REFERENCES categoria(id) ON DELETE SET NULL,
        ativa        INTEGER NOT NULL DEFAULT 1
    );
    INSERT INTO pessoa (id, nome) VALUES (1, 'Eu');
    INSERT INTO categoria (nome, cor) VALUES
        ('Carro', '#4e79a7'),
        ('Comida', '#f28e2b'),
        ('Relacionamento', '#e15759'),
        ('Lazer', '#59a14f');
    "#,
    // v2 — mensalidades com dono e período, entradas e configurações (senha)
    r#"
    ALTER TABLE assinatura ADD COLUMN pessoa_id INTEGER REFERENCES pessoa(id) ON DELETE RESTRICT;
    ALTER TABLE assinatura ADD COLUMN inicio TEXT NOT NULL DEFAULT '2000-01';
    ALTER TABLE assinatura ADD COLUMN fim TEXT;
    CREATE TABLE entrada (
        id        INTEGER PRIMARY KEY,
        conta_id  INTEGER REFERENCES conta(id) ON DELETE RESTRICT,
        descricao TEXT NOT NULL,
        data      TEXT NOT NULL,
        valor     INTEGER NOT NULL CHECK (valor > 0),
        tipo      TEXT NOT NULL CHECK (tipo IN ('pix', 'salario', 'pagamento', 'reembolso', 'outro')),
        pessoa_id INTEGER REFERENCES pessoa(id) ON DELETE RESTRICT
    );
    CREATE INDEX idx_entrada_data ON entrada(data);
    CREATE TABLE config (
        chave TEXT PRIMARY KEY,
        valor TEXT NOT NULL
    );
    "#,
];

/// Mensalidade já com o deslocamento de fatura calculado.
struct MensalidadeInterna {
    conta_id: i64,
    valor: i64,
    categoria: String,
    pessoa_id: i64,
    inicio: Competencia,
    fim: Option<Competencia>,
    deslocamento: i32,
}

impl MensalidadeInterna {
    fn cai_em(&self, c: Competencia) -> bool {
        dominio::mensalidade_cai_em(self.inicio, self.fim, self.deslocamento, c)
    }
}

/// Acumula totais por nome e devolve do maior para o menor.
#[derive(Default)]
struct Somador(BTreeMap<String, i64>);

impl Somador {
    fn somar(&mut self, nome: impl Into<String>, valor: i64) {
        *self.0.entry(nome.into()).or_default() += valor;
    }
    fn ordenado(self) -> Vec<TotalNomeado> {
        let mut v: Vec<TotalNomeado> =
            self.0.into_iter().map(|(nome, valor)| TotalNomeado { nome, valor }).collect();
        v.sort_by(|a, b| b.valor.cmp(&a.valor).then(a.nome.cmp(&b.nome)));
        v
    }
}

pub struct Repositorio {
    conn: Connection,
}

/// Traduz violações de restrição do SQLite em mensagens para o usuário.
fn traduzir(e: rusqlite::Error, em_uso: &str, duplicado: &str) -> Erro {
    if let rusqlite::Error::SqliteFailure(f, ref msg) = e {
        if f.code == ErrorCode::ConstraintViolation {
            let m = msg.as_deref().unwrap_or("");
            if m.contains("UNIQUE") {
                return Erro::Validacao(duplicado.into());
            }
            if m.contains("FOREIGN KEY") {
                return Erro::Validacao(em_uso.into());
            }
        }
    }
    Erro::Banco(e)
}

fn texto_obrigatorio(valor: &str, campo: &str) -> Resultado<String> {
    let v = valor.trim();
    if v.is_empty() {
        return invalido(format!("{campo} não pode ficar vazio"));
    }
    Ok(v.to_string())
}

impl Repositorio {
    pub fn abrir(caminho: impl AsRef<Path>) -> Resultado<Self> {
        Self::preparar(Connection::open(caminho)?)
    }

    pub fn em_memoria() -> Resultado<Self> {
        Self::preparar(Connection::open_in_memory()?)
    }

    fn preparar(conn: Connection) -> Resultado<Self> {
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
        let mut repo = Self { conn };
        repo.migrar()?;
        Ok(repo)
    }

    /// Aplica as migrações pendentes, usando `PRAGMA user_version` como contador.
    fn migrar(&mut self) -> Resultado<()> {
        let versao: usize = self.conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        for (i, sql) in MIGRACOES.iter().enumerate().skip(versao) {
            let tx = self.conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.execute_batch(&format!("PRAGMA user_version = {}", i + 1))?;
            tx.commit()?;
        }
        Ok(())
    }

    // ---------- Pessoas ----------

    pub fn listar_pessoas(&self) -> Resultado<Vec<Pessoa>> {
        let mut st = self.conn.prepare("SELECT id, nome FROM pessoa ORDER BY id")?;
        let linhas = st.query_map([], |r| Ok(Pessoa { id: r.get(0)?, nome: r.get(1)? }))?;
        Ok(linhas.collect::<Result<_, _>>()?)
    }

    pub fn criar_pessoa(&self, nome: &str) -> Resultado<Pessoa> {
        let nome = texto_obrigatorio(nome, "o nome")?;
        self.conn
            .execute("INSERT INTO pessoa (nome) VALUES (?1)", [&nome])
            .map_err(|e| traduzir(e, "", "já existe uma pessoa com esse nome"))?;
        Ok(Pessoa { id: self.conn.last_insert_rowid(), nome })
    }

    pub fn excluir_pessoa(&self, id: i64) -> Resultado<()> {
        if id == PESSOA_EU {
            return invalido("a pessoa \"Eu\" não pode ser excluída");
        }
        let n = self
            .conn
            .execute("DELETE FROM pessoa WHERE id = ?1", [id])
            .map_err(|e| traduzir(e, "essa pessoa aparece em compras divididas", ""))?;
        if n == 0 {
            return Err(Erro::NaoEncontrado(format!("pessoa {id}")));
        }
        Ok(())
    }

    // ---------- Contas ----------

    fn ler_conta(r: &rusqlite::Row) -> rusqlite::Result<Conta> {
        let tipo: String = r.get(2)?;
        Ok(Conta {
            id: r.get(0)?,
            nome: r.get(1)?,
            tipo: TipoConta::de_texto(&tipo),
            dia_fechamento: r.get(3)?,
            dia_vencimento: r.get(4)?,
        })
    }

    pub fn listar_contas(&self) -> Resultado<Vec<Conta>> {
        let mut st = self.conn.prepare(
            "SELECT id, nome, tipo, dia_fechamento, dia_vencimento FROM conta ORDER BY nome",
        )?;
        let linhas = st.query_map([], Self::ler_conta)?;
        Ok(linhas.collect::<Result<_, _>>()?)
    }

    pub fn buscar_conta(&self, id: i64) -> Resultado<Conta> {
        self.conn
            .query_row(
                "SELECT id, nome, tipo, dia_fechamento, dia_vencimento FROM conta WHERE id = ?1",
                [id],
                Self::ler_conta,
            )
            .optional()?
            .ok_or_else(|| Erro::NaoEncontrado(format!("conta {id}")))
    }

    pub fn criar_conta(&self, nova: NovaConta) -> Resultado<Conta> {
        let nome = texto_obrigatorio(&nova.nome, "o nome da conta")?;
        let (fechamento, vencimento) = match nova.tipo {
            TipoConta::Cartao => {
                let valido = |d: Option<u32>| d.filter(|d| (1..=31).contains(d));
                match (valido(nova.dia_fechamento), valido(nova.dia_vencimento)) {
                    (Some(f), Some(v)) => (Some(f), Some(v)),
                    _ => return invalido("cartão precisa de dia de fechamento e de vencimento (1 a 31)"),
                }
            }
            TipoConta::Corrente => (None, None),
        };
        self.conn
            .execute(
                "INSERT INTO conta (nome, tipo, dia_fechamento, dia_vencimento) VALUES (?1, ?2, ?3, ?4)",
                params![nome, nova.tipo.como_texto(), fechamento, vencimento],
            )
            .map_err(|e| traduzir(e, "", "já existe uma conta com esse nome"))?;
        Ok(Conta {
            id: self.conn.last_insert_rowid(),
            nome,
            tipo: nova.tipo,
            dia_fechamento: fechamento,
            dia_vencimento: vencimento,
        })
    }

    pub fn excluir_conta(&self, id: i64) -> Resultado<()> {
        let n = self
            .conn
            .execute("DELETE FROM conta WHERE id = ?1", [id])
            .map_err(|e| traduzir(e, "essa conta tem compras lançadas; exclua as compras antes", ""))?;
        if n == 0 {
            return Err(Erro::NaoEncontrado(format!("conta {id}")));
        }
        Ok(())
    }

    // ---------- Categorias ----------

    pub fn listar_categorias(&self) -> Resultado<Vec<Categoria>> {
        let mut st = self.conn.prepare("SELECT id, nome, cor FROM categoria ORDER BY nome")?;
        let linhas = st.query_map([], |r| {
            Ok(Categoria { id: r.get(0)?, nome: r.get(1)?, cor: r.get(2)? })
        })?;
        Ok(linhas.collect::<Result<_, _>>()?)
    }

    pub fn criar_categoria(&self, nome: &str, cor: &str) -> Resultado<Categoria> {
        let nome = texto_obrigatorio(nome, "o nome da categoria")?;
        let cor = if cor.trim().is_empty() { "#888888".to_string() } else { cor.trim().to_string() };
        self.conn
            .execute("INSERT INTO categoria (nome, cor) VALUES (?1, ?2)", params![nome, cor])
            .map_err(|e| traduzir(e, "", "já existe uma categoria com esse nome"))?;
        Ok(Categoria { id: self.conn.last_insert_rowid(), nome, cor })
    }

    pub fn excluir_categoria(&self, id: i64) -> Resultado<()> {
        let n = self.conn.execute("DELETE FROM categoria WHERE id = ?1", [id])?;
        if n == 0 {
            return Err(Erro::NaoEncontrado(format!("categoria {id}")));
        }
        Ok(())
    }

    // ---------- Transações ----------

    pub fn criar_transacao(&mut self, nova: NovaTransacao) -> Resultado<Transacao> {
        let descricao = texto_obrigatorio(&nova.descricao, "a descrição")?;
        let data = dominio::ler_data(&nova.data_compra)?;
        let conta = self.buscar_conta(nova.conta_id)?;
        let parcelas =
            dominio::gerar_parcelas(data, nova.valor_total, nova.num_parcelas, conta.dia_fechamento)?;

        let divisoes = if nova.divisoes.is_empty() {
            vec![Divisao { pessoa_id: PESSOA_EU, valor: nova.valor_total }]
        } else {
            nova.divisoes.clone()
        };
        let pares: Vec<(i64, i64)> = divisoes.iter().map(|d| (d.pessoa_id, d.valor)).collect();
        dominio::validar_divisoes(nova.valor_total, &pares)?;

        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO transacao (conta_id, descricao, data_compra, valor_total, num_parcelas, categoria_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![conta.id, descricao, nova.data_compra, nova.valor_total, nova.num_parcelas, nova.categoria_id],
        )
        .map_err(|e| traduzir(e, "categoria inexistente", ""))?;
        let id = tx.last_insert_rowid();
        for p in &parcelas {
            tx.execute(
                "INSERT INTO parcela (transacao_id, numero, valor, competencia) VALUES (?1, ?2, ?3, ?4)",
                params![id, p.numero, p.valor, p.competencia.to_string()],
            )?;
        }
        for d in &divisoes {
            tx.execute(
                "INSERT INTO divisao (transacao_id, pessoa_id, valor) VALUES (?1, ?2, ?3)",
                params![id, d.pessoa_id, d.valor],
            )
            .map_err(|e| traduzir(e, "pessoa inexistente na divisão", ""))?;
        }
        tx.commit()?;
        self.buscar_transacao(id)
    }

    fn divisoes_de(&self, transacao_id: i64) -> Resultado<Vec<Divisao>> {
        let mut st = self
            .conn
            .prepare("SELECT pessoa_id, valor FROM divisao WHERE transacao_id = ?1 ORDER BY id")?;
        let linhas = st.query_map([transacao_id], |r| {
            Ok(Divisao { pessoa_id: r.get(0)?, valor: r.get(1)? })
        })?;
        Ok(linhas.collect::<Result<_, _>>()?)
    }

    const SELECT_TRANSACAO: &'static str =
        "SELECT t.id, t.conta_id, c.nome, t.descricao, t.data_compra, t.valor_total,
                t.num_parcelas, t.categoria_id, cat.nome
         FROM transacao t
         JOIN conta c ON c.id = t.conta_id
         LEFT JOIN categoria cat ON cat.id = t.categoria_id";

    fn ler_transacao(r: &rusqlite::Row) -> rusqlite::Result<Transacao> {
        Ok(Transacao {
            id: r.get(0)?,
            conta_id: r.get(1)?,
            conta_nome: r.get(2)?,
            descricao: r.get(3)?,
            data_compra: r.get(4)?,
            valor_total: r.get(5)?,
            num_parcelas: r.get(6)?,
            categoria_id: r.get(7)?,
            categoria_nome: r.get(8)?,
            divisoes: Vec::new(),
        })
    }

    pub fn buscar_transacao(&self, id: i64) -> Resultado<Transacao> {
        let sql = format!("{} WHERE t.id = ?1", Self::SELECT_TRANSACAO);
        let mut t = self
            .conn
            .query_row(&sql, [id], Self::ler_transacao)
            .optional()?
            .ok_or_else(|| Erro::NaoEncontrado(format!("transação {id}")))?;
        t.divisoes = self.divisoes_de(id)?;
        Ok(t)
    }

    /// Todas as compras, das mais recentes para as mais antigas.
    pub fn listar_transacoes(&self) -> Resultado<Vec<Transacao>> {
        let sql = format!("{} ORDER BY t.data_compra DESC, t.id DESC", Self::SELECT_TRANSACAO);
        let mut st = self.conn.prepare(&sql)?;
        let mut lista: Vec<Transacao> =
            st.query_map([], Self::ler_transacao)?.collect::<Result<_, _>>()?;
        for t in &mut lista {
            t.divisoes = self.divisoes_de(t.id)?;
        }
        Ok(lista)
    }

    pub fn excluir_transacao(&self, id: i64) -> Resultado<()> {
        let n = self.conn.execute("DELETE FROM transacao WHERE id = ?1", [id])?;
        if n == 0 {
            return Err(Erro::NaoEncontrado(format!("transação {id}")));
        }
        Ok(())
    }

    /// Parcelas que caem numa competência (todas as contas).
    pub fn lancamentos_do_mes(&self, competencia: Competencia) -> Resultado<Vec<LancamentoDoMes>> {
        let mut st = self.conn.prepare(
            "SELECT t.id, t.descricao, c.nome, cat.nome, p.numero, t.num_parcelas, p.valor, t.data_compra
             FROM parcela p
             JOIN transacao t ON t.id = p.transacao_id
             JOIN conta c ON c.id = t.conta_id
             LEFT JOIN categoria cat ON cat.id = t.categoria_id
             WHERE p.competencia = ?1
             ORDER BY t.data_compra DESC, t.id DESC",
        )?;
        let linhas = st.query_map([competencia.to_string()], |r| {
            Ok(LancamentoDoMes {
                transacao_id: r.get(0)?,
                descricao: r.get(1)?,
                conta_nome: r.get(2)?,
                categoria_nome: r.get(3)?,
                numero: r.get(4)?,
                num_parcelas: r.get(5)?,
                valor: r.get(6)?,
                data_compra: r.get(7)?,
            })
        })?;
        Ok(linhas.collect::<Result<_, _>>()?)
    }

    // ---------- Mensalidades ----------

    pub fn criar_mensalidade(&self, nova: NovaMensalidade) -> Resultado<Mensalidade> {
        let descricao = texto_obrigatorio(&nova.descricao, "a descrição")?;
        if nova.valor <= 0 {
            return invalido("o valor precisa ser maior que zero");
        }
        let data = dominio::ler_data(&nova.primeira_cobranca)?;
        self.buscar_conta(nova.conta_id)?;
        let pessoa = nova.pessoa_id.unwrap_or(PESSOA_EU);
        self.conn
            .execute(
                "INSERT INTO assinatura (descricao, valor, conta_id, dia_cobranca, categoria_id, ativa, pessoa_id, inicio)
                 VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7)",
                params![
                    descricao,
                    nova.valor,
                    nova.conta_id,
                    data.day(),
                    nova.categoria_id,
                    pessoa,
                    Competencia::de_data(data).to_string()
                ],
            )
            .map_err(|e| traduzir(e, "pessoa ou categoria inexistente", ""))?;
        self.buscar_mensalidade(self.conn.last_insert_rowid())
    }

    const SELECT_MENSALIDADE: &'static str =
        "SELECT a.id, a.conta_id, c.nome, a.descricao, a.valor, a.dia_cobranca, a.categoria_id,
                cat.nome, COALESCE(a.pessoa_id, 1), COALESCE(p.nome, 'Eu'), a.inicio, a.fim
         FROM assinatura a
         JOIN conta c ON c.id = a.conta_id
         LEFT JOIN categoria cat ON cat.id = a.categoria_id
         LEFT JOIN pessoa p ON p.id = a.pessoa_id";

    fn ler_mensalidade(r: &rusqlite::Row) -> rusqlite::Result<(Mensalidade, String, Option<String>)> {
        Ok((
            Mensalidade {
                id: r.get(0)?,
                conta_id: r.get(1)?,
                conta_nome: r.get(2)?,
                descricao: r.get(3)?,
                valor: r.get(4)?,
                dia_cobranca: r.get(5)?,
                categoria_id: r.get(6)?,
                categoria_nome: r.get(7)?,
                pessoa_id: r.get(8)?,
                pessoa_nome: r.get(9)?,
                inicio: Competencia { ano: 2000, mes: 1 },
                fim: None,
            },
            r.get(10)?,
            r.get(11)?,
        ))
    }

    fn montar_mensalidade(
        (mut m, inicio, fim): (Mensalidade, String, Option<String>),
    ) -> Resultado<Mensalidade> {
        m.inicio = inicio.parse()?;
        m.fim = fim.map(|f| f.parse()).transpose()?;
        Ok(m)
    }

    pub fn buscar_mensalidade(&self, id: i64) -> Resultado<Mensalidade> {
        let sql = format!("{} WHERE a.id = ?1", Self::SELECT_MENSALIDADE);
        let linha = self
            .conn
            .query_row(&sql, [id], Self::ler_mensalidade)
            .optional()?
            .ok_or_else(|| Erro::NaoEncontrado(format!("mensalidade {id}")))?;
        Self::montar_mensalidade(linha)
    }

    /// Ativas primeiro, depois as canceladas.
    pub fn listar_mensalidades(&self) -> Resultado<Vec<Mensalidade>> {
        let sql = format!(
            "{} ORDER BY a.fim IS NOT NULL, a.descricao COLLATE NOCASE",
            Self::SELECT_MENSALIDADE
        );
        let mut st = self.conn.prepare(&sql)?;
        let linhas: Vec<_> = st.query_map([], Self::ler_mensalidade)?.collect::<Result<_, _>>()?;
        linhas.into_iter().map(Self::montar_mensalidade).collect()
    }

    /// Encerra a mensalidade: `ultimo_mes` é o último mês em que ela foi cobrada.
    pub fn cancelar_mensalidade(&self, id: i64, ultimo_mes: Competencia) -> Resultado<Mensalidade> {
        let m = self.buscar_mensalidade(id)?;
        if ultimo_mes < m.inicio {
            return invalido("o último mês não pode ser antes da primeira cobrança");
        }
        self.conn.execute(
            "UPDATE assinatura SET fim = ?1, ativa = 0 WHERE id = ?2",
            params![ultimo_mes.to_string(), id],
        )?;
        self.buscar_mensalidade(id)
    }

    pub fn reativar_mensalidade(&self, id: i64) -> Resultado<Mensalidade> {
        self.buscar_mensalidade(id)?;
        self.conn.execute("UPDATE assinatura SET fim = NULL, ativa = 1 WHERE id = ?1", [id])?;
        self.buscar_mensalidade(id)
    }

    pub fn excluir_mensalidade(&self, id: i64) -> Resultado<()> {
        let n = self.conn.execute("DELETE FROM assinatura WHERE id = ?1", [id])?;
        if n == 0 {
            return Err(Erro::NaoEncontrado(format!("mensalidade {id}")));
        }
        Ok(())
    }

    /// Todas as mensalidades, com o deslocamento de fatura de cada conta já calculado.
    fn mensalidades_internas(&self) -> Resultado<Vec<MensalidadeInterna>> {
        let mut st = self.conn.prepare(
            "SELECT a.conta_id, a.valor, COALESCE(cat.nome, 'Sem categoria'), COALESCE(a.pessoa_id, 1),
                    a.inicio, a.fim, a.dia_cobranca, c.dia_fechamento
             FROM assinatura a
             JOIN conta c ON c.id = a.conta_id
             LEFT JOIN categoria cat ON cat.id = a.categoria_id",
        )?;
        type Linha = (i64, i64, String, i64, String, Option<String>, u32, Option<u32>);
        let linhas: Vec<Linha> = st
            .query_map([], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?))
            })?
            .collect::<Result<_, _>>()?;
        linhas
            .into_iter()
            .map(|(conta_id, valor, categoria, pessoa_id, inicio, fim, dia, fechamento)| {
                Ok(MensalidadeInterna {
                    conta_id,
                    valor,
                    categoria,
                    pessoa_id,
                    inicio: inicio.parse()?,
                    fim: fim.map(|f| f.parse()).transpose()?,
                    deslocamento: dominio::deslocamento_cobranca(dia, fechamento),
                })
            })
            .collect()
    }

    // ---------- Entradas ----------

    pub fn criar_entrada(&self, nova: NovaEntrada) -> Resultado<Entrada> {
        let descricao = texto_obrigatorio(&nova.descricao, "a descrição")?;
        if nova.valor <= 0 {
            return invalido("o valor precisa ser maior que zero");
        }
        dominio::ler_data(&nova.data)?;
        if let Some(c) = nova.conta_id {
            self.buscar_conta(c)?;
        }
        self.conn
            .execute(
                "INSERT INTO entrada (conta_id, descricao, data, valor, tipo, pessoa_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![nova.conta_id, descricao, nova.data, nova.valor, nova.tipo.como_texto(), nova.pessoa_id],
            )
            .map_err(|e| traduzir(e, "pessoa inexistente", ""))?;
        let id = self.conn.last_insert_rowid();
        self.listar_entradas_onde("e.id = ?1", params![id])?
            .pop()
            .ok_or_else(|| Erro::NaoEncontrado(format!("entrada {id}")))
    }

    fn listar_entradas_onde(&self, filtro: &str, p: impl rusqlite::Params) -> Resultado<Vec<Entrada>> {
        let sql = format!(
            "SELECT e.id, e.conta_id, c.nome, e.descricao, e.data, e.valor, e.tipo, e.pessoa_id, p.nome
             FROM entrada e
             LEFT JOIN conta c ON c.id = e.conta_id
             LEFT JOIN pessoa p ON p.id = e.pessoa_id
             WHERE {filtro}
             ORDER BY e.data DESC, e.id DESC"
        );
        let mut st = self.conn.prepare(&sql)?;
        let linhas = st.query_map(p, |r| {
            let tipo: String = r.get(6)?;
            Ok(Entrada {
                id: r.get(0)?,
                conta_id: r.get(1)?,
                conta_nome: r.get(2)?,
                descricao: r.get(3)?,
                data: r.get(4)?,
                valor: r.get(5)?,
                tipo: TipoEntrada::de_texto(&tipo),
                pessoa_id: r.get(7)?,
                pessoa_nome: r.get(8)?,
            })
        })?;
        Ok(linhas.collect::<Result<_, _>>()?)
    }

    pub fn listar_entradas(&self) -> Resultado<Vec<Entrada>> {
        self.listar_entradas_onde("1 = 1", [])
    }

    /// Entradas cuja data cai no mês (calendário) da competência.
    pub fn entradas_do_mes(&self, competencia: Competencia) -> Resultado<Vec<Entrada>> {
        self.listar_entradas_onde("substr(e.data, 1, 7) = ?1", [competencia.to_string()])
    }

    pub fn excluir_entrada(&self, id: i64) -> Resultado<()> {
        let n = self.conn.execute("DELETE FROM entrada WHERE id = ?1", [id])?;
        if n == 0 {
            return Err(Erro::NaoEncontrado(format!("entrada {id}")));
        }
        Ok(())
    }

    // ---------- Projeção e simulação ----------

    fn parcelas_da_conta(
        &self,
        conta_id: i64,
        inicio: Competencia,
        fim: Competencia,
    ) -> Resultado<Vec<(Competencia, i64)>> {
        let mut st = self.conn.prepare(
            "SELECT p.competencia, p.valor FROM parcela p
             JOIN transacao t ON t.id = p.transacao_id
             WHERE t.conta_id = ?1 AND p.competencia BETWEEN ?2 AND ?3",
        )?;
        let linhas = st.query_map(params![conta_id, inicio.to_string(), fim.to_string()], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })?;
        let mut saida = Vec::new();
        for l in linhas {
            let (c, v) = l?;
            saida.push((c.parse()?, v));
        }
        Ok(saida)
    }

    fn validar_meses(meses: u32) -> Resultado<()> {
        if meses == 0 || meses > 36 {
            return invalido("a projeção vai de 1 a 36 meses");
        }
        Ok(())
    }

    /// Total de mensalidades de uma conta que cai na competência.
    fn mensalidades_da_conta(mens: &[MensalidadeInterna], conta_id: i64, c: Competencia) -> i64 {
        mens.iter().filter(|m| m.conta_id == conta_id && m.cai_em(c)).map(|m| m.valor).sum()
    }

    /// Regra 4: faturas projetadas de uma conta a partir de `inicio`.
    pub fn projetar_faturas(
        &self,
        conta_id: i64,
        inicio: Competencia,
        meses: u32,
    ) -> Resultado<Vec<FaturaMes>> {
        Self::validar_meses(meses)?;
        self.buscar_conta(conta_id)?;
        let fim = inicio.somar_meses(meses as i32 - 1);
        let lanc = self.parcelas_da_conta(conta_id, inicio, fim)?;
        let mens = self.mensalidades_internas()?;
        Ok(dominio::projetar(&lanc, |c| Self::mensalidades_da_conta(&mens, conta_id, c), inicio, meses))
    }

    /// Composição de cada fatura projetada (por categoria, pessoa e origem).
    pub fn detalhar_faturas(
        &self,
        conta_id: i64,
        inicio: Competencia,
        meses: u32,
    ) -> Resultado<Vec<DetalheMes>> {
        Self::validar_meses(meses)?;
        self.buscar_conta(conta_id)?;
        let mens = self.mensalidades_internas()?;
        let pessoas = self.nomes_pessoas()?;
        (0..meses as i32)
            .map(|i| self.detalhar(Some(conta_id), inicio.somar_meses(i), &mens, &pessoas))
            .collect()
    }

    /// Regra 5: mostra as faturas antes e depois de uma compra hipotética, sem gravar nada.
    pub fn simular_compra(
        &self,
        sim: &SimulacaoCompra,
        inicio: Competencia,
        meses: u32,
    ) -> Resultado<Vec<ComparacaoMes>> {
        Self::validar_meses(meses)?;
        let conta = self.buscar_conta(sim.conta_id)?;
        let data = dominio::ler_data(&sim.data_compra)?;
        let novas =
            dominio::gerar_parcelas(data, sim.valor_total, sim.num_parcelas, conta.dia_fechamento)?;

        let fim = inicio.somar_meses(meses as i32 - 1);
        let mut lanc = self.parcelas_da_conta(conta.id, inicio, fim)?;
        let mens = self.mensalidades_internas()?;
        let recorrente = |c| Self::mensalidades_da_conta(&mens, conta.id, c);
        let atual = dominio::projetar(&lanc, recorrente, inicio, meses);
        lanc.extend(novas.iter().map(|p| (p.competencia, p.valor)));
        let simulado = dominio::projetar(&lanc, recorrente, inicio, meses);

        Ok(atual
            .into_iter()
            .zip(simulado)
            .map(|(a, s)| ComparacaoMes { competencia: a.competencia, atual: a.valor, simulado: s.valor })
            .collect())
    }

    // ---------- Detalhe e resumo do mês ----------

    fn nomes_pessoas(&self) -> Resultado<BTreeMap<i64, String>> {
        Ok(self.listar_pessoas()?.into_iter().map(|p| (p.id, p.nome)).collect())
    }

    /// Gastos de uma competência (de uma conta ou de todas), por categoria, pessoa e origem.
    /// Cada parcela é repartida entre as pessoas na mesma proporção da divisão da compra.
    fn detalhar(
        &self,
        conta_id: Option<i64>,
        competencia: Competencia,
        mens: &[MensalidadeInterna],
        pessoas: &BTreeMap<i64, String>,
    ) -> Resultado<DetalheMes> {
        let nome_pessoa = |id: i64| pessoas.get(&id).cloned().unwrap_or_else(|| format!("#{id}"));
        let mut por_categoria = Somador::default();
        let mut por_pessoa = Somador::default();
        let mut por_origem = Somador::default();
        let mut total = 0;

        let mut st = self.conn.prepare(
            "SELECT p.transacao_id, p.valor, COALESCE(cat.nome, 'Sem categoria'), t.num_parcelas
             FROM parcela p
             JOIN transacao t ON t.id = p.transacao_id
             LEFT JOIN categoria cat ON cat.id = t.categoria_id
             WHERE p.competencia = ?1 AND (?2 IS NULL OR t.conta_id = ?2)",
        )?;
        let parcelas: Vec<(i64, i64, String, u32)> = st
            .query_map(params![competencia.to_string(), conta_id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })?
            .collect::<Result<_, _>>()?;

        for (transacao_id, valor, categoria, num_parcelas) in parcelas {
            total += valor;
            por_categoria.somar(categoria, valor);
            por_origem.somar(if num_parcelas > 1 { "Parcelado" } else { "À vista" }, valor);
            let divisoes = self.divisoes_de(transacao_id)?;
            let pesos: Vec<i64> = divisoes.iter().map(|d| d.valor).collect();
            for (d, parte) in divisoes.iter().zip(ratear(valor, &pesos)) {
                por_pessoa.somar(nome_pessoa(d.pessoa_id), parte);
            }
        }

        for m in mens.iter().filter(|m| conta_id.is_none_or(|c| c == m.conta_id) && m.cai_em(competencia)) {
            total += m.valor;
            por_categoria.somar(m.categoria.clone(), m.valor);
            por_pessoa.somar(nome_pessoa(m.pessoa_id), m.valor);
            por_origem.somar("Mensalidades", m.valor);
        }

        Ok(DetalheMes {
            competencia,
            total,
            por_categoria: por_categoria.ordenado(),
            por_pessoa: por_pessoa.ordenado(),
            por_origem: por_origem.ordenado(),
        })
    }

    /// Quanto foi gasto e recebido numa competência. Gastos seguem o mês da fatura;
    /// entradas seguem a data em que o dinheiro entrou.
    pub fn resumo_mes(&self, competencia: Competencia) -> Resultado<ResumoMes> {
        let pessoas = self.nomes_pessoas()?;
        let mens = self.mensalidades_internas()?;
        let d = self.detalhar(None, competencia, &mens, &pessoas)?;
        let entradas = self.entradas_do_mes(competencia)?;
        let total_entradas: i64 = entradas.iter().map(|e| e.valor).sum();

        // Acerto acumulado: tudo o que cada pessoa (fora "Eu") gastou até esta fatura
        // menos tudo o que ela pagou até o fim deste mês.
        let gastos = self.gasto_acumulado_por_pessoa(competencia, &mens)?;
        let pagos = self.pago_acumulado_por_pessoa(competencia)?;
        let mut acertos: Vec<Acerto> = pessoas
            .iter()
            .filter(|(id, _)| **id != PESSOA_EU)
            .map(|(id, nome)| Acerto {
                pessoa_id: *id,
                nome: nome.clone(),
                gasto: gastos.get(id).copied().unwrap_or(0),
                pago: pagos.get(id).copied().unwrap_or(0),
            })
            .filter(|a| a.gasto != 0 || a.pago != 0)
            .collect();
        acertos.sort_by_key(|a| std::cmp::Reverse(a.gasto - a.pago));

        Ok(ResumoMes {
            competencia,
            total: d.total,
            por_categoria: d.por_categoria,
            por_pessoa: d.por_pessoa,
            por_origem: d.por_origem,
            total_entradas,
            saldo: total_entradas - d.total,
            entradas,
            acertos,
        })
    }

    /// Gasto de cada pessoa em todas as faturas até `ate` (inclusive).
    fn gasto_acumulado_por_pessoa(
        &self,
        ate: Competencia,
        mens: &[MensalidadeInterna],
    ) -> Resultado<BTreeMap<i64, i64>> {
        let mut total: BTreeMap<i64, i64> = BTreeMap::new();
        let mut st = self.conn.prepare(
            "SELECT p.transacao_id, p.valor FROM parcela p WHERE p.competencia <= ?1",
        )?;
        let parcelas: Vec<(i64, i64)> = st
            .query_map([ate.to_string()], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        for (transacao_id, valor) in parcelas {
            let divisoes = self.divisoes_de(transacao_id)?;
            let pesos: Vec<i64> = divisoes.iter().map(|d| d.valor).collect();
            for (d, parte) in divisoes.iter().zip(ratear(valor, &pesos)) {
                *total.entry(d.pessoa_id).or_default() += parte;
            }
        }
        for m in mens {
            // Percorre do início da mensalidade até `ate` contando as faturas em que ela caiu.
            let mut c = m.inicio;
            while c <= ate {
                if m.cai_em(c) {
                    *total.entry(m.pessoa_id).or_default() += m.valor;
                }
                c = c.somar_meses(1);
            }
        }
        Ok(total)
    }

    /// Quanto cada pessoa pagou (entradas com "quem pagou") até o fim do mês `ate`.
    fn pago_acumulado_por_pessoa(&self, ate: Competencia) -> Resultado<BTreeMap<i64, i64>> {
        let mut st = self.conn.prepare(
            "SELECT pessoa_id, SUM(valor) FROM entrada
             WHERE pessoa_id IS NOT NULL AND substr(data, 1, 7) <= ?1
             GROUP BY pessoa_id",
        )?;
        let linhas = st.query_map([ate.to_string()], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(linhas.collect::<Result<_, _>>()?)
    }

    // ---------- Configurações e senha ----------

    fn ler_config(&self, chave: &str) -> Resultado<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT valor FROM config WHERE chave = ?1", [chave], |r| r.get(0))
            .optional()?)
    }

    fn gravar_config(&self, chave: &str, valor: Option<&str>) -> Resultado<()> {
        match valor {
            Some(v) => self.conn.execute(
                "INSERT INTO config (chave, valor) VALUES (?1, ?2)
                 ON CONFLICT (chave) DO UPDATE SET valor = excluded.valor",
                params![chave, v],
            )?,
            None => self.conn.execute("DELETE FROM config WHERE chave = ?1", [chave])?,
        };
        Ok(())
    }

    pub fn tem_senha(&self) -> Resultado<bool> {
        Ok(self.ler_config("senha_hash")?.is_some())
    }

    pub fn conferir_senha(&self, senha: &str) -> Resultado<bool> {
        Ok(self.ler_config("senha_hash")?.is_some_and(|h| seguranca::conferir(senha, &h)))
    }

    /// Define, troca ou remove (`nova = None`) a senha. Se já houver senha, exige a atual.
    pub fn definir_senha(&self, atual: Option<&str>, nova: Option<&str>) -> Resultado<()> {
        if self.tem_senha()? && !self.conferir_senha(atual.unwrap_or(""))? {
            return invalido("senha atual incorreta");
        }
        match nova {
            Some(n) => self.gravar_config("senha_hash", Some(&seguranca::gerar_hash(n)?)),
            None => self.gravar_config("senha_hash", None),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn c(s: &str) -> Competencia {
        s.parse().unwrap()
    }

    fn repo_com_cartao() -> (Repositorio, Conta) {
        let repo = Repositorio::em_memoria().unwrap();
        let cartao = repo
            .criar_conta(NovaConta {
                nome: "Nubank".into(),
                tipo: TipoConta::Cartao,
                dia_fechamento: Some(10),
                dia_vencimento: Some(17),
            })
            .unwrap();
        (repo, cartao)
    }

    fn compra(conta_id: i64, data: &str, valor: i64, parcelas: u32) -> NovaTransacao {
        NovaTransacao {
            conta_id,
            descricao: "Compra".into(),
            data_compra: data.into(),
            valor_total: valor,
            num_parcelas: parcelas,
            categoria_id: None,
            divisoes: vec![],
        }
    }

    #[test]
    fn banco_novo_tem_eu_e_categorias_padrao() {
        let repo = Repositorio::em_memoria().unwrap();
        assert_eq!(repo.listar_pessoas().unwrap()[0].nome, "Eu");
        assert_eq!(repo.listar_categorias().unwrap().len(), 4);
    }

    #[test]
    fn cartao_sem_fechamento_e_recusado() {
        let repo = Repositorio::em_memoria().unwrap();
        let r = repo.criar_conta(NovaConta {
            nome: "X".into(),
            tipo: TipoConta::Cartao,
            dia_fechamento: None,
            dia_vencimento: Some(5),
        });
        assert!(matches!(r, Err(Erro::Validacao(_))));
    }

    #[test]
    fn compra_parcelada_aparece_na_projecao() {
        let (mut repo, cartao) = repo_com_cartao();
        repo.criar_transacao(compra(cartao.id, "2026-11-15", 30000, 3)).unwrap();
        let f = repo.projetar_faturas(cartao.id, c("2026-11"), 5).unwrap();
        let v: Vec<i64> = f.iter().map(|x| x.valor).collect();
        assert_eq!(v, vec![0, 10000, 10000, 10000, 0]);
    }

    #[test]
    fn simulacao_nao_grava_nada() {
        let (mut repo, cartao) = repo_com_cartao();
        repo.criar_transacao(compra(cartao.id, "2026-11-01", 2056, 1)).unwrap();
        let sim = SimulacaoCompra {
            conta_id: cartao.id,
            valor_total: 60000,
            num_parcelas: 2,
            data_compra: "2026-11-01".into(),
        };
        let r = repo.simular_compra(&sim, c("2026-11"), 3).unwrap();
        assert_eq!(r[0], ComparacaoMes { competencia: c("2026-11"), atual: 2056, simulado: 32056 });
        assert_eq!(r[1].simulado, 30000);
        assert_eq!(r[2].simulado, 0);
        assert_eq!(repo.listar_transacoes().unwrap().len(), 1);
    }

    #[test]
    fn divisao_com_irma_entra_no_resumo() {
        let (mut repo, cartao) = repo_com_cartao();
        let irma = repo.criar_pessoa("Irmã").unwrap();
        let mut t = compra(cartao.id, "2026-11-01", 10000, 1);
        t.divisoes = vec![
            Divisao { pessoa_id: PESSOA_EU, valor: 5000 },
            Divisao { pessoa_id: irma.id, valor: 5000 },
        ];
        repo.criar_transacao(t).unwrap();
        let r = repo.resumo_mes(c("2026-11")).unwrap();
        assert_eq!(r.total, 10000);
        assert!(r.por_pessoa.contains(&TotalNomeado { nome: "Irmã".into(), valor: 5000 }));
        assert!(r.por_pessoa.contains(&TotalNomeado { nome: "Eu".into(), valor: 5000 }));
    }

    #[test]
    fn divisao_errada_nao_grava() {
        let (mut repo, cartao) = repo_com_cartao();
        let mut t = compra(cartao.id, "2026-11-01", 10000, 1);
        t.divisoes = vec![Divisao { pessoa_id: PESSOA_EU, valor: 4000 }];
        assert!(repo.criar_transacao(t).is_err());
        assert!(repo.listar_transacoes().unwrap().is_empty());
    }

    #[test]
    fn nao_exclui_conta_com_compras_nem_pessoa_eu() {
        let (mut repo, cartao) = repo_com_cartao();
        repo.criar_transacao(compra(cartao.id, "2026-11-01", 100, 1)).unwrap();
        assert!(matches!(repo.excluir_conta(cartao.id), Err(Erro::Validacao(_))));
        assert!(repo.excluir_pessoa(PESSOA_EU).is_err());
    }

    #[test]
    fn excluir_compra_remove_parcelas() {
        let (mut repo, cartao) = repo_com_cartao();
        let t = repo.criar_transacao(compra(cartao.id, "2026-11-01", 900, 3)).unwrap();
        repo.excluir_transacao(t.id).unwrap();
        let f = repo.projetar_faturas(cartao.id, c("2026-11"), 3).unwrap();
        assert!(f.iter().all(|x| x.valor == 0));
    }

    #[test]
    fn nome_duplicado_vira_mensagem_amigavel() {
        let repo = Repositorio::em_memoria().unwrap();
        let e = repo.criar_categoria("Lazer", "").unwrap_err();
        assert_eq!(e.to_string(), "já existe uma categoria com esse nome");
    }

    #[test]
    fn reabrir_arquivo_nao_repete_migracao() {
        let dir = std::env::temp_dir().join(format!("financas-teste-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let arq = dir.join("t.db");
        let _ = std::fs::remove_file(&arq);
        Repositorio::abrir(&arq).unwrap().criar_pessoa("Mãe").unwrap();
        let repo = Repositorio::abrir(&arq).unwrap();
        assert_eq!(repo.listar_pessoas().unwrap().len(), 2);
        drop(repo);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn mensalidade(conta_id: i64, valor: i64, data: &str) -> NovaMensalidade {
        NovaMensalidade {
            conta_id,
            descricao: "Claude Pro".into(),
            valor,
            primeira_cobranca: data.into(),
            categoria_id: None,
            pessoa_id: None,
        }
    }

    #[test]
    fn mensalidade_entra_nas_faturas_ate_ser_cancelada() {
        let (repo, cartao) = repo_com_cartao();
        // Cobrada dia 5 (antes do fechamento dia 10): cai na fatura do próprio mês.
        let m = repo.criar_mensalidade(mensalidade(cartao.id, 11000, "2026-11-05")).unwrap();
        let v: Vec<i64> = repo
            .projetar_faturas(cartao.id, c("2026-10"), 4)
            .unwrap()
            .iter()
            .map(|f| f.valor)
            .collect();
        assert_eq!(v, vec![0, 11000, 11000, 11000]);

        repo.cancelar_mensalidade(m.id, c("2026-12")).unwrap();
        let v: Vec<i64> = repo
            .projetar_faturas(cartao.id, c("2026-11"), 3)
            .unwrap()
            .iter()
            .map(|f| f.valor)
            .collect();
        assert_eq!(v, vec![11000, 11000, 0]);
        assert!(repo.cancelar_mensalidade(m.id, c("2026-01")).is_err());

        let r = repo.reativar_mensalidade(m.id).unwrap();
        assert!(r.fim.is_none());
    }

    #[test]
    fn mensalidade_cobrada_depois_do_fechamento_vai_para_o_mes_seguinte() {
        let (repo, cartao) = repo_com_cartao();
        repo.criar_mensalidade(mensalidade(cartao.id, 5000, "2026-11-20")).unwrap();
        let f = repo.projetar_faturas(cartao.id, c("2026-11"), 2).unwrap();
        assert_eq!(f[0].valor, 0);
        assert_eq!(f[1].valor, 5000);
    }

    #[test]
    fn compra_so_da_mae_e_entrada_dela_fecham_o_acerto() {
        let (mut repo, cartao) = repo_com_cartao();
        let mae = repo.criar_pessoa("Mãe").unwrap();
        let mut t = compra(cartao.id, "2026-11-01", 8000, 1);
        t.divisoes = vec![Divisao { pessoa_id: mae.id, valor: 8000 }];
        repo.criar_transacao(t).unwrap();
        repo.criar_entrada(NovaEntrada {
            conta_id: None,
            descricao: "Mãe devolveu".into(),
            data: "2026-11-20".into(),
            valor: 5000,
            tipo: TipoEntrada::Reembolso,
            pessoa_id: Some(mae.id),
        })
        .unwrap();
        repo.criar_entrada(NovaEntrada {
            conta_id: None,
            descricao: "Pix recebido".into(),
            data: "2026-11-02".into(),
            valor: 20000,
            tipo: TipoEntrada::Pix,
            pessoa_id: None,
        })
        .unwrap();

        let r = repo.resumo_mes(c("2026-11")).unwrap();
        assert_eq!(r.total, 8000);
        assert_eq!(r.total_entradas, 25000);
        assert_eq!(r.saldo, 17000);
        assert_eq!(r.entradas.len(), 2);
        assert_eq!(
            r.acertos,
            vec![Acerto { pessoa_id: mae.id, nome: "Mãe".into(), gasto: 8000, pago: 5000 }]
        );
        // Entrada de outro mês não entra.
        assert_eq!(repo.resumo_mes(c("2026-12")).unwrap().total_entradas, 0);
    }

    #[test]
    fn detalhe_da_fatura_separa_origem_e_pessoa() {
        let (mut repo, cartao) = repo_com_cartao();
        let mae = repo.criar_pessoa("Mãe").unwrap();
        repo.criar_transacao(compra(cartao.id, "2026-11-01", 1000, 1)).unwrap();
        repo.criar_transacao(compra(cartao.id, "2026-11-01", 3000, 3)).unwrap();
        let mut m = mensalidade(cartao.id, 500, "2026-11-01");
        m.pessoa_id = Some(mae.id);
        repo.criar_mensalidade(m).unwrap();

        let d = repo.detalhar_faturas(cartao.id, c("2026-11"), 2).unwrap();
        assert_eq!(d[0].total, 2500);
        let origem = |n: &str| d[0].por_origem.iter().find(|t| t.nome == n).map(|t| t.valor);
        assert_eq!(origem("À vista"), Some(1000));
        assert_eq!(origem("Parcelado"), Some(1000));
        assert_eq!(origem("Mensalidades"), Some(500));
        assert!(d[0].por_pessoa.contains(&TotalNomeado { nome: "Mãe".into(), valor: 500 }));
        assert_eq!(d[1].total, 1500);
    }

    #[test]
    fn senha_define_troca_e_remove() {
        let repo = Repositorio::em_memoria().unwrap();
        assert!(!repo.tem_senha().unwrap());
        repo.definir_senha(None, Some("1234")).unwrap();
        assert!(repo.conferir_senha("1234").unwrap());
        assert!(!repo.conferir_senha("0000").unwrap());
        assert!(repo.definir_senha(Some("errada"), Some("5678")).is_err());
        repo.definir_senha(Some("1234"), Some("5678")).unwrap();
        assert!(repo.conferir_senha("5678").unwrap());
        repo.definir_senha(Some("5678"), None).unwrap();
        assert!(!repo.tem_senha().unwrap());
    }

    #[test]
    fn acerto_acumula_entre_meses() {
        let (mut repo, cartao) = repo_com_cartao();
        let irma = repo.criar_pessoa("Irmã").unwrap();
        // Compra dia 12 (depois do fechamento): cai na fatura de dezembro.
        let mut t = compra(cartao.id, "2026-11-12", 10000, 1);
        t.divisoes = vec![Divisao { pessoa_id: irma.id, valor: 10000 }];
        repo.criar_transacao(t).unwrap();
        // Ela devolve uma parte ainda em novembro.
        repo.criar_entrada(NovaEntrada {
            conta_id: None,
            descricao: "Irmã devolveu".into(),
            data: "2026-11-20".into(),
            valor: 4000,
            tipo: TipoEntrada::Reembolso,
            pessoa_id: Some(irma.id),
        })
        .unwrap();
        let nov = repo.resumo_mes(c("2026-11")).unwrap().acertos;
        assert_eq!((nov[0].gasto, nov[0].pago), (0, 4000));
        let dez = repo.resumo_mes(c("2026-12")).unwrap().acertos;
        assert_eq!((dez[0].gasto, dez[0].pago), (10000, 4000));
        let jan = repo.resumo_mes(c("2027-01")).unwrap().acertos;
        assert_eq!((jan[0].gasto, jan[0].pago), (10000, 4000));
    }
}
