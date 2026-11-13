//! Situação das faturas (vencimento, paga ou em aberto) e simulação de compra
//! olhando todos os cartões juntos.

use chrono::{Local, NaiveDate};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;

use super::Repositorio;
use crate::competencia::Competencia;
use crate::dominio;
use crate::erro::{invalido, Erro, Resultado};
use crate::modelos::{Conta, SimulacaoCompra, TipoConta, TotalNomeado};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PagamentoFatura {
    /// AAAA-MM-DD
    pub data: String,
    pub valor: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StatusFatura {
    pub conta_id: i64,
    pub conta_nome: String,
    pub competencia: Competencia,
    /// Valor projetado da fatura (parcelas + mensalidades).
    pub valor: i64,
    /// Data de vencimento (AAAA-MM-DD).
    pub vencimento: Option<String>,
    pub paga: Option<PagamentoFatura>,
    /// Venceu, tem valor e não foi marcada como paga.
    pub atrasada: bool,
}

/// Um mês da simulação com todos os cartões.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComparacaoGeral {
    pub competencia: Competencia,
    /// Fatura de hoje de cada cartão (e da conta da compra, se não for cartão).
    pub por_conta: Vec<TotalNomeado>,
    /// Soma de hoje de todas as contas acima.
    pub atual: i64,
    /// Soma com a compra simulada.
    pub simulado: i64,
    /// Contas cuja fatura desse mês já foi paga.
    pub pagas: Vec<String>,
}

/// Dia de vencimento da fatura da competência. Se o vencimento é depois do fechamento,
/// vence no mesmo mês; senão (ex.: fecha 28, vence 5), no mês seguinte.
pub fn vencimento_da_fatura(conta: &Conta, competencia: Competencia) -> Option<NaiveDate> {
    let venc = conta.dia_vencimento?;
    let fech = conta.dia_fechamento.unwrap_or(0);
    let c = if venc > fech { competencia } else { competencia.somar_meses(1) };
    (0..4).find_map(|recuo| NaiveDate::from_ymd_opt(c.ano, c.mes, venc.saturating_sub(recuo)))
}

impl Repositorio {
    fn pagamento_fatura(&self, conta_id: i64, c: Competencia) -> Resultado<Option<PagamentoFatura>> {
        Ok(self
            .conn
            .query_row(
                "SELECT data_pagamento, valor FROM fatura_paga WHERE conta_id = ?1 AND competencia = ?2",
                params![conta_id, c.to_string()],
                |r| Ok(PagamentoFatura { data: r.get(0)?, valor: r.get(1)? }),
            )
            .optional()?)
    }

    /// Situação das faturas de um cartão, mês a mês.
    pub fn status_faturas(&self, conta_id: i64, inicio: Competencia, meses: u32) -> Resultado<Vec<StatusFatura>> {
        self.status_faturas_em(conta_id, inicio, meses, Local::now().date_naive())
    }

    fn status_faturas_em(
        &self,
        conta_id: i64,
        inicio: Competencia,
        meses: u32,
        hoje: NaiveDate,
    ) -> Resultado<Vec<StatusFatura>> {
        let conta = self.buscar_conta(conta_id)?;
        let projecao = self.projetar_faturas(conta_id, inicio, meses)?;
        projecao
            .into_iter()
            .map(|f| {
                let venc = vencimento_da_fatura(&conta, f.competencia);
                let paga = self.pagamento_fatura(conta_id, f.competencia)?;
                Ok(StatusFatura {
                    conta_id,
                    conta_nome: conta.nome.clone(),
                    competencia: f.competencia,
                    valor: f.valor,
                    vencimento: venc.map(|d| d.format("%Y-%m-%d").to_string()),
                    atrasada: paga.is_none() && f.valor > 0 && venc.is_some_and(|v| v < hoje),
                    paga,
                })
            })
            .collect()
    }

    /// Faturas de todos os cartões numa competência (para o Resumo).
    pub fn faturas_do_mes(&self, competencia: Competencia) -> Resultado<Vec<StatusFatura>> {
        let mut saida = Vec::new();
        for conta in self.listar_contas()?.into_iter().filter(|c| c.tipo == TipoConta::Cartao) {
            saida.extend(self.status_faturas(conta.id, competencia, 1)?);
        }
        Ok(saida)
    }

