//! Sugestão de mensalidades: acha cobranças que se repetem todo mês nas compras
//! lançadas ou importadas (ex.: Netflix, Claude, academia) e ainda não cadastradas.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use chrono::{Datelike, Local, NaiveDate};
use serde::Serialize;

use super::Repositorio;
use crate::competencia::Competencia;
use crate::erro::Resultado;
use crate::extrato::chave_descricao;

/// Precisa aparecer em pelo menos 3 meses diferentes.
const MESES_MINIMOS: usize = 3;
/// Valores podem variar até 15% da mediana (reajustes, IOF).
const TOLERANCIA: f64 = 0.15;
/// A última cobrança precisa ter sido há no máximo 45 dias (ainda ativa).
const DIAS_MAXIMOS: i64 = 45;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SugestaoMensalidade {
    /// Identifica a sugestão (para ignorar).
    pub chave: String,
    pub descricao: String,
    pub conta_id: i64,
    pub conta_nome: String,
    /// Valor da cobrança mais recente.
    pub valor: i64,
    pub dia_cobranca: u32,
    pub meses: usize,
    pub ultima_data: String,
    /// Próxima cobrança esperada: vira a "primeira cobrança" se você cadastrar.
    pub proxima_cobranca: String,
    pub categoria_id: Option<i64>,
}

impl Repositorio {
    fn sugestoes_ignoradas(&self) -> Resultado<HashSet<String>> {
        Ok(self
            .ler_config("sugestoes_ignoradas")?
            .map(|s| s.split('\n').filter(|l| !l.is_empty()).map(str::to_string).collect())
            .unwrap_or_default())
    }

    pub fn ignorar_sugestao(&self, chave: &str) -> Resultado<()> {
        let mut todas = self.sugestoes_ignoradas()?;
        todas.insert(chave.to_string());
        let mut lista: Vec<String> = todas.into_iter().collect();
        lista.sort();
        self.gravar_config("sugestoes_ignoradas", Some(&lista.join("\n")))
    }

    pub fn sugerir_mensalidades(&self) -> Resultado<Vec<SugestaoMensalidade>> {
        self.sugerir_mensalidades_em(Local::now().date_naive())
    }

    pub(crate) fn sugerir_mensalidades_em(&self, hoje: NaiveDate) -> Resultado<Vec<SugestaoMensalidade>> {
        let desde = hoje - chrono::Duration::days(400);
        let mut st = self.conn.prepare(
            "SELECT t.conta_id, c.nome, t.descricao, t.data_compra, t.valor_total, t.categoria_id
             FROM transacao t JOIN conta c ON c.id = t.conta_id
             WHERE t.num_parcelas = 1 AND t.data_compra >= ?1
             ORDER BY t.data_compra",
        )?;
        type Linha = (i64, String, String, String, i64, Option<i64>);
        let linhas: Vec<Linha> = st
            .query_map([desde.format("%Y-%m-%d").to_string()], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
            })?
            .collect::<Result<_, _>>()?;

        // O que já está cadastrado como mensalidade (ativa) não é sugerido de novo.
        let mut st = self.conn.prepare("SELECT conta_id, descricao FROM assinatura WHERE fim IS NULL")?;
        let cadastradas: Vec<(i64, String)> = st
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
            .filter_map(|r| r.ok())
            .map(|(c, d)| (c, chave_descricao(&d)))
            .collect();
        let ignoradas = self.sugestoes_ignoradas()?;

        let mut grupos: BTreeMap<(i64, String), Vec<&Linha>> = BTreeMap::new();
        for l in &linhas {
            let chave = chave_descricao(&l.2);
            if chave.len() >= 3 {
                grupos.entry((l.0, chave)).or_default().push(l);
            }
        }

