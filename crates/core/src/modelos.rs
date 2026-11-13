//! Estruturas trocadas entre o núcleo e a interface. Valores em centavos.
//! Os campos viram camelCase no JSON para casar com o TypeScript.

use serde::{Deserialize, Serialize};

use crate::competencia::Competencia;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Pessoa {
    pub id: i64,
    pub nome: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TipoConta {
    Corrente,
    Cartao,
}

impl TipoConta {
    pub fn como_texto(self) -> &'static str {
        match self {
            TipoConta::Corrente => "corrente",
            TipoConta::Cartao => "cartao",
        }
    }

    pub fn de_texto(s: &str) -> Self {
        if s == "cartao" { TipoConta::Cartao } else { TipoConta::Corrente }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Conta {
    pub id: i64,
    pub nome: String,
    pub tipo: TipoConta,
    pub dia_fechamento: Option<u32>,
    pub dia_vencimento: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NovaConta {
    pub nome: String,
    pub tipo: TipoConta,
    pub dia_fechamento: Option<u32>,
    pub dia_vencimento: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Categoria {
    pub id: i64,
    pub nome: String,
    pub cor: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Divisao {
    pub pessoa_id: i64,
    pub valor: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NovaTransacao {
    pub conta_id: i64,
    pub descricao: String,
    /// AAAA-MM-DD
    pub data_compra: String,
    pub valor_total: i64,
    pub num_parcelas: u32,
    pub categoria_id: Option<i64>,
    /// Vazio = tudo da pessoa "Eu".
    #[serde(default)]
    pub divisoes: Vec<Divisao>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transacao {
    pub id: i64,
    pub conta_id: i64,
    pub conta_nome: String,
    pub descricao: String,
    pub data_compra: String,
    pub valor_total: i64,
    pub num_parcelas: u32,
    pub categoria_id: Option<i64>,
    pub categoria_nome: Option<String>,
    pub divisoes: Vec<Divisao>,
}

/// Uma parcela que cai numa competência, com o contexto da compra.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LancamentoDoMes {
    pub transacao_id: i64,
    pub descricao: String,
    pub conta_nome: String,
    pub categoria_nome: Option<String>,
    pub numero: u32,
    pub num_parcelas: u32,
    pub valor: i64,
    pub data_compra: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FaturaMes {
    pub competencia: Competencia,
    pub valor: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComparacaoMes {
    pub competencia: Competencia,
    pub atual: i64,
    pub simulado: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulacaoCompra {
    pub conta_id: i64,
    pub valor_total: i64,
    pub num_parcelas: u32,
    /// AAAA-MM-DD
    pub data_compra: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TotalNomeado {
    pub nome: String,
    pub valor: i64,
}

// ---------- Mensalidades (gastos recorrentes, ex.: assinatura do Claude) ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NovaMensalidade {
    pub conta_id: i64,
    pub descricao: String,
    pub valor: i64,
    /// Data da primeira cobrança (AAAA-MM-DD): define o mês de início e o dia da cobrança.
    pub primeira_cobranca: String,
    pub categoria_id: Option<i64>,
    /// De quem é a mensalidade; vazio = "Eu".
    pub pessoa_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mensalidade {
    pub id: i64,
    pub conta_id: i64,
    pub conta_nome: String,
    pub descricao: String,
    pub valor: i64,
    pub dia_cobranca: u32,
    pub categoria_id: Option<i64>,
    pub categoria_nome: Option<String>,
    pub pessoa_id: i64,
    pub pessoa_nome: String,
    pub inicio: Competencia,
    /// Último mês cobrado; vazio = ativa.
    pub fim: Option<Competencia>,
}

// ---------- Entradas (saldo positivo: pix, salário, pagamentos recebidos) ----------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TipoEntrada {
    Pix,
    Salario,
    Pagamento,
    Reembolso,
    Outro,
}

impl TipoEntrada {
    pub fn como_texto(self) -> &'static str {
        match self {
            TipoEntrada::Pix => "pix",
            TipoEntrada::Salario => "salario",
            TipoEntrada::Pagamento => "pagamento",
            TipoEntrada::Reembolso => "reembolso",
            TipoEntrada::Outro => "outro",
        }
    }

    pub fn de_texto(s: &str) -> Self {
        match s {
            "pix" => TipoEntrada::Pix,
            "salario" => TipoEntrada::Salario,
            "pagamento" => TipoEntrada::Pagamento,
            "reembolso" => TipoEntrada::Reembolso,
            _ => TipoEntrada::Outro,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NovaEntrada {
    pub conta_id: Option<i64>,
    pub descricao: String,
    /// AAAA-MM-DD
    pub data: String,
    pub valor: i64,
    pub tipo: TipoEntrada,
    /// Quem pagou (ex.: a mãe devolvendo o que usou do cartão). Vazio = não é de ninguém da lista.
    pub pessoa_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entrada {
    pub id: i64,
    pub conta_id: Option<i64>,
    pub conta_nome: Option<String>,
    pub descricao: String,
    pub data: String,
    pub valor: i64,
    pub tipo: TipoEntrada,
    pub pessoa_id: Option<i64>,
    pub pessoa_nome: Option<String>,
}

// ---------- Detalhes e resumo ----------

/// Composição de uma fatura (ou de todos os gastos de um mês).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetalheMes {
    pub competencia: Competencia,
    pub total: i64,
    pub por_categoria: Vec<TotalNomeado>,
    pub por_pessoa: Vec<TotalNomeado>,
    /// "À vista", "Parcelado", "Mensalidades"
    pub por_origem: Vec<TotalNomeado>,
}

/// Saldo acumulado com uma pessoa: tudo o que ela gastou até esta fatura
/// e tudo o que ela pagou até o fim deste mês.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Acerto {
    pub pessoa_id: i64,
    pub nome: String,
    pub gasto: i64,
    pub pago: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumoMes {
    pub competencia: Competencia,
    /// Total de gastos (saídas) do mês.
    pub total: i64,
    pub por_categoria: Vec<TotalNomeado>,
    pub por_pessoa: Vec<TotalNomeado>,
    pub por_origem: Vec<TotalNomeado>,
    pub total_entradas: i64,
    /// Entradas − gastos.
    pub saldo: i64,
    pub entradas: Vec<Entrada>,
    /// Pessoas além de "Eu" com gasto ou pagamento acumulado até o mês.
    pub acertos: Vec<Acerto>,
}
