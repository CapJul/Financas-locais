use std::fmt;
use std::str::FromStr;

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::erro::Erro;

/// Mês de referência de uma fatura, no formato `AAAA-MM`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Competencia {
    pub ano: i32,
    /// 1 a 12
    pub mes: u32,
}

impl Competencia {
    pub fn nova(ano: i32, mes: u32) -> Result<Self, Erro> {
        if !(1..=12).contains(&mes) {
            return Err(Erro::Validacao(format!("mês inválido: {mes}")));
        }
        Ok(Self { ano, mes })
    }

    pub fn de_data(data: NaiveDate) -> Self {
        Self { ano: data.year(), mes: data.month() }
    }

    pub fn atual() -> Self {
        Self::de_data(chrono::Local::now().date_naive())
    }

    /// Soma (ou subtrai, se negativo) meses.
    pub fn somar_meses(self, meses: i32) -> Self {
        let indice = self.ano * 12 + (self.mes as i32 - 1) + meses;
        Self { ano: indice.div_euclid(12), mes: indice.rem_euclid(12) as u32 + 1 }
    }
}

impl fmt::Display for Competencia {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}", self.ano, self.mes)
    }
}

impl FromStr for Competencia {
    type Err = Erro;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let erro = || Erro::Validacao(format!("competência inválida (use AAAA-MM): {s}"));
        let (ano, mes) = s.split_once('-').ok_or_else(erro)?;
        let ano: i32 = ano.parse().map_err(|_| erro())?;
        let mes: u32 = mes.parse().map_err(|_| erro())?;
        Competencia::nova(ano, mes).map_err(|_| erro())
    }
}

impl Serialize for Competencia {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Competencia {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn soma_meses_vira_o_ano() {
        let c = Competencia::nova(2026, 11).unwrap();
        assert_eq!(c.somar_meses(3).to_string(), "2027-02");
        assert_eq!(c.somar_meses(-11).to_string(), "2025-12");
        assert_eq!(c.somar_meses(0), c);
    }

    #[test]
    fn le_e_escreve_texto() {
        let c: Competencia = "2027-01".parse().unwrap();
        assert_eq!(c, Competencia::nova(2027, 1).unwrap());
        assert!("2027-13".parse::<Competencia>().is_err());
        assert!("abc".parse::<Competencia>().is_err());
    }
}
