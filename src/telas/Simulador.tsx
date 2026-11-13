import { FormEvent, useEffect, useState } from "react";
import {
  Bar,
  BarChart,
  CartesianGrid,
  Legend,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type { Base } from "../App";
import { api, ComparacaoGeral } from "../api";
import { competenciaAtual, hoje, lerReais, mensagemErro, nomeMes, reais } from "../util";

const CORES = ["#9aa5b1", "#7b8794", "#b8c2cc", "#616e7c", "#cbd2d9", "#52606d"];

/**
 * "Se eu comprar o produto X no valor Y em Z vezes, como ficam as faturas?"
 * Soma todos os cartões, para mostrar quanto a mais você vai pagar no total. Nada é gravado.
 */
export default function Simulador({ base }: { base: Base }) {
  const { contas } = base;
  const [contaId, setContaId] = useState<number | "">("");
  const [produto, setProduto] = useState("");
  const [valor, setValor] = useState("");
  const [parcelas, setParcelas] = useState(1);
  const [data, setData] = useState(hoje());
  const [meses, setMeses] = useState(6);
  const [resultado, setResultado] = useState<ComparacaoGeral[] | null>(null);
  const [erro, setErro] = useState<string | null>(null);

  useEffect(() => {
    if (contaId === "" && contas.length > 0) {
      setContaId((contas.find((c) => c.tipo === "cartao") ?? contas[0]).id);
    }
  }, [contas, contaId]);

  async function simular(e: FormEvent) {
    e.preventDefault();
    setErro(null);
    const total = lerReais(valor);
    if (contaId === "") return setErro("Escolha uma conta.");
    if (total === null || total <= 0) return setErro("Valor inválido. Ex.: 1.299,90");
    const mesesNecessarios = Math.max(meses, parcelas + 1);
    try {
      const r = await api.simularCompraTodas(
        { contaId, valorTotal: total, numParcelas: parcelas, dataCompra: data },
        competenciaAtual(),
        Math.min(mesesNecessarios, 36),
      );
      // Corta os meses vazios do fim, para o gráfico não ficar com espaço sobrando.
      let fim = r.length;
      while (fim > 1 && r[fim - 1].atual === 0 && r[fim - 1].simulado === 0) fim--;
      setResultado(r.slice(0, fim));
    } catch (e) {
      setErro(mensagemErro(e));
    }
  }

  const contasNoGrafico = resultado?.[0]?.porConta.map((t) => t.nome) ?? [];
  const dados = (resultado ?? []).map((r) => {
    const linha: Record<string, string | number> = {
      mes: nomeMes(r.competencia),
      comCompra: r.simulado / 100,
    };
    r.porConta.forEach((t) => (linha[`hoje:${t.nome}`] = t.valor / 100));
    return linha;
  });
  const aMais = (resultado ?? []).reduce((s, r) => s + (r.simulado - r.atual), 0);
  const maior = (resultado ?? []).reduce<ComparacaoGeral | null>(
    (m, r) => (!m || r.simulado > m.simulado ? r : m),
    null,
  );
  const mesesAfetados = (resultado ?? []).filter((r) => r.simulado > r.atual).length;

  return (
    <section>
      <h2>Simulador de compra</h2>
      <form className="cartao formulario" onSubmit={simular}>
        <div className="grade">
          <label>
            Conta
            <select value={contaId} onChange={(e) => setContaId(Number(e.target.value))}>
              {contas.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.nome}
                </option>
              ))}
            </select>
          </label>
          <label className="largo">
            Produto (opcional)
            <input value={produto} onChange={(e) => setProduto(e.target.value)} />
          </label>
          <label>
            Valor (R$)
            <input
              value={valor}
              onChange={(e) => setValor(e.target.value)}
              placeholder="0,00"
              inputMode="decimal"
              required
            />
          </label>
          <label>
            Parcelas
            <input
              type="number"
              min={1}
              max={120}
              value={parcelas}
              onChange={(e) => setParcelas(Math.max(1, Number(e.target.value) || 1))}
            />
          </label>
          <label>
            Data da compra
            <input type="date" value={data} onChange={(e) => setData(e.target.value)} />
          </label>
          <label>
            Mostrar
            <select value={meses} onChange={(e) => setMeses(Number(e.target.value))}>
              {[3, 6, 12, 24].map((m) => (
                <option key={m} value={m}>
                  {m} meses
                </option>
              ))}
            </select>
          </label>
        </div>
        {erro && <div className="aviso erro">{erro}</div>}
        <button type="submit" className="primario" disabled={contas.length === 0}>
          Simular
        </button>
      </form>

      {resultado && (
        <>
          <div className="tres-colunas">
            <div className="destaque">
              <span>{produto ? `"${produto}" acrescenta` : "A compra acrescenta"}</span>
              <strong className="negativo">{reais(aMais)}</strong>
            </div>
            <div className="destaque">
              <span>Meses afetados</span>
              <strong>{mesesAfetados}</strong>
            </div>
            <div className="destaque">
              <span>Maior total de faturas</span>
              <strong>{maior ? `${reais(maior.simulado)} em ${nomeMes(maior.competencia)}` : "—"}</strong>
            </div>
          </div>
          <p className="dica">
            Soma das faturas de todos os cartões{contasNoGrafico.length > 0 && ` (${contasNoGrafico.join(", ")})`}.
            Nada foi salvo.
          </p>
          <div className="cartao grafico">
            <ResponsiveContainer width="100%" height={300}>
              <BarChart data={dados}>
                <CartesianGrid strokeDasharray="3 3" vertical={false} />
                <XAxis dataKey="mes" />
                <YAxis tickFormatter={(v: number) => `R$ ${v}`} width={80} />
                <Tooltip formatter={(v: number) => reais(Math.round(v * 100))} />
                <Legend />
                {contasNoGrafico.map((nome, i) => (
                  <Bar
                    key={nome}
                    dataKey={`hoje:${nome}`}
                    name={`Hoje · ${nome}`}
                    stackId="hoje"
                    fill={CORES[i % CORES.length]}
                  />
                ))}
                <Bar dataKey="comCompra" name="Total com a compra" fill="#e15759" radius={[4, 4, 0, 0]} />
              </BarChart>
            </ResponsiveContainer>
          </div>
          <table>
            <thead>
              <tr>
                <th>Mês</th>
                {contasNoGrafico.map((n) => (
                  <th key={n} className="num">
                    {n}
                  </th>
                ))}
                <th className="num">Total hoje</th>
                <th className="num">Com a compra</th>
                <th className="num">A mais</th>
              </tr>
            </thead>
            <tbody>
              {resultado.map((r) => (
                <tr key={r.competencia}>
                  <td>{nomeMes(r.competencia)}</td>
                  {r.porConta.map((t) => (
                    <td key={t.nome} className="num">
                      {reais(t.valor)}
                      {r.pagas.includes(t.nome) && <span className="etiqueta">paga</span>}
                    </td>
                  ))}
                  <td className="num">{reais(r.atual)}</td>
                  <td className="num">
                    <strong>{reais(r.simulado)}</strong>
                  </td>
                  <td className="num negativo">
                    {r.simulado > r.atual ? `+${reais(r.simulado - r.atual)}` : "—"}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}
    </section>
  );
}