    /// Marca a fatura como paga. Sem `valor`, usa o valor projetado da fatura.
    pub fn marcar_fatura_paga(
        &self,
        conta_id: i64,
        competencia: Competencia,
        data: &str,
        valor: Option<i64>,
    ) -> Resultado<StatusFatura> {
        let conta = self.buscar_conta(conta_id)?;
        if conta.tipo != TipoConta::Cartao {
            return invalido("só cartões têm fatura para pagar");
        }
        dominio::ler_data(data)?;
        let valor = match valor {
            Some(v) if v < 0 => return invalido("valor pago inválido"),
            Some(v) => v,
            None => self.projetar_faturas(conta_id, competencia, 1)?[0].valor,
        };
        self.conn.execute(
            "INSERT INTO fatura_paga (conta_id, competencia, data_pagamento, valor) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (conta_id, competencia) DO UPDATE
             SET data_pagamento = excluded.data_pagamento, valor = excluded.valor",
            params![conta_id, competencia.to_string(), data, valor],
        )?;
        self.status_faturas(conta_id, competencia, 1)?
            .pop()
            .ok_or_else(|| Erro::NaoEncontrado("fatura".into()))
    }

    pub fn desmarcar_fatura_paga(&self, conta_id: i64, competencia: Competencia) -> Resultado<()> {
        self.conn.execute(
            "DELETE FROM fatura_paga WHERE conta_id = ?1 AND competencia = ?2",
            params![conta_id, competencia.to_string()],
        )?;
        Ok(())
    }