        let mut sugestoes = Vec::new();
        for ((conta_id, chave), itens) in grupos {
            let id_sugestao = format!("{conta_id}:{chave}");
            let ja_cadastrada = cadastradas
                .iter()
                .any(|(c, d)| *c == conta_id && !d.is_empty() && (d.contains(&chave) || chave.contains(d.as_str())));
            if ignoradas.contains(&id_sugestao) || ja_cadastrada {
                continue;
            }
            let mut valores: Vec<i64> = itens.iter().map(|l| l.4).collect();
            valores.sort_unstable();
            let mediana = valores[valores.len() / 2] as f64;
            let parecidos: Vec<&&Linha> = itens
                .iter()
                .filter(|l| ((l.4 as f64) - mediana).abs() <= mediana * TOLERANCIA)
                .collect();
            let meses: BTreeSet<Competencia> = parecidos
                .iter()
                .filter_map(|l| NaiveDate::parse_from_str(&l.3, "%Y-%m-%d").ok())
                .map(Competencia::de_data)
                .collect();
            if meses.len() < MESES_MINIMOS {
                continue;
            }
            let Some(ultima) = parecidos.last() else { continue };
            let Ok(data_ultima) = NaiveDate::parse_from_str(&ultima.3, "%Y-%m-%d") else { continue };
            if (hoje - data_ultima).num_days() > DIAS_MAXIMOS {
                continue;
            }
            // Mesmo dia no mês seguinte (ou o último dia, se o mês for mais curto).
            let c = Competencia::de_data(data_ultima).somar_meses(1);
            let proxima = (0..4)
                .find_map(|recuo| NaiveDate::from_ymd_opt(c.ano, c.mes, data_ultima.day().saturating_sub(recuo)))
                .unwrap_or(data_ultima);
            sugestoes.push(SugestaoMensalidade {
                chave: id_sugestao,
                descricao: ultima.2.clone(),
                conta_id,
                conta_nome: ultima.1.clone(),
                valor: ultima.4,
                dia_cobranca: data_ultima.day(),
                meses: meses.len(),
                ultima_data: ultima.3.clone(),
                proxima_cobranca: proxima.format("%Y-%m-%d").to_string(),
                categoria_id: ultima.5,
            });
        }
        sugestoes.sort_by_key(|s| std::cmp::Reverse(s.valor));
        Ok(sugestoes)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::modelos::*;

    fn compra(repo: &mut Repositorio, conta: i64, desc: &str, data: &str, valor: i64) {
        repo.criar_transacao(NovaTransacao {
            conta_id: conta,
            descricao: desc.into(),
            data_compra: data.into(),
            valor_total: valor,
            num_parcelas: 1,
            categoria_id: None,
            divisoes: vec![],
            competencia_fatura: None,
        })
        .unwrap();
    }

    #[test]
    fn sugere_cobranca_repetida_e_some_quando_ignorada() {
        let mut repo = Repositorio::em_memoria().unwrap();
        let c = repo
            .criar_conta(NovaConta {
                nome: "Cartão".into(),
                tipo: TipoConta::Cartao,
                dia_fechamento: Some(10),
                dia_vencimento: Some(17),
            })
            .unwrap()
            .id;
        for (d, v) in [("2026-08-12", 11000), ("2026-09-12", 11000), ("2026-10-12", 11500)] {
            compra(&mut repo, c, &format!("CLAUDE.AI SUBSCRIPTION {}", &d[5..7]), d, v);
        }
        // Repete, mas com valores muito diferentes: não é mensalidade.
        for (d, v) in [("2026-08-03", 2000), ("2026-09-03", 9000), ("2026-10-03", 500)] {
            compra(&mut repo, c, "MERCADO", d, v);
        }
        // Só dois meses: ainda não sugere.
        for d in ["2026-09-20", "2026-10-20"] {
            compra(&mut repo, c, "SPOTIFY", d, 2190);
        }

        let hoje = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
        let s = repo.sugerir_mensalidades_em(hoje).unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].valor, 11500);
        assert_eq!(s[0].meses, 3);
        assert_eq!(s[0].dia_cobranca, 12);
        assert_eq!(s[0].proxima_cobranca, "2026-11-12");

        // Muito tempo depois da última cobrança: não sugere (provavelmente cancelou).
        let depois = NaiveDate::from_ymd_opt(2027, 1, 15).unwrap();
        assert!(repo.sugerir_mensalidades_em(depois).unwrap().is_empty());

        repo.ignorar_sugestao(&s[0].chave).unwrap();
        assert!(repo.sugerir_mensalidades_em(hoje).unwrap().is_empty());
    }

    #[test]
    fn mensalidade_ja_cadastrada_nao_vira_sugestao() {
        let mut repo = Repositorio::em_memoria().unwrap();
        let c = repo
            .criar_conta(NovaConta {
                nome: "BB".into(),
                tipo: TipoConta::Corrente,
                dia_fechamento: None,
                dia_vencimento: None,
            })
            .unwrap()
            .id;
        for d in ["2026-08-05", "2026-09-05", "2026-10-05"] {
            compra(&mut repo, c, "NETFLIX.COM", d, 5590);
        }
        let hoje = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
        assert_eq!(repo.sugerir_mensalidades_em(hoje).unwrap().len(), 1);
        repo.criar_mensalidade(NovaMensalidade {
            conta_id: c,
            descricao: "Netflix.com".into(),
            valor: 5590,
            primeira_cobranca: "2026-11-05".into(),
            categoria_id: None,
            pessoa_id: None,
            divisoes: vec![],
        })
        .unwrap();
        assert!(repo.sugerir_mensalidades_em(hoje).unwrap().is_empty());
    }
}
