//! Importação de extrato/fatura: prévia (nada é gravado) e confirmação.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::Repositorio;
use crate::competencia::Competencia;
use crate::dominio;
use crate::erro::{Erro, Resultado};
use crate::extrato::{self, Formato, MapaColunas};
use crate::modelos::{NovaEntrada, NovaTransacao, TipoConta, TipoEntrada};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TipoLinha {
    Gasto,
    Entrada,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinhaPrevia {
    pub data: String,
    pub descricao: String,
    /// Sempre positivo; o sentido está em `tipo`.
    pub valor: i64,
    pub tipo: TipoLinha,
    pub hash: String,
    /// Já foi importada antes: não entra de novo.
    pub duplicada: bool,
    /// Se preenchido, vem desmarcada na tela (ex.: linha de saldo).
    pub motivo_ignorar: Option<String>,
    /// Categoria usada da última vez numa compra com descrição parecida.
    pub categoria_id: Option<i64>,
    /// "02/10" quando a fatura indica parcela.
    pub parcela: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviaImportacao {
    pub formato: Formato,
    pub cabecalho: Vec<String>,
    pub mapa: Option<MapaColunas>,
    pub conta_cartao: bool,
    /// Valores positivos tratados como gasto (CSV de fatura de cartão costuma vir assim).
    pub sinal_invertido: bool,
    /// Para cartão: fatura sugerida para todos os gastos do arquivo.
    pub competencia_sugerida: Option<Competencia>,
    pub linhas: Vec<LinhaPrevia>,
    pub ilegiveis: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemImportacao {
    pub data: String,
    pub descricao: String,
    pub valor: i64,
    pub tipo: TipoLinha,
    pub hash: String,
    pub categoria_id: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResultadoImportacao {
    pub compras: usize,
    pub entradas: usize,
    pub duplicadas: usize,
}

fn tipo_entrada(descricao: &str) -> TipoEntrada {
    let d = extrato::normalizar(descricao);
    if d.contains("pix") {
        TipoEntrada::Pix
    } else if d.contains("salario") || d.contains("bolsa") || d.contains("proventos") || d.contains("folha") {
        TipoEntrada::Salario
    } else if d.contains("estorno") || d.contains("reembolso") || d.contains("devolucao") {
        TipoEntrada::Reembolso
    } else if d.contains("ted") || d.contains("doc") || d.contains("transf") {
        TipoEntrada::Pagamento
    } else {
        TipoEntrada::Outro
    }
}

impl Repositorio {
    fn hashes_importados(&self) -> Resultado<HashSet<String>> {
        let mut st = self.conn.prepare(
            "SELECT hash_importacao FROM transacao WHERE hash_importacao IS NOT NULL
             UNION SELECT hash_importacao FROM entrada WHERE hash_importacao IS NOT NULL",
        )?;
        let h = st.query_map([], |r| r.get::<_, String>(0))?.collect::<Result<_, _>>()?;
        Ok(h)
    }

    /// Descrição sem números → última categoria usada.
    fn categorias_por_descricao(&self) -> Resultado<HashMap<String, i64>> {
        let mut st = self.conn.prepare(
            "SELECT descricao, categoria_id FROM transacao WHERE categoria_id IS NOT NULL ORDER BY id",
        )?;
        let linhas: Vec<(String, i64)> =
            st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
        Ok(linhas.into_iter().map(|(d, c)| (extrato::chave_descricao(&d), c)).collect())
    }

    /// Lê o arquivo e mostra o que seria importado, sem gravar nada.
    /// `inverter_sinal`: None = decide sozinho (CSV de cartão com maioria positiva).
    pub fn previa_importacao(
        &self,
        bytes: &[u8],
        conta_id: i64,
        mapa: Option<MapaColunas>,
        inverter_sinal: Option<bool>,
    ) -> Resultado<PreviaImportacao> {
        let conta = self.buscar_conta(conta_id)?;
        let eh_cartao = conta.tipo == TipoConta::Cartao;
        let mut lido = extrato::ler(bytes, mapa)?;

        let positivos = lido.linhas.iter().filter(|l| l.valor > 0).count();
        let invertido = inverter_sinal
            .unwrap_or(eh_cartao && lido.formato == Formato::Csv && positivos * 2 > lido.linhas.len());
        if invertido {
            for l in &mut lido.linhas {
                l.valor = -l.valor;
            }
        }

        let ja_importados = self.hashes_importados()?;
        let categorias = self.categorias_por_descricao()?;
        let mut ocorrencias: HashMap<(String, i64, String), usize> = HashMap::new();
        let mut linhas = Vec::with_capacity(lido.linhas.len());
        for l in &lido.linhas {
            let chave = (l.data.to_string(), l.valor, extrato::normalizar(&l.descricao));
            let n = ocorrencias.entry(chave).or_default();
            let hash = extrato::hash_linha(conta_id, l, *n);
            *n += 1;
            linhas.push(LinhaPrevia {
                data: l.data.format("%Y-%m-%d").to_string(),
                descricao: l.descricao.clone(),
                valor: l.valor.abs(),
                tipo: if l.valor < 0 { TipoLinha::Gasto } else { TipoLinha::Entrada },
                duplicada: ja_importados.contains(&hash),
                hash,
                motivo_ignorar: extrato::motivo_ignorar(&l.descricao, l.valor, eh_cartao).map(str::to_string),
                categoria_id: categorias.get(&extrato::chave_descricao(&l.descricao)).copied(),
                parcela: if eh_cartao { extrato::parcela_na_descricao(&l.descricao) } else { None },
            });
        }

        linhas.sort_by(|a, b| a.data.cmp(&b.data));

        // Fatura do arquivo: a do lançamento mais recente, pela regra do fechamento.
        let competencia_sugerida = if eh_cartao {
            lido.linhas
                .iter()
                .map(|l| l.data)
                .max()
                .map(|d| dominio::competencia_da_compra(d, conta.dia_fechamento))
        } else {
            None
        };

        Ok(PreviaImportacao {
            formato: lido.formato,
            cabecalho: lido.cabecalho,
            mapa: lido.mapa,
            conta_cartao: eh_cartao,
            sinal_invertido: invertido,
            competencia_sugerida,
            linhas,
            ilegiveis: lido.ignoradas,
        })
    }

    /// Grava os itens escolhidos na prévia. Gastos viram compras à vista e entradas viram
    /// entradas. Em cartão, `competencia_fatura` coloca todos os gastos na mesma fatura.
    pub fn confirmar_importacao(
        &mut self,
        conta_id: i64,
        competencia_fatura: Option<Competencia>,
        itens: Vec<ItemImportacao>,
    ) -> Resultado<ResultadoImportacao> {
        self.buscar_conta(conta_id)?;
        let mut r = ResultadoImportacao::default();
        let ja = self.hashes_importados()?;
        for item in itens {
            if ja.contains(&item.hash) {
                r.duplicadas += 1;
                continue;
            }
            let gravado = match item.tipo {
                TipoLinha::Gasto => self
                    .gravar_transacao(
                        None,
                        &NovaTransacao {
                            conta_id,
                            descricao: item.descricao.clone(),
                            data_compra: item.data.clone(),
                            valor_total: item.valor,
                            num_parcelas: 1,
                            categoria_id: item.categoria_id,
                            divisoes: vec![],
                            competencia_fatura,
                        },
                        Some(&item.hash),
                    )
                    .map(|_| r.compras += 1),
                TipoLinha::Entrada => self
                    .gravar_entrada(
                        None,
                        &NovaEntrada {
                            conta_id: Some(conta_id),
                            descricao: item.descricao.clone(),
                            data: item.data.clone(),
                            valor: item.valor,
                            tipo: tipo_entrada(&item.descricao),
                            pessoa_id: None,
                        },
                        Some(&item.hash),
                    )
                    .map(|_| r.entradas += 1),
            };
            match gravado {
                Ok(()) => {}
                Err(Erro::Validacao(m)) if m.contains("já foi importada") => r.duplicadas += 1,
                Err(e) => return Err(e),
            }
        }
        Ok(r)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::modelos::NovaConta;

    const OFX_CONTA: &str = "OFXHEADER:100\n<OFX><BANKTRANLIST>\
<STMTTRN><TRNTYPE>DEBIT<DTPOSTED>20261103<TRNAMT>-45.90<FITID>1<MEMO>SUPERMERCADO</STMTTRN>\
<STMTTRN><TRNTYPE>CREDIT<DTPOSTED>20261105<TRNAMT>1500.00<FITID>2<MEMO>Pix recebido</STMTTRN>\
<STMTTRN><TRNTYPE>DEBIT<DTPOSTED>20261110<TRNAMT>-800.00<FITID>3<MEMO>Pagto cartao credito</STMTTRN>\
</BANKTRANLIST></OFX>";

    fn itens(p: &PreviaImportacao) -> Vec<ItemImportacao> {
        p.linhas
            .iter()
            .filter(|l| l.motivo_ignorar.is_none() && !l.duplicada)
            .map(|l| ItemImportacao {
                data: l.data.clone(),
                descricao: l.descricao.clone(),
                valor: l.valor,
                tipo: l.tipo,
                hash: l.hash.clone(),
                categoria_id: l.categoria_id,
            })
            .collect()
    }

    #[test]
    fn importa_extrato_sem_duplicar() {
        let mut repo = Repositorio::em_memoria().unwrap();
        let conta = repo
            .criar_conta(NovaConta {
                nome: "BB".into(),
                tipo: TipoConta::Corrente,
                dia_fechamento: None,
                dia_vencimento: None,
            })
            .unwrap();
        let p = repo.previa_importacao(OFX_CONTA.as_bytes(), conta.id, None, None).unwrap();
        assert_eq!(p.linhas.len(), 3);
        assert!(p.linhas[2].motivo_ignorar.is_some(), "pagamento de fatura vem desmarcado");
        assert_eq!(p.linhas[1].tipo, TipoLinha::Entrada);

        let r = repo.confirmar_importacao(conta.id, None, itens(&p)).unwrap();
        assert_eq!(r, ResultadoImportacao { compras: 1, entradas: 1, duplicadas: 0 });
        let e = repo.listar_entradas().unwrap();
        assert_eq!(e[0].tipo, TipoEntrada::Pix);
        assert!(repo.listar_transacoes().unwrap()[0].importada);

        // Mesmo arquivo de novo: tudo aparece como duplicado.
        let p2 = repo.previa_importacao(OFX_CONTA.as_bytes(), conta.id, None, None).unwrap();
        assert!(p2.linhas[0].duplicada && p2.linhas[1].duplicada);
        let todos: Vec<ItemImportacao> = itens(&p);
        let r2 = repo.confirmar_importacao(conta.id, None, todos).unwrap();
        assert_eq!(r2.duplicadas, 2);
        assert_eq!(repo.listar_transacoes().unwrap().len(), 1);
    }

    #[test]
    fn fatura_de_cartao_em_csv_vai_toda_para_a_fatura_escolhida() {
        let mut repo = Repositorio::em_memoria().unwrap();
        let cartao = repo
            .criar_conta(NovaConta {
                nome: "Ourocard".into(),
                tipo: TipoConta::Cartao,
                dia_fechamento: Some(10),
                dia_vencimento: Some(17),
            })
            .unwrap();
        let comida = repo.listar_categorias().unwrap().into_iter().find(|c| c.nome == "Comida").unwrap();
        // Compra anterior com categoria: a importação sugere a mesma.
        repo.criar_transacao(NovaTransacao {
            conta_id: cartao.id,
            descricao: "IFOOD 1234".into(),
            data_compra: "2026-09-01".into(),
            valor_total: 100,
            num_parcelas: 1,
            categoria_id: Some(comida.id),
            divisoes: vec![],
            competencia_fatura: None,
        })
        .unwrap();

        let csv = "Data,Descrição,Valor\n15/10/2026,IFOOD 9876,35.50\n20/10/2026,LOJA PARC 02/10,100.00\n05/11/2026,PAGAMENTO RECEBIDO,-800.00\n";
        let p = repo.previa_importacao(csv.as_bytes(), cartao.id, None, None).unwrap();
        assert!(p.sinal_invertido, "fatura em CSV vem com gastos positivos");
        assert_eq!(p.linhas[0].tipo, TipoLinha::Gasto);
        assert_eq!(p.linhas[0].categoria_id, Some(comida.id));
        assert_eq!(p.linhas[1].parcela.as_deref(), Some("02/10"));
        assert!(p.linhas[2].motivo_ignorar.is_some());
        assert_eq!(p.competencia_sugerida, Some("2026-11".parse().unwrap()));

        let fatura: Competencia = "2026-11".parse().unwrap();
        repo.confirmar_importacao(cartao.id, Some(fatura), itens(&p)).unwrap();
        let f = repo.projetar_faturas(cartao.id, "2026-10".parse().unwrap(), 2).unwrap();
        assert_eq!(f[0].valor, 0);
        assert_eq!(f[1].valor, 13550);
    }
}