    /// Simula a compra e mostra o total de TODOS os cartões antes e depois.
    /// Se a compra for numa conta corrente, ela entra na soma também.
    pub fn simular_compra_todas(
        &self,
        sim: &SimulacaoCompra,
        inicio: Competencia,
        meses: u32,
    ) -> Resultado<Vec<ComparacaoGeral>> {
        let conta_compra = self.buscar_conta(sim.conta_id)?;
        let data = dominio::ler_data(&sim.data_compra)?;
        let novas =
            dominio::gerar_parcelas(data, sim.valor_total, sim.num_parcelas, conta_compra.dia_fechamento)?;

        let mut contas: Vec<Conta> =
            self.listar_contas()?.into_iter().filter(|c| c.tipo == TipoConta::Cartao).collect();
        if !contas.iter().any(|c| c.id == conta_compra.id) {
            contas.push(conta_compra);
        }
        let projecoes: Vec<(Conta, Vec<crate::modelos::FaturaMes>)> = contas
            .into_iter()
            .map(|c| {
                let p = self.projetar_faturas(c.id, inicio, meses)?;
                Ok((c, p))
            })
            .collect::<Resultado<_>>()?;

        (0..meses as usize)
            .map(|i| {
                let competencia = inicio.somar_meses(i as i32);
                let por_conta: Vec<TotalNomeado> = projecoes
                    .iter()
                    .map(|(c, p)| TotalNomeado { nome: c.nome.clone(), valor: p[i].valor })
                    .collect();
                let atual: i64 = por_conta.iter().map(|t| t.valor).sum();
                let extra: i64 = novas.iter().filter(|p| p.competencia == competencia).map(|p| p.valor).sum();
                let mut pagas = Vec::new();
                for (c, _) in &projecoes {
                    if c.tipo == TipoConta::Cartao && self.pagamento_fatura(c.id, competencia)?.is_some() {
                        pagas.push(c.nome.clone());
                    }
                }
                Ok(ComparacaoGeral { competencia, por_conta, atual, simulado: atual + extra, pagas })
            })
            .collect()
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::modelos::*;

    fn c(s: &str) -> Competencia {
        s.parse().unwrap()
    }

    fn cartao(repo: &Repositorio, nome: &str, fech: u32, venc: u32) -> Conta {
        repo.criar_conta(NovaConta {
            nome: nome.into(),
            tipo: TipoConta::Cartao,
            dia_fechamento: Some(fech),
            dia_vencimento: Some(venc),
        })
        .unwrap()
    }

    fn compra(repo: &mut Repositorio, conta: i64, data: &str, valor: i64, n: u32) {
        repo.criar_transacao(NovaTransacao {
            conta_id: conta,
            descricao: "x".into(),
            data_compra: data.into(),
            valor_total: valor,
            num_parcelas: n,
            categoria_id: None,
            divisoes: vec![],
            competencia_fatura: None,
        })
        .unwrap();
    }

    #[test]
    fn vencimento_no_mesmo_mes_ou_no_seguinte() {
        let repo = Repositorio::em_memoria().unwrap();
        let a = cartao(&repo, "A", 10, 17);
        let b = cartao(&repo, "B", 28, 5);
        assert_eq!(vencimento_da_fatura(&a, c("2026-11")), NaiveDate::from_ymd_opt(2026, 11, 17));
        assert_eq!(vencimento_da_fatura(&b, c("2026-11")), NaiveDate::from_ymd_opt(2026, 12, 5));
        let c31 = cartao(&repo, "C", 20, 31);
        assert_eq!(vencimento_da_fatura(&c31, c("2027-02")), NaiveDate::from_ymd_opt(2027, 2, 28));
    }

    #[test]
    fn marcar_paga_e_atraso() {
        let mut repo = Repositorio::em_memoria().unwrap();
        let a = cartao(&repo, "A", 10, 17);
        compra(&mut repo, a.id, "2026-11-01", 50000, 1);
        let hoje = NaiveDate::from_ymd_opt(2026, 11, 20).unwrap();
        let s = repo.status_faturas_em(a.id, c("2026-11"), 2, hoje).unwrap();
        assert!(s[0].atrasada, "venceu dia 17 e não foi paga");
        assert!(!s[1].atrasada);

        let paga = repo.marcar_fatura_paga(a.id, c("2026-11"), "2026-11-12", None).unwrap();
        assert_eq!(paga.paga, Some(PagamentoFatura { data: "2026-11-12".into(), valor: 50000 }));
        assert!(!paga.atrasada);
        // Marcar de novo atualiza.
        let p2 = repo.marcar_fatura_paga(a.id, c("2026-11"), "2026-11-13", Some(40000)).unwrap();
        assert_eq!(p2.paga.unwrap().valor, 40000);
        assert_eq!(repo.faturas_do_mes(c("2026-11")).unwrap().len(), 1);

        repo.desmarcar_fatura_paga(a.id, c("2026-11")).unwrap();
        assert!(repo.status_faturas_em(a.id, c("2026-11"), 1, hoje).unwrap()[0].paga.is_none());

        let corrente = repo
            .criar_conta(NovaConta { nome: "BB".into(), tipo: TipoConta::Corrente, dia_fechamento: None, dia_vencimento: None })
            .unwrap();
        assert!(repo.marcar_fatura_paga(corrente.id, c("2026-11"), "2026-11-12", None).is_err());
    }

    #[test]
    fn simulacao_soma_todos_os_cartoes() {
        let mut repo = Repositorio::em_memoria().unwrap();
        let a = cartao(&repo, "A", 10, 17);
        let b = cartao(&repo, "B", 10, 17);
        compra(&mut repo, a.id, "2026-11-01", 10000, 1);
        compra(&mut repo, b.id, "2026-11-01", 30000, 3);
        repo.marcar_fatura_paga(a.id, c("2026-11"), "2026-11-12", None).unwrap();

        let sim = SimulacaoCompra { conta_id: b.id, valor_total: 20000, num_parcelas: 2, data_compra: "2026-11-01".into() };
        let r = repo.simular_compra_todas(&sim, c("2026-11"), 3).unwrap();
        assert_eq!(r[0].atual, 20000);
        assert_eq!(r[0].simulado, 30000);
        assert_eq!(r[0].por_conta, vec![
            TotalNomeado { nome: "A".into(), valor: 10000 },
            TotalNomeado { nome: "B".into(), valor: 10000 },
        ]);
        assert_eq!(r[0].pagas, vec!["A".to_string()]);
        assert_eq!((r[1].atual, r[1].simulado), (10000, 20000));
        assert_eq!((r[2].atual, r[2].simulado), (10000, 10000));

        // Compra numa conta corrente entra na soma junto com os cartões.
        let pix = repo
            .criar_conta(NovaConta { nome: "PicPay".into(), tipo: TipoConta::Corrente, dia_fechamento: None, dia_vencimento: None })
            .unwrap();
        let sim2 = SimulacaoCompra { conta_id: pix.id, valor_total: 5000, num_parcelas: 1, data_compra: "2026-11-05".into() };
        let r2 = repo.simular_compra_todas(&sim2, c("2026-11"), 1).unwrap();
        assert_eq!(r2[0].por_conta.len(), 3);
        assert_eq!(r2[0].simulado - r2[0].atual, 5000);
    }
}
