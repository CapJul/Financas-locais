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
import { api, ComparacaoMes } from "../api";
import { competenciaAtual, hoje, lerReais, mensagemErro, nomeMes, reais } from "../util";

/** "Se eu comprar o produto X no valor Y em Z vezes, como ficam as faturas?" Nada é gravado. */
export default function Simulador({ base }: { base: Base }) {
  const { contas } = base;
  const [contaId, setContaId] = useState<number | "">("");
  const [produto, setProduto] = useState("");
  const [valor, setValor] = useState("");
  const [parcelas, setParcelas] = useState(1);
  const [data, setData] = useState(hoje());
  const [meses, setMeses] = useState(6);
  const [resultado, setResultado] = useState<ComparacaoMes[] | null>(null);
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
      setResultado(
        await api.simularCompra(
          { contaId, valorTotal: total, numParcelas: parcelas, dataCompra: data },
          competenciaAtual(),
          Math.min(mesesNecessarios, 36),
        ),
      );
    } catch (e) {
      setErro(mensagemErro(e));
    }
  }

  const dados = (resultado ?? []).map((r) => ({
    mes: nomeMes(r.competencia),
    atual: r.atual / 100,
    comCompra: r.simulado / 100,
  }));
  const maiorFatura = resultado ? Math.max(...resultado.map((r) => r.simulado)) : 0;

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
          <p className="dica">
            {produto ? `Com "${produto}"` : "Com a compra"}, a maior fatura do período fica em{" "}
            <strong>{reais(maiorFatura)}</strong>. Nada foi salvo.
          </p>
          <div className="cartao grafico">
            <ResponsiveContainer width="100%" height={280}>
              <BarChart data={dados}>
                <CartesianGrid strokeDasharray="3 3" vertical={false} />
                <XAxis dataKey="mes" />
                <YAxis tickFormatter={(v: number) => `R$ ${v}`} width={80} />
                <Tooltip formatter={(v: number) => reais(Math.round(v * 100))} />
                <Legend />
                <Bar dataKey="atual" name="Hoje" fill="#9aa5b1" radius={[4, 4, 0, 0]} />
                <Bar dataKey="comCompra" name="Com a compra" fill="#e15759" radius={[4, 4, 0, 0]} />
              </BarChart>
            </ResponsiveContainer>
          </div>
          <table>
            <thead>
              <tr>
                <th>Mês</th>
                <th className="num">Hoje</th>
                <th className="num">Com a compra</th>
                <th className="num">Diferença</th>
              </tr>
            </thead>
            <tbody>
              {resultado.map((r) => (
                <tr key={r.competencia}>
                  <td>{nomeMes(r.competencia)}</td>
                  <td className="num">{reais(r.atual)}</td>
                  <td className="num">{reais(r.simulado)}</td>
                  <td className="num">
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
