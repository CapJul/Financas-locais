// Formatação e conversões usadas pelas telas.

const fmt = new Intl.NumberFormat("pt-BR", { style: "currency", currency: "BRL" });

/** 2056 → "R$ 20,56" */
export function reais(centavos: number): string {
  return fmt.format(centavos / 100);
}

/**
 * Converte o que o usuário digitou em centavos.
 * Aceita "1.234,56", "1234,56", "1234.56", "R$ 20" etc. Retorna null se inválido.
 */
export function lerReais(texto: string): number | null {
  let t = texto.replace(/R\$|\s/g, "");
  if (t.includes(",")) t = t.replace(/\./g, "").replace(",", ".");
  if (!/^\d+(\.\d{1,2})?$/.test(t)) return null;
  return Math.round(parseFloat(t) * 100);
}

/** Centavos → texto editável "20,56" */
export function paraCampo(centavos: number): string {
  return (centavos / 100).toFixed(2).replace(".", ",");
}

/** Data de hoje em AAAA-MM-DD (fuso local). */
export function hoje(): string {
  const d = new Date();
  const mm = String(d.getMonth() + 1).padStart(2, "0");
  const dd = String(d.getDate()).padStart(2, "0");
  return `${d.getFullYear()}-${mm}-${dd}`;
}

/** Competência atual em AAAA-MM. */
export function competenciaAtual(): string {
  return hoje().slice(0, 7);
}

export function somarMeses(competencia: string, meses: number): string {
  const [a, m] = competencia.split("-").map(Number);
  const indice = a * 12 + (m - 1) + meses;
  const ano = Math.floor(indice / 12);
  const mes = (indice % 12) + 1;
  return `${ano}-${String(mes).padStart(2, "0")}`;
}

const MESES = ["Jan", "Fev", "Mar", "Abr", "Mai", "Jun", "Jul", "Ago", "Set", "Out", "Nov", "Dez"];

/** "2026-11" → "Nov/26" */
export function nomeMes(competencia: string): string {
  const [a, m] = competencia.split("-").map(Number);
  return `${MESES[m - 1]}/${String(a).slice(2)}`;
}

/** "2026-11-15" → "15/11/2026" */
export function dataBR(data: string): string {
  const [a, m, d] = data.split("-");
  return `${d}/${m}/${a}`;
}

/** Mensagem legível para erros vindos do Rust (strings) ou do JS. */
export function mensagemErro(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
