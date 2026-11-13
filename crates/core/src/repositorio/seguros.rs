//! Veículos e seguros (auto, saúde, vida, residencial). O pagamento do seguro
//! é uma mensalidade comum, então entra nas faturas e no resumo como as outras.

use chrono::{Local, NaiveDate};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::{texto_obrigatorio, traduzir, Repositorio, PESSOA_EU};
use crate::dominio;
use crate::erro::{invalido, Erro, Resultado};
use crate::modelos::NovaMensalidade;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NovoVeiculo {
    pub nome: String,
    pub placa: Option<String>,
    pub ano: Option<i32>,
    pub pessoa_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Veiculo {
    pub id: i64,
    pub nome: String,
    pub placa: Option<String>,
    pub ano: Option<i32>,
    pub pessoa_id: i64,
    pub pessoa_nome: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TipoSeguro {
    Auto,
    Saude,
    Vida,
    Residencial,
    Outro,
}

impl TipoSeguro {
    fn texto(self) -> &'static str {
        match self {
            TipoSeguro::Auto => "auto",
            TipoSeguro::Saude => "saude",
            TipoSeguro::Vida => "vida",
            TipoSeguro::Residencial => "residencial",
            TipoSeguro::Outro => "outro",
        }
    }
    fn de_texto(s: &str) -> Self {
        match s {
            "auto" => TipoSeguro::Auto,
            "saude" => TipoSeguro::Saude,
            "vida" => TipoSeguro::Vida,
            "residencial" => TipoSeguro::Residencial,
            _ => TipoSeguro::Outro,
        }
    }
}

/// Como o seguro é pago: vira uma mensalidade.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PagamentoSeguro {
    pub conta_id: i64,
    pub valor: i64,
    /// AAAA-MM-DD
    pub primeira_cobranca: String,
    pub categoria_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NovoSeguro {
    pub tipo: TipoSeguro,
    pub descricao: String,
    pub seguradora: Option<String>,
    pub pessoa_id: Option<i64>,
    pub veiculo_id: Option<i64>,
    /// Fim da vigência da apólice (AAAA-MM-DD), para avisar a renovação.
    pub vigencia_fim: Option<String>,
    /// Só usado ao criar (ou se o seguro ainda não tiver pagamento ligado).
    #[serde(default)]
    pub pagamento: Option<PagamentoSeguro>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Seguro {
    pub id: i64,
    pub tipo: TipoSeguro,
    pub descricao: String,
    pub seguradora: Option<String>,
    pub pessoa_id: i64,
    pub pessoa_nome: String,
    pub veiculo_id: Option<i64>,
    pub veiculo_nome: Option<String>,
    pub vigencia_fim: Option<String>,
    /// Dias até o fim da vigência (negativo = já venceu).
    pub dias_para_vencer: Option<i64>,
    pub mensalidade_id: Option<i64>,
    /// Valor mensal da mensalidade ligada, se ativa.
    pub valor_mensal: Option<i64>,
}

fn opcional(texto: &Option<String>) -> Option<String> {
    texto.as_ref().map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
}

impl Repositorio {
    // ---------- Veículos ----------

    pub fn listar_veiculos(&self) -> Resultado<Vec<Veiculo>> {
        let mut st = self.conn.prepare(
            "SELECT v.id, v.nome, v.placa, v.ano, v.pessoa_id, p.nome
             FROM veiculo v JOIN pessoa p ON p.id = v.pessoa_id ORDER BY v.nome",
        )?;
        let linhas = st.query_map([], |r| {
            Ok(Veiculo {
                id: r.get(0)?,
                nome: r.get(1)?,
                placa: r.get(2)?,
                ano: r.get(3)?,
                pessoa_id: r.get(4)?,
                pessoa_nome: r.get(5)?,
            })
        })?;
        Ok(linhas.collect::<Result<_, _>>()?)
    }

    fn validar_veiculo(v: &NovoVeiculo) -> Resultado<String> {
        if let Some(ano) = v.ano {
            if !(1900..=2100).contains(&ano) {
                return invalido("ano do veículo inválido");
            }
        }
        texto_obrigatorio(&v.nome, "o nome do veículo")
    }

    pub fn criar_veiculo(&self, v: NovoVeiculo) -> Resultado<Veiculo> {
        let nome = Self::validar_veiculo(&v)?;
        self.conn
            .execute(
                "INSERT INTO veiculo (nome, placa, ano, pessoa_id) VALUES (?1, ?2, ?3, ?4)",
                params![nome, opcional(&v.placa).map(|p| p.to_uppercase()), v.ano, v.pessoa_id.unwrap_or(PESSOA_EU)],
            )
            .map_err(|e| traduzir(e, "pessoa inexistente", ""))?;
        let id = self.conn.last_insert_rowid();
        self.buscar_veiculo(id)
    }

    pub fn atualizar_veiculo(&self, id: i64, v: NovoVeiculo) -> Resultado<Veiculo> {
        self.buscar_veiculo(id)?;
        let nome = Self::validar_veiculo(&v)?;
        self.conn
            .execute(
                "UPDATE veiculo SET nome = ?1, placa = ?2, ano = ?3, pessoa_id = ?4 WHERE id = ?5",
                params![
                    nome,
                    opcional(&v.placa).map(|p| p.to_uppercase()),
                    v.ano,
                    v.pessoa_id.unwrap_or(PESSOA_EU),
                    id
                ],
            )
            .map_err(|e| traduzir(e, "pessoa inexistente", ""))?;
        self.buscar_veiculo(id)
    }

    fn buscar_veiculo(&self, id: i64) -> Resultado<Veiculo> {
        self.listar_veiculos()?
            .into_iter()
            .find(|v| v.id == id)
            .ok_or_else(|| Erro::NaoEncontrado(format!("veículo {id}")))
    }

    /// Seguros ligados ao veículo ficam sem veículo, mas não são apagados.
    pub fn excluir_veiculo(&self, id: i64) -> Resultado<()> {
        let n = self.conn.execute("DELETE FROM veiculo WHERE id = ?1", [id])?;
        if n == 0 {
            return Err(Erro::NaoEncontrado(format!("veículo {id}")));
        }
        Ok(())
    }

    // ---------- Seguros ----------

    pub fn listar_seguros(&self) -> Resultado<Vec<Seguro>> {
        self.listar_seguros_em(Local::now().date_naive())
    }

    fn listar_seguros_em(&self, hoje: NaiveDate) -> Resultado<Vec<Seguro>> {
        let mut st = self.conn.prepare(
            "SELECT s.id, s.tipo, s.descricao, s.seguradora, s.pessoa_id, p.nome, s.veiculo_id, v.nome,
                    s.vigencia_fim, s.mensalidade_id, CASE WHEN a.fim IS NULL THEN a.valor END
             FROM seguro s
             JOIN pessoa p ON p.id = s.pessoa_id
             LEFT JOIN veiculo v ON v.id = s.veiculo_id
             LEFT JOIN assinatura a ON a.id = s.mensalidade_id
             ORDER BY s.vigencia_fim IS NULL, s.vigencia_fim, s.descricao",
        )?;
        let linhas = st.query_map([], |r| {
            let tipo: String = r.get(1)?;
            let vigencia: Option<String> = r.get(8)?;
            let dias = vigencia
                .as_deref()
                .and_then(|v| NaiveDate::parse_from_str(v, "%Y-%m-%d").ok())
                .map(|v| (v - hoje).num_days());
            Ok(Seguro {
                id: r.get(0)?,
                tipo: TipoSeguro::de_texto(&tipo),
                descricao: r.get(2)?,
                seguradora: r.get(3)?,
                pessoa_id: r.get(4)?,
                pessoa_nome: r.get(5)?,
                veiculo_id: r.get(6)?,
                veiculo_nome: r.get(7)?,
                vigencia_fim: vigencia,
                dias_para_vencer: dias,
                mensalidade_id: r.get(9)?,
                valor_mensal: r.get(10)?,
            })
        })?;
        Ok(linhas.collect::<Result<_, _>>()?)
    }

    fn buscar_seguro(&self, id: i64) -> Resultado<Seguro> {
        self.listar_seguros()?
            .into_iter()
            .find(|s| s.id == id)
            .ok_or_else(|| Erro::NaoEncontrado(format!("seguro {id}")))
    }

    /// Seguros com vigência terminando em até `dias` dias (ou já vencidos há até 30 dias).
    pub fn seguros_vencendo(&self, dias: i64) -> Resultado<Vec<Seguro>> {
        Ok(self
            .listar_seguros()?
            .into_iter()
            .filter(|s| s.dias_para_vencer.is_some_and(|d| (-30..=dias).contains(&d)))
            .collect())
    }

    fn validar_seguro(&self, s: &NovoSeguro) -> Resultado<String> {
        if let Some(v) = opcional(&s.vigencia_fim) {
            dominio::ler_data(&v)?;
        }
        if let Some(v) = s.veiculo_id {
            self.buscar_veiculo(v)?;
        }
        texto_obrigatorio(&s.descricao, "a descrição do seguro")
    }

    fn criar_pagamento(&self, descricao: &str, s: &NovoSeguro, p: &PagamentoSeguro) -> Resultado<i64> {
        let m = self.criar_mensalidade(NovaMensalidade {
            conta_id: p.conta_id,
            descricao: format!("Seguro: {descricao}"),
            valor: p.valor,
            primeira_cobranca: p.primeira_cobranca.clone(),
            categoria_id: p.categoria_id,
            pessoa_id: s.pessoa_id,
        })?;
        Ok(m.id)
    }

    pub fn criar_seguro(&self, s: NovoSeguro) -> Resultado<Seguro> {
        let descricao = self.validar_seguro(&s)?;
        let mensalidade = match &s.pagamento {
            Some(p) => Some(self.criar_pagamento(&descricao, &s, p)?),
            None => None,
        };
        self.conn
            .execute(
                "INSERT INTO seguro (tipo, descricao, seguradora, pessoa_id, veiculo_id, vigencia_fim, mensalidade_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    s.tipo.texto(),
                    descricao,
                    opcional(&s.seguradora),
                    s.pessoa_id.unwrap_or(PESSOA_EU),
                    s.veiculo_id,
                    opcional(&s.vigencia_fim),
                    mensalidade
                ],
            )
            .map_err(|e| traduzir(e, "pessoa ou veículo inexistente", ""))?;
        self.buscar_seguro(self.conn.last_insert_rowid())
    }

    pub fn atualizar_seguro(&self, id: i64, s: NovoSeguro) -> Resultado<Seguro> {
        let atual = self.buscar_seguro(id)?;
        let descricao = self.validar_seguro(&s)?;
        let mensalidade = match (atual.mensalidade_id, &s.pagamento) {
            (None, Some(p)) => Some(self.criar_pagamento(&descricao, &s, p)?),
            (m, _) => m,
        };
        self.conn
            .execute(
                "UPDATE seguro SET tipo = ?1, descricao = ?2, seguradora = ?3, pessoa_id = ?4, veiculo_id = ?5,
                                   vigencia_fim = ?6, mensalidade_id = ?7
                 WHERE id = ?8",
                params![
                    s.tipo.texto(),
                    descricao,
                    opcional(&s.seguradora),
                    s.pessoa_id.unwrap_or(PESSOA_EU),
                    s.veiculo_id,
                    opcional(&s.vigencia_fim),
                    mensalidade,
                    id
                ],
            )
            .map_err(|e| traduzir(e, "pessoa ou veículo inexistente", ""))?;
        self.buscar_seguro(id)
    }

    /// Exclui o seguro. Com `excluir_pagamento`, apaga também a mensalidade ligada.
    pub fn excluir_seguro(&self, id: i64, excluir_pagamento: bool) -> Resultado<()> {
        let mensalidade: Option<i64> = self
            .conn
            .query_row("SELECT mensalidade_id FROM seguro WHERE id = ?1", [id], |r| r.get(0))
            .optional()?
            .ok_or_else(|| Erro::NaoEncontrado(format!("seguro {id}")))?;
        self.conn.execute("DELETE FROM seguro WHERE id = ?1", [id])?;
        if let (true, Some(m)) = (excluir_pagamento, mensalidade) {
            self.excluir_mensalidade(m)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::modelos::*;

    #[test]
    fn seguro_do_carro_com_pagamento_mensal_e_aviso_de_vencimento() {
        let repo = Repositorio::em_memoria().unwrap();
        let cartao = repo
            .criar_conta(NovaConta {
                nome: "Cartão".into(),
                tipo: TipoConta::Cartao,
                dia_fechamento: Some(10),
                dia_vencimento: Some(17),
            })
            .unwrap();
        let carro = repo
            .criar_veiculo(NovoVeiculo { nome: "Onix".into(), placa: Some("abc1d23".into()), ano: Some(2020), pessoa_id: None })
            .unwrap();
        assert_eq!(carro.placa.as_deref(), Some("ABC1D23"));

        let s = repo
            .criar_seguro(NovoSeguro {
                tipo: TipoSeguro::Auto,
                descricao: "Seguro do Onix".into(),
                seguradora: Some("Porto".into()),
                pessoa_id: None,
                veiculo_id: Some(carro.id),
                vigencia_fim: Some("2026-12-01".into()),
                pagamento: Some(PagamentoSeguro {
                    conta_id: cartao.id,
                    valor: 25000,
                    primeira_cobranca: "2026-11-05".into(),
                    categoria_id: None,
                }),
            })
            .unwrap();
        assert_eq!(s.valor_mensal, Some(25000));
        assert_eq!(s.veiculo_nome.as_deref(), Some("Onix"));
        let f = repo.projetar_faturas(cartao.id, "2026-11".parse().unwrap(), 1).unwrap();
        assert_eq!(f[0].valor, 25000, "pagamento do seguro entra na fatura");

        let hoje = NaiveDate::from_ymd_opt(2026, 11, 13).unwrap();
        let lista = repo.listar_seguros_em(hoje).unwrap();
        assert_eq!(lista[0].dias_para_vencer, Some(18));

        // Excluir o veículo mantém o seguro, sem veículo.
        repo.excluir_veiculo(carro.id).unwrap();
        assert_eq!(repo.listar_seguros().unwrap()[0].veiculo_id, None);

        repo.excluir_seguro(s.id, true).unwrap();
        assert!(repo.listar_seguros().unwrap().is_empty());
        assert!(repo.listar_mensalidades().unwrap().is_empty());
    }

    #[test]
    fn seguro_sem_pagamento_e_validacoes() {
        let repo = Repositorio::em_memoria().unwrap();
        let base = NovoSeguro {
            tipo: TipoSeguro::Saude,
            descricao: "Plano de saúde da mãe".into(),
            seguradora: None,
            pessoa_id: None,
            veiculo_id: None,
            vigencia_fim: None,
            pagamento: None,
        };
        let s = repo.criar_seguro(base.clone()).unwrap();
        assert_eq!((s.mensalidade_id, s.dias_para_vencer), (None, None));
        let mut ruim = base.clone();
        ruim.vigencia_fim = Some("31/12/2026".into());
        assert!(repo.criar_seguro(ruim).is_err());
        let mut sem_nome = base;
        sem_nome.descricao = " ".into();
        assert!(repo.atualizar_seguro(s.id, sem_nome).is_err());
        assert!(repo.criar_veiculo(NovoVeiculo { nome: "X".into(), placa: None, ano: Some(1800), pessoa_id: None }).is_err());
    }
}
