//! Leitura de extratos e faturas exportados pelos bancos (OFX e CSV).
//! Funções puras: recebem os bytes do arquivo e devolvem linhas padronizadas.
//!
//! Testado com os formatos do Banco do Brasil e do PicPay; o CSV é lido por
//! nome de coluna, então outros bancos com cabeçalho parecido também funcionam.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::erro::{invalido, Resultado};

/// Uma linha do extrato. `valor` em centavos: negativo = saída, positivo = entrada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinhaExtrato {
    pub data: NaiveDate,
    pub descricao: String,
    pub valor: i64,
    /// Identificador do banco (FITID do OFX), quando existe.
    pub id_externo: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Formato {
    Ofx,
    Csv,
}

/// Índices (a partir de 0) das colunas do CSV.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapaColunas {
    pub data: usize,
    pub descricao: usize,
    pub valor: usize,
    /// Coluna opcional que diz se é crédito ou débito (C/D, Entrada/Saída).
    pub tipo: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct ExtratoLido {
    pub formato: Formato,
    pub linhas: Vec<LinhaExtrato>,
    /// Cabeçalho do CSV (vazio no OFX), para a tela deixar trocar as colunas.
    pub cabecalho: Vec<String>,
    pub mapa: Option<MapaColunas>,
    /// Linhas que não deu para ler (sem data ou sem valor).
    pub ignoradas: usize,
}

/// Converte os bytes em texto: UTF-8 se for válido, senão Latin-1 (comum no BB).
pub fn decodificar(bytes: &[u8]) -> String {
    let sem_bom = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    match std::str::from_utf8(sem_bom) {
        Ok(s) => s.to_string(),
        Err(_) => sem_bom.iter().map(|&b| b as char).collect(),
    }
}

/// Minúsculas, sem acento, sem pontuação e sem espaços repetidos.
pub fn normalizar(texto: &str) -> String {
    let mut saida = String::with_capacity(texto.len());
    for c in texto.to_lowercase().chars() {
        let c = match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            c if c.is_alphanumeric() => c,
            _ => ' ',
        };
        saida.push(c);
    }
    saida.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Descrição sem números (datas, parcelas, códigos): agrupa cobranças do mesmo serviço.
