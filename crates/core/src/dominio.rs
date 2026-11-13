//! Regras de negócio puras (sem banco). Valores sempre em centavos.

use std::collections::{BTreeMap, HashSet};

use chrono::{Datelike, NaiveDate};

use crate::competencia::Competencia;
use crate::erro::{invalido, Resultado};
use crate::modelos::FaturaMes;

/// Regra 1: compra feita antes do dia de fechamento entra na fatura do mês;
/// no dia ou depois, na do mês seguinte. Conta sem fechamento (corrente)
/// usa o próprio mês da compra.
pub fn competencia_da_compra(data: NaiveDate, dia_fechamento: Option<u32>) -> Competencia {
    let base = Competencia::de_data(data);
    match dia_fechamento {
        Some(fechamento) if data.day() >= fechamento => base.somar_meses(1),
        _ => base,
    }
}

/// Regra 2: divide o total em `n` parcelas; o centavo que sobra vai para a primeira.
pub fn dividir_em_parcelas(total: i64, n: u32) -> Resultado<Vec<i64>> {
    if total <= 0 {
        return invalido("o valor precisa ser maior que zero");
    }
    if n == 0 || n > 120 {
        return invalido("o número de parcelas precisa estar entre 1 e 120");
    }
    let n_i = n as i64;
    let base = total / n_i;
    let resto = total % n_i;
    if base == 0 {
        return invalido("valor pequeno demais para esse número de parcelas");
    }
    let mut parcelas = vec![base; n as usize];
    parcelas[0] += resto;
    Ok(parcelas)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParcelaGerada {
    pub numero: u32,
    pub valor: i64,
    pub competencia: Competencia,
}

/// Gera as parcelas de uma compra, cada uma já com o mês da fatura.
pub fn gerar_parcelas(
    data_compra: NaiveDate,
    total: i64,
    n: u32,
    dia_fechamento: Option<u32>,
) -> Resultado<Vec<ParcelaGerada>> {
    let primeira = competencia_da_compra(data_compra, dia_fechamento);
    Ok(dividir_em_parcelas(total, n)?
        .into_iter()
        .enumerate()
        .map(|(i, valor)| ParcelaGerada {
            numero: i as u32 + 1,
            valor,
            competencia: primeira.somar_meses(i as i32),
        })
        .collect())
}

/// Regra 3: as divisões precisam somar o total, sem pessoa repetida.
pub fn validar_divisoes(total: i64, divisoes: &[(i64, i64)]) -> Resultado<()> {
    let mut vistos = HashSet::new();
    for &(pessoa_id, valor) in divisoes {
        if valor <= 0 {
            return invalido("cada parte da divisão precisa ser maior que zero");
        }
        if !vistos.insert(pessoa_id) {
            return invalido("a mesma pessoa aparece duas vezes na divisão");
        }
    }
    let soma: i64 = divisoes.iter().map(|&(_, v)| v).sum();
    if soma != total {
        return invalido(format!(
            "a divisão soma {} mas a compra é de {}",
            formatar_reais(soma),
            formatar_reais(total)
        ));
    }
    Ok(())
}

/// Reparte `valor` proporcionalmente aos `pesos`, sem perder centavos:
/// a sobra do arredondamento vai para o primeiro.
pub fn ratear(valor: i64, pesos: &[i64]) -> Vec<i64> {
    let soma: i128 = pesos.iter().map(|&p| p as i128).sum();
    if pesos.is_empty() || soma <= 0 {
        return vec![0; pesos.len()];
    }
    let mut partes: Vec<i64> = pesos
        .iter()
        .map(|&p| ((valor as i128 * p as i128) / soma) as i64)
        .collect();
    let sobra = valor - partes.iter().sum::<i64>();
    partes[0] += sobra;
    partes
}

/// Regra 4: fatura de cada mês = parcelas daquele mês + mensalidades que caem nele.
/// `mensalidades(c)` devolve o total de mensalidades da competência `c`.
pub fn projetar(
    lancamentos: &[(Competencia, i64)],
    mensalidades: impl Fn(Competencia) -> i64,
    inicio: Competencia,
    meses: u32,
) -> Vec<FaturaMes> {
    let mut por_mes: BTreeMap<Competencia, i64> = BTreeMap::new();
    for &(c, v) in lancamentos {
        *por_mes.entry(c).or_default() += v;
    }
    (0..meses as i32)
        .map(|i| {
            let competencia = inicio.somar_meses(i);
            FaturaMes {
                competencia,
                valor: por_mes.get(&competencia).copied().unwrap_or(0) + mensalidades(competencia),
            }
        })
        .collect()
}

/// Quantos meses a cobrança de uma mensalidade "anda" até cair na fatura:
/// no cartão, cobrança no dia do fechamento ou depois vai para a fatura seguinte.
pub fn deslocamento_cobranca(dia_cobranca: u32, dia_fechamento: Option<u32>) -> i32 {
    match dia_fechamento {
        Some(f) if dia_cobranca >= f => 1,
        _ => 0,
    }
}

/// A mensalidade aparece na `competencia` se o mês da cobrança correspondente
/// está entre `inicio` e `fim` (fim vazio = ainda ativa).
pub fn mensalidade_cai_em(
    inicio: Competencia,
    fim: Option<Competencia>,
    deslocamento: i32,
    competencia: Competencia,
) -> bool {
    let mes_cobranca = competencia.somar_meses(-deslocamento);
    mes_cobranca >= inicio && fim.is_none_or(|f| mes_cobranca <= f)
}

/// Converte texto `AAAA-MM-DD` em data.
pub fn ler_data(texto: &str) -> Resultado<NaiveDate> {
    NaiveDate::parse_from_str(texto, "%Y-%m-%d")
        .or_else(|_| invalido(format!("data inválida (use AAAA-MM-DD): {texto}")))
}

pub fn formatar_reais(centavos: i64) -> String {
    let sinal = if centavos < 0 { "-" } else { "" };
    let c = centavos.abs();
    format!("{sinal}R$ {},{:02}", c / 100, c % 100)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        ler_data(s).unwrap()
    }
    fn c(s: &str) -> Competencia {
        s.parse().unwrap()
    }

    #[test]
    fn compra_antes_do_fechamento_cai_no_mes() {
        assert_eq!(competencia_da_compra(d("2026-11-09"), Some(10)), c("2026-11"));
    }

    #[test]
    fn compra_no_dia_do_fechamento_cai_no_mes_seguinte() {
        assert_eq!(competencia_da_compra(d("2026-11-10"), Some(10)), c("2026-12"));
        assert_eq!(competencia_da_compra(d("2026-12-20"), Some(10)), c("2027-01"));
    }

    #[test]
    fn conta_corrente_usa_o_mes_da_compra() {
        assert_eq!(competencia_da_compra(d("2026-11-25"), None), c("2026-11"));
    }

    #[test]
    fn sobra_de_centavo_vai_na_primeira_parcela() {
        assert_eq!(dividir_em_parcelas(10000, 3).unwrap(), vec![3334, 3333, 3333]);
        assert_eq!(dividir_em_parcelas(30000, 3).unwrap(), vec![10000; 3]);
    }

    #[test]
    fn parcelas_invalidas_sao_recusadas() {
        assert!(dividir_em_parcelas(0, 1).is_err());
        assert!(dividir_em_parcelas(100, 0).is_err());
        assert!(dividir_em_parcelas(2, 3).is_err());
    }

    #[test]
    fn gera_parcelas_em_meses_seguidos() {
        let p = gerar_parcelas(d("2026-11-15"), 30000, 3, Some(10)).unwrap();
        let meses: Vec<String> = p.iter().map(|x| x.competencia.to_string()).collect();
        assert_eq!(meses, vec!["2026-12", "2027-01", "2027-02"]);
        assert_eq!(p.iter().map(|x| x.valor).sum::<i64>(), 30000);
        assert_eq!(p[2].numero, 3);
    }

    #[test]
    fn divisao_precisa_bater_com_o_total() {
        assert!(validar_divisoes(10000, &[(1, 5000), (2, 5000)]).is_ok());
        assert!(validar_divisoes(10000, &[(1, 5000), (2, 4000)]).is_err());
        assert!(validar_divisoes(10000, &[(1, 5000), (1, 5000)]).is_err());
        assert!(validar_divisoes(10000, &[(1, 10000), (2, 0)]).is_err());
    }

    #[test]
    fn rateio_nao_perde_centavos() {
        assert_eq!(ratear(3334, &[5000, 5000]), vec![1667, 1667]);
        assert_eq!(ratear(1001, &[1, 1, 1]), vec![335, 333, 333]);
        assert_eq!(ratear(100, &[]), Vec::<i64>::new());
    }

    #[test]
    fn projecao_soma_parcelas_e_recorrentes() {
        let lanc = vec![(c("2026-12"), 1000), (c("2026-12"), 500), (c("2027-02"), 200)];
        let f = projetar(&lanc, |_| 100, c("2026-12"), 3);
        let valores: Vec<i64> = f.iter().map(|x| x.valor).collect();
        assert_eq!(valores, vec![1600, 100, 300]);
    }

    #[test]
    fn mensalidade_respeita_inicio_fim_e_fechamento() {
        // Claude cobrado dia 15, cartão fecha dia 10: cobrança de nov cai na fatura de dez.
        let desloc = deslocamento_cobranca(15, Some(10));
        assert_eq!(desloc, 1);
        let inicio = c("2026-11");
        assert!(!mensalidade_cai_em(inicio, None, desloc, c("2026-11")));
        assert!(mensalidade_cai_em(inicio, None, desloc, c("2026-12")));
        assert!(mensalidade_cai_em(inicio, None, desloc, c("2027-06")));
        // Cancelada depois da cobrança de jan: última fatura é a de fev.
        let fim = Some(c("2027-01"));
        assert!(mensalidade_cai_em(inicio, fim, desloc, c("2027-02")));
        assert!(!mensalidade_cai_em(inicio, fim, desloc, c("2027-03")));
        // Conta corrente: sem deslocamento.
        assert_eq!(deslocamento_cobranca(15, None), 0);
    }

    #[test]
    fn formata_em_reais() {
        assert_eq!(formatar_reais(2056), "R$ 20,56");
        assert_eq!(formatar_reais(-5), "-R$ 0,05");
    }
}