pub fn chave_descricao(texto: &str) -> String {
    normalizar(texto)
        .split_whitespace()
        .filter(|p| !p.chars().any(|c| c.is_ascii_digit()))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Lê valores como "1.234,56", "-1234.56", "R$ 12,00", "(12,00)", "12,00 D".
pub fn ler_valor(texto: &str) -> Option<i64> {
    let t = texto.trim();
    if t.is_empty() {
        return None;
    }
    let limpo = t.replace("R$", "");
    let limpo = limpo.trim();
    let mut negativo = limpo.starts_with('-')
        || limpo.ends_with('-')
        || (limpo.starts_with('(') && limpo.ends_with(')'));
    let maiusculo = t.to_uppercase();
    if maiusculo.ends_with(" D") || maiusculo.ends_with("-D") {
        negativo = true;
    }
    let mut numeros: String = t.chars().filter(|c| c.is_ascii_digit() || *c == ',' || *c == '.').collect();
    if numeros.is_empty() {
        return None;
    }
    let ultima_virgula = numeros.rfind(',');
    let ultimo_ponto = numeros.rfind('.');
    numeros = match (ultima_virgula, ultimo_ponto) {
        // Os dois aparecem: o que vem por último é o decimal.
        (Some(v), Some(p)) if v > p => numeros.replace('.', "").replace(',', "."),
        (Some(_), Some(_)) => numeros.replace(',', ""),
        (Some(_), None) => numeros.replace(',', "."),
        (None, Some(p)) => {
            // Só ponto: "1.234" (milhar) vs "12.5" / "12.50" (decimal).
            let casas = numeros.len() - p - 1;
            if casas == 3 && numeros.matches('.').count() >= 1 && numeros.len() > 4 {
                numeros.replace('.', "")
            } else {
                numeros
            }
        }
        (None, None) => numeros,
    };
    let (inteiro, fracao) = numeros.split_once('.').unwrap_or((&numeros, ""));
    if inteiro.contains('.') || fracao.contains('.') {
        return None;
    }
    let inteiro: i64 = if inteiro.is_empty() { 0 } else { inteiro.parse().ok()? };
    let centavos: i64 = match fracao.len() {
        0 => 0,
        1 => fracao.parse::<i64>().ok()? * 10,
        _ => fracao[..2].parse().ok()?,
    };
    let v = inteiro.checked_mul(100)?.checked_add(centavos)?;
    Some(if negativo { -v } else { v })
}

/// Lê datas "dd/mm/aaaa", "dd/mm/aa", "aaaa-mm-dd", "dd-mm-aaaa" e "aaaammdd" (OFX).
pub fn ler_data(texto: &str) -> Option<NaiveDate> {
    let t = texto.split_whitespace().next().unwrap_or("");
    let t = t.split('T').next().unwrap_or(t);
    // dd/mm/aa precisa vir antes: "%Y" aceitaria "26" como ano 26.
    let ano_curto = t.len() == 8 && t.as_bytes().get(2) == Some(&b'/') && t.as_bytes().get(5) == Some(&b'/');
    if ano_curto {
        return NaiveDate::parse_from_str(t, "%d/%m/%y").ok();
    }
    for formato in ["%d/%m/%Y", "%Y-%m-%d", "%d-%m-%Y", "%d.%m.%Y", "%Y/%m/%d"] {
        if let Ok(d) = NaiveDate::parse_from_str(t, formato) {
            return Some(d);
        }
    }
    if t.len() >= 8 && t[..8].chars().all(|c| c.is_ascii_digit()) {
        return NaiveDate::parse_from_str(&t[..8], "%Y%m%d").ok();
    }
    None
}

/// Lê o arquivo, detectando se é OFX ou CSV.
pub fn ler(bytes: &[u8], mapa: Option<MapaColunas>) -> Resultado<ExtratoLido> {
    let texto = decodificar(bytes);
    let inicio = texto.trim_start().to_uppercase();
    if inicio.starts_with("OFXHEADER") || inicio.starts_with("<?XML") || texto.to_uppercase().contains("<OFX>") {
        ler_ofx(&texto)
    } else {
        ler_csv(&texto, mapa)
    }
}

fn campo_ofx(bloco: &str, tag: &str) -> Option<String> {
    let maiusculo = bloco.to_uppercase();
    let abre = format!("<{tag}>");
    let i = maiusculo.find(&abre)? + abre.len();
    let resto = &bloco[i..];
    let fim = resto.find(['<', '\n', '\r']).unwrap_or(resto.len());
    let valor = resto[..fim].trim();
    (!valor.is_empty()).then(|| valor.replace("&amp;", "&"))
}

/// OFX de conta corrente ou de cartão (SGML do BB ou XML).
pub fn ler_ofx(texto: &str) -> Resultado<ExtratoLido> {
    let maiusculo = texto.to_uppercase();
    let mut linhas = Vec::new();
    let mut ignoradas = 0;
    let mut pos = 0;
    while let Some(i) = maiusculo[pos..].find("<STMTTRN>") {
        let ini = pos + i;
        let fim = maiusculo[ini..].find("</STMTTRN>").map(|f| ini + f).unwrap_or(maiusculo.len());
        let bloco = &texto[ini..fim];
        pos = fim.max(ini + 9);

        let data = campo_ofx(bloco, "DTPOSTED").and_then(|d| ler_data(&d));
        let valor = campo_ofx(bloco, "TRNAMT").and_then(|v| ler_valor(&v));
        let descricao = campo_ofx(bloco, "MEMO")
            .or_else(|| campo_ofx(bloco, "NAME"))
            .unwrap_or_else(|| "Sem descrição".into());
        match (data, valor) {
            (Some(data), Some(valor)) if valor != 0 => linhas.push(LinhaExtrato {
                data,
                descricao,
                valor,
                id_externo: campo_ofx(bloco, "FITID"),
            }),
            _ => ignoradas += 1,
        }
    }
    if linhas.is_empty() {
        return invalido("não encontrei lançamentos nesse OFX");
    }
    Ok(ExtratoLido { formato: Formato::Ofx, linhas, cabecalho: Vec::new(), mapa: None, ignoradas })
}

fn separador(texto: &str) -> u8 {
    let amostra: String = texto.lines().take(10).collect::<Vec<_>>().join("\n");
    let contar = |c: char| amostra.matches(c).count();
    if contar(';') >= contar(',') && contar(';') > 0 {
        b';'
    } else if contar('\t') > contar(',') {
        b'\t'
    } else {
        b','
    }
}

/// Descobre as colunas pelo nome do cabeçalho.
pub fn adivinhar_mapa(cabecalho: &[String]) -> Option<MapaColunas> {
    let h: Vec<String> = cabecalho.iter().map(|c| normalizar(c)).collect();
    let achar = |pred: &dyn Fn(&str) -> bool| h.iter().position(|c| pred(c));

    let data = achar(&|c| c == "data")
        .or_else(|| achar(&|c| c.starts_with("data") && !c.contains("balancete")))?;
    let descricao = ["descricao", "historico", "lancamento", "estabelecimento", "detalhes", "titulo", "memo"]
        .iter()
        .find_map(|p| achar(&|c| c.contains(p) && !c.starts_with("data") && !c.starts_with("tipo")))?;
    let valor = achar(&|c| c.contains("valor") && c.contains("r"))
        .filter(|&i| !h[i].contains("us") && !h[i].contains("dolar"))
        .or_else(|| achar(&|c| c.contains("valor") && !c.contains("us") && !c.contains("dolar")))
        .or_else(|| achar(&|c| c == "montante" || c == "amount"))?;
    let tipo = achar(&|c| {
        c == "tipo" || c == "d c" || c.contains("natureza") || c == "tipo lancamento" || c == "tipo de lancamento"
    })
    .filter(|&i| i != descricao);
    Some(MapaColunas { data, descricao, valor, tipo })
}

/// CSV com cabeçalho. Sem `mapa`, as colunas são descobertas pelo nome.
pub fn ler_csv(texto: &str, mapa: Option<MapaColunas>) -> Resultado<ExtratoLido> {
    let mut leitor = csv::ReaderBuilder::new()
        .delimiter(separador(texto))
        .has_headers(false)
        .flexible(true)
        .from_reader(texto.as_bytes());
    let registros: Vec<Vec<String>> = leitor
        .records()
        .filter_map(|r| r.ok())
        .map(|r| r.iter().map(|c| c.trim().to_string()).collect())
        .collect();

    // O cabeçalho é a primeira linha que tem uma coluna "data…" (o BB às vezes tem linhas antes).
    let idx_cabecalho = registros
        .iter()
        .position(|r| r.iter().any(|c| normalizar(c).starts_with("data")));
    let (cabecalho, corpo) = match idx_cabecalho {
        Some(i) => (registros[i].clone(), &registros[i + 1..]),
        None => (Vec::new(), &registros[..]),
    };
    let mapa = match mapa.or_else(|| adivinhar_mapa(&cabecalho)) {
        Some(m) => m,
        None => return invalido("não reconheci as colunas desse CSV; escolha data, descrição e valor"),
    };

    let mut linhas = Vec::new();
    let mut ignoradas = 0;
    for r in corpo {
        let celula = |i: usize| r.get(i).map(String::as_str).unwrap_or("");
        let data = ler_data(celula(mapa.data));
        let mut valor = ler_valor(celula(mapa.valor));
        if let (Some(v), Some(t)) = (valor, mapa.tipo) {
            let tipo = normalizar(celula(t));
            if tipo.starts_with('d') || tipo.starts_with("saida") || tipo.starts_with("debito") {
                valor = Some(-v.abs());
            } else if tipo.starts_with('c') || tipo.starts_with("entrada") || tipo.starts_with("credito") {
                valor = Some(v.abs());
            }
        }
        let descricao = celula(mapa.descricao).to_string();
        match (data, valor) {
            (Some(data), Some(valor)) if valor != 0 => linhas.push(LinhaExtrato {
                data,
                descricao: if descricao.is_empty() { "Sem descrição".into() } else { descricao },
                valor,
                id_externo: None,
            }),
            _ => {
                if r.iter().any(|c| !c.is_empty()) {
                    ignoradas += 1;
                }
            }
        }
    }
    if linhas.is_empty() {
        return invalido("não encontrei lançamentos nesse CSV; confira as colunas escolhidas");
    }
    Ok(ExtratoLido { formato: Formato::Csv, linhas, cabecalho, mapa: Some(mapa), ignoradas })
}

/// Identificador estável de uma linha, para não importar duas vezes.
/// `ocorrencia` separa lançamentos idênticos no mesmo arquivo (dois cafés no mesmo dia).
pub fn hash_linha(conta_id: i64, linha: &LinhaExtrato, ocorrencia: usize) -> String {
    let base = format!(
        "{conta_id}|{}|{}|{}|{}|{ocorrencia}",
        linha.data,
        linha.valor,
        normalizar(&linha.descricao),
        linha.id_externo.as_deref().unwrap_or("")
    );
    // FNV-1a 64 bits: estável entre versões e máquinas.
    let mut h: u64 = 0xcbf29ce484222325;
    for b in base.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

/// Motivo para vir desmarcada na prévia (o usuário pode marcar mesmo assim).
pub fn motivo_ignorar(descricao: &str, valor: i64, eh_cartao: bool) -> Option<&'static str> {
    let d = normalizar(descricao);
    if d.replace(' ', "").contains("saldo") {
        return Some("linha de saldo");
    }
    let pagamento = d.contains("pagamento") || d.contains("pagto") || d.contains("pgto");
    let fatura = d.contains("fatura") || d.contains("cartao") || d.contains("card");
    if !eh_cartao && valor < 0 && pagamento && fatura {
        return Some("pagamento de fatura: os gastos já entram pelo cartão");
    }
    if eh_cartao && valor > 0 {
        return Some(if pagamento { "pagamento da fatura" } else { "crédito ou estorno no cartão" });
    }
    if d.contains("aplicacao") || d.contains("resgate") || d.contains("rende facil") {
        return Some("aplicação ou resgate");
    }
    None
}

/// Detecta "02/10" ou "PARC 02/10" na descrição de fatura.
pub fn parcela_na_descricao(descricao: &str) -> Option<String> {
    descricao.split_whitespace().find_map(|palavra| {
        let (a, b) = palavra.split_once('/')?;
        let curto = |x: &str| (1..=2).contains(&x.len()) && x.chars().all(|c| c.is_ascii_digit());
        if !curto(a) || !curto(b) {
            return None;
        }
        let (n, total): (u32, u32) = (a.parse().ok()?, b.parse().ok()?);
        (n >= 1 && total >= 2 && n <= total).then(|| format!("{n:02}/{total:02}"))
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn valores_em_varios_formatos() {
        assert_eq!(ler_valor("1.234,56"), Some(123456));
        assert_eq!(ler_valor("-1.234,56"), Some(-123456));
        assert_eq!(ler_valor("-1234.56"), Some(-123456));
        assert_eq!(ler_valor("R$ 12,00"), Some(1200));
        assert_eq!(ler_valor("R$ -12,5"), Some(-1250));
        assert_eq!(ler_valor("(12,00)"), Some(-1200));
        assert_eq!(ler_valor("12,00 D"), Some(-1200));
        assert_eq!(ler_valor("1.234"), Some(123400));
        assert_eq!(ler_valor("-50.00"), Some(-5000));
        assert_eq!(ler_valor("1,234.56"), Some(123456));
        assert_eq!(ler_valor(""), None);
        assert_eq!(ler_valor("abc"), None);
    }

    #[test]
    fn datas_em_varios_formatos() {
        assert_eq!(ler_data("05/11/2026"), Some(d("2026-11-05")));
        assert_eq!(ler_data("05/11/26"), Some(d("2026-11-05")));
        assert_eq!(ler_data("2026-11-05"), Some(d("2026-11-05")));
        assert_eq!(ler_data("20261105120000[-3:BRT]"), Some(d("2026-11-05")));
        assert_eq!(ler_data("2026-11-05T10:00:00"), Some(d("2026-11-05")));
        assert_eq!(ler_data("Saldo"), None);
    }

    #[test]
    fn ofx_sgml_do_bb() {
        let ofx = "OFXHEADER:100\nDATA:OFXSGML\nCHARSET:1252\n\n<OFX>\n<BANKMSGSRSV1><STMTTRNRS><STMTRS>\n<BANKTRANLIST>\n\
<STMTTRN>\n<TRNTYPE>DEBIT\n<DTPOSTED>20261103120000[-3:BRT]\n<TRNAMT>-45.90\n<FITID>2026110301\n<MEMO>Compra com Cartão - SUPERMERCADO\n</STMTTRN>\n\
<STMTTRN>\n<TRNTYPE>CREDIT\n<DTPOSTED>20261105\n<TRNAMT>1500.00\n<FITID>2026110502\n<MEMO>Pix - Recebido JOAO\n</STMTTRN>\n\
</BANKTRANLIST></STMTRS></STMTTRNRS></BANKMSGSRSV1></OFX>";
        let e = ler(ofx.as_bytes(), None).unwrap();
        assert_eq!(e.formato, Formato::Ofx);
        assert_eq!(e.linhas.len(), 2);
        assert_eq!(e.linhas[0].valor, -4590);
        assert_eq!(e.linhas[0].descricao, "Compra com Cartão - SUPERMERCADO");
        assert_eq!(e.linhas[0].id_externo.as_deref(), Some("2026110301"));
        assert_eq!(e.linhas[1].data, d("2026-11-05"));
        assert_eq!(e.linhas[1].valor, 150000);
    }

    #[test]
    fn csv_extrato_bb_latin1_com_saldo() {
        let texto = "\"Data\",\"Dependencia Origem\",\"Histórico\",\"Data do Balancete\",\"Número do documento\",\"Valor\",\n\
\"31/10/2026\",\"\",\"Saldo Anterior\",\"\",\"0\",\"1200.00\",\n\
\"03/11/2026\",\"\",\"Compra com Cartão - PADARIA\",\"\",\"123\",\"-12.50\",\n\
\"05/11/2026\",\"\",\"Pix - Recebido\",\"\",\"456\",\"300.00\",\n\
\"05/11/2026\",\"\",\"S A L D O\",\"\",\"0\",\"1487.50\",\n";
        let bytes: Vec<u8> = texto.chars().map(|c| c as u32 as u8).collect(); // Latin-1
        let e = ler(&bytes, None).unwrap();
        assert_eq!(e.formato, Formato::Csv);
        assert_eq!(e.mapa, Some(MapaColunas { data: 0, descricao: 2, valor: 5, tipo: None }));
        assert_eq!(e.linhas.len(), 4);
        assert_eq!(e.linhas[1].descricao, "Compra com Cartão - PADARIA");
        assert_eq!(e.linhas[1].valor, -1250);
        assert_eq!(motivo_ignorar(&e.linhas[0].descricao, e.linhas[0].valor, false), Some("linha de saldo"));
        assert_eq!(motivo_ignorar(&e.linhas[3].descricao, e.linhas[3].valor, false), Some("linha de saldo"));
        assert_eq!(motivo_ignorar(&e.linhas[1].descricao, e.linhas[1].valor, false), None);
    }

    #[test]
    fn csv_picpay_com_ponto_e_virgula_e_coluna_de_tipo() {
        let texto = "Data;Descrição;Tipo;Valor (R$)\n02/11/2026 14:10;Netflix;Saída;55,90\n04/11/2026;Pix recebido de Maria;Entrada;100,00\n";
        let e = ler(texto.as_bytes(), None).unwrap();
        assert_eq!(e.mapa, Some(MapaColunas { data: 0, descricao: 1, valor: 3, tipo: Some(2) }));
        assert_eq!(e.linhas[0].valor, -5590);
        assert_eq!(e.linhas[0].data, d("2026-11-02"));
        assert_eq!(e.linhas[1].valor, 10000);
    }

    #[test]
    fn csv_com_mapa_escolhido_pelo_usuario() {
        let texto = "quando,o que,quanto\n01/11/2026,Coisa,-10,00\n";
        assert!(ler(texto.as_bytes(), None).is_err() || ler(texto.as_bytes(), None).unwrap().linhas.is_empty());
        let texto = "x;y;z\n01/11/2026;Coisa;-10,00\n";
        let e = ler(texto.as_bytes(), Some(MapaColunas { data: 0, descricao: 1, valor: 2, tipo: None })).unwrap();
        assert_eq!(e.linhas.len(), 1);
        assert_eq!(e.linhas[0].valor, -1000);
    }

    #[test]
    fn hash_estavel_e_diferente_por_ocorrencia() {
        let l = LinhaExtrato { data: d("2026-11-01"), descricao: "Café".into(), valor: -500, id_externo: None };
        assert_eq!(hash_linha(1, &l, 0), hash_linha(1, &l, 0));
        assert_ne!(hash_linha(1, &l, 0), hash_linha(1, &l, 1));
        assert_ne!(hash_linha(1, &l, 0), hash_linha(2, &l, 0));
    }

    #[test]
    fn detecta_pagamento_de_fatura_e_parcela() {
        assert!(motivo_ignorar("Pagto cartão crédito", -50000, false).is_some());
        assert!(motivo_ignorar("PAGAMENTO RECEBIDO", 50000, true).is_some());
        assert!(motivo_ignorar("MERCADO", -5000, true).is_none());
        assert_eq!(parcela_na_descricao("LOJA X PARC 02/10"), Some("02/10".into()));
        assert_eq!(parcela_na_descricao("LOJA 3/12"), Some("03/12".into()));
        assert_eq!(parcela_na_descricao("Compra 05/11/2026"), None);
        assert_eq!(parcela_na_descricao("SUPERMERCADO"), None);
    }

    #[test]
    fn chave_ignora_numeros() {
        assert_eq!(chave_descricao("NETFLIX.COM 0811"), chave_descricao("Netflix.com 0812"));
        assert_eq!(chave_descricao("Pagamento Claude.ai*Sub 12/11"), "pagamento claude ai sub");
    }
}
