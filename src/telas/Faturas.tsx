import { useEffect, useMemo, useState } from "react";
import {
  Bar,
  BarChart,
  CartesianGrid,
  Cell,
  Legend,
  Pie,
  PieChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type { Base } from "../App";
import { api, DetalheMes, StatusFatura, TotalNomeado } from "../api";
import { competenciaAtual, dataBR, hoje, mensagemErro, nomeMes, reais } from "../util";

const CORES_ORIGEM: Record<string, string> = {
  "À vista": "#4e79a7",
  Parcelado: "#f28e2b",
  Mensalidades: "#76b7b2",
};
const PALETA = ["#4e79a7", "#e15759", "#59a14f", "#edc948", "#b07aa1", "#ff9da7", "#9c755f", "#76b7b2"];
const SEM_CATEGORIA = "#9aa5b1";

const emReais = (v: number) => reais(Math.round(v * 100));
const eixoReais = (v: number) => `R$ ${v}`;

/** Transforma os detalhes em linhas para um gráfico empilhado e devolve as séries. */
function empilhar(detalhes: DetalheMes[], campo: (d: DetalheMes) => TotalNomeado[]) {
  const series: string[] = [];
  const linhas = detalhes.map((d) => {
    const linha: Record<string, string | number> = { mes: nomeMes(d.competencia) };
    for (const t of campo(d)) {
      if (!series.includes(t.nome)) series.push(t.nome);
      linha[t.nome] = t.valor / 100;
    }
    return linha;
  });
  return { linhas, series };
}

/** Faturas projetadas de um cartão, com a composição de cada mês. */
export default function Faturas({ base }: { base: Base }) {
  const cartoes = base.contas.filter((c) => c.tipo === "cartao");
  const [contaId, setContaId] = useState<number | "">("");
  const [meses, setMeses] = useState(6);
  const [detalhes, setDetalhes] = useState<DetalheMes[]>([]);
  const [status, setStatus] = useState<StatusFatura[]>([]);
  // Fatura sendo marcada como paga: competência + data escolhida.
  const [pagando, setPagando] = useState<{ competencia: string; data: string } | null>(null);
  const [mesSelecionado, setMesSelecionado] = useState(0);
  const [erro, setErro] = useState<string | null>(null);

  useEffect(() => {
    if (contaId === "" && cartoes.length > 0) setContaId(cartoes[0].id);
  }, [cartoes, contaId]);

  const carregarStatus = (id: number) =>
    api.statusFaturas(id, competenciaAtual(), meses).then(setStatus);

  useEffect(() => {
    if (contaId === "") return;
    Promise.all([api.detalharFaturas(contaId, competenciaAtual(), meses), carregarStatus(contaId)])
      .then(([d]) => {
        setDetalhes(d);
        // Começa mostrando a primeira fatura que tem algum valor.
        setMesSelecionado(Math.max(0, d.findIndex((x) => x.total > 0)));
        setErro(null);
      })
      .catch((e) => setErro(mensagemErro(e)));
  }, [contaId, meses]);

  async function marcarPaga() {
    if (!pagando || contaId === "") return;
    try {
      await api.marcarFaturaPaga(contaId, pagando.competencia, pagando.data, null);
      setPagando(null);
      await carregarStatus(contaId);
    } catch (e) {
      setErro(mensagemErro(e));
    }
  }

  async function desmarcar(competencia: string) {
    if (contaId === "") return;
    try {
      await api.desmarcarFaturaPaga(contaId, competencia);
      await carregarStatus(contaId);
    } catch (e) {
      setErro(mensagemErro(e));
    }
  }

  const statusDe = (competencia: string) => status.find((s) => s.competencia === competencia);

  const corCategoria = useMemo(() => {
    const mapa = new Map(base.categorias.map((c) => [c.nome, c.cor]));
    return (nome: string) => mapa.get(nome) ?? SEM_CATEGORIA;
  }, [base.categorias]);
  const corPessoa = (nome: string) => {
    const i = base.pessoas.findIndex((p) => p.nome === nome);
    return PALETA[(i < 0 ? 0 : i) % PALETA.length];
  };

  const origem = useMemo(() => empilhar(detalhes, (d) => d.porOrigem), [detalhes]);
  const categoria = useMemo(() => empilhar(detalhes, (d) => d.porCategoria), [detalhes]);
  const pessoa = useMemo(() => empilhar(detalhes, (d) => d.porPessoa), [detalhes]);

  const conta = cartoes.find((c) => c.id === contaId);
  const selecionado = detalhes[mesSelecionado];
  // O que ainda falta pagar: faturas do período que não foram marcadas como pagas.
  const totalPeriodo = detalhes.reduce((s, d) => s + (statusDe(d.competencia)?.paga ? 0 : d.total), 0);
  const proxima = detalhes.find((d) => !statusDe(d.competencia)?.paga && d.total > 0);
  const maior = detalhes.reduce<DetalheMes | null>((m, d) => (!m || d.total > m.total ? d : m), null);

  if (cartoes.length === 0) {
    return (
      <section>
        <h2>Faturas</h2>
        <p className="vazio">Cadastre um cartão (com dia de fechamento) para ver as faturas.</p>
      </section>
    );
  }

  return (
    <section>
      <header className="cabecalho">
        <h2>Próximas faturas</h2>
        <div className="filtros">
          <select value={contaId} onChange={(e) => setContaId(Number(e.target.value))}>
            {cartoes.map((c) => (
              <option key={c.id} value={c.id}>
                {c.nome}
              </option>
            ))}
          </select>
          <select value={meses} onChange={(e) => setMeses(Number(e.target.value))}>
            {[3, 6, 12, 24].map((m) => (
              <option key={m} value={m}>
                {m} meses
              </option>
            ))}
          </select>
        </div>
      </header>
      {conta && (
        <p className="dica">
          Fecha dia {conta.diaFechamento}, vence dia {conta.diaVencimento}. Compras a partir do
          dia {conta.diaFechamento} entram na fatura do mês seguinte.
        </p>
      )}
      {erro && <div className="aviso erro">{erro}</div>}

      <div className="tres-colunas">
        <div className="destaque">
          <span>
            Próxima a pagar
            {proxima &&
              ` (${nomeMes(proxima.competencia)}${
                statusDe(proxima.competencia)?.vencimento
                  ? `, vence ${dataBR(statusDe(proxima.competencia)!.vencimento!)}`
                  : ""
              })`}
          </span>
          <strong>{proxima ? reais(proxima.total) : "nada em aberto"}</strong>
        </div>
        <div className="destaque">
          <span>Maior fatura do período</span>
          <strong>
            {maior ? `${reais(maior.total)} em ${nomeMes(maior.competencia)}` : "—"}
          </strong>
        </div>
        <div className="destaque">
          <span>Falta pagar em {meses} meses</span>
          <strong>{reais(totalPeriodo)}</strong>
        </div>
      </div>

      <div className="cartao grafico">
        <h3>Faturas por tipo de gasto</h3>
        <p className="dica">Clique numa barra para ver o detalhe daquele mês abaixo.</p>
        <ResponsiveContainer width="100%" height={280}>
          <BarChart
            data={origem.linhas}
            onClick={(e) => {
              if (e && typeof e.activeTooltipIndex === "number") setMesSelecionado(e.activeTooltipIndex);
            }}
          >
            <CartesianGrid strokeDasharray="3 3" vertical={false} />
            <XAxis dataKey="mes" />
            <YAxis tickFormatter={eixoReais} width={80} />
            <Tooltip formatter={emReais} />
            <Legend />
            {origem.series.map((s) => (
              <Bar key={s} dataKey={s} stackId="o" fill={CORES_ORIGEM[s] ?? SEM_CATEGORIA} cursor="pointer" />
            ))}
          </BarChart>
        </ResponsiveContainer>
      </div>

      <div className="colunas">
        <div className="cartao grafico">
          <h3>Por categoria</h3>
          <ResponsiveContainer width="100%" height={260}>
            <BarChart data={categoria.linhas}>
              <CartesianGrid strokeDasharray="3 3" vertical={false} />
              <XAxis dataKey="mes" />
              <YAxis tickFormatter={eixoReais} width={70} />
              <Tooltip formatter={emReais} />
              <Legend />
              {categoria.series.map((s) => (
                <Bar key={s} dataKey={s} stackId="c" fill={corCategoria(s)} />
              ))}
            </BarChart>
          </ResponsiveContainer>
        </div>
        <div className="cartao grafico">
          <h3>Por pessoa</h3>
          <ResponsiveContainer width="100%" height={260}>
            <BarChart data={pessoa.linhas}>
              <CartesianGrid strokeDasharray="3 3" vertical={false} />
              <XAxis dataKey="mes" />
              <YAxis tickFormatter={eixoReais} width={70} />
              <Tooltip formatter={emReais} />
              <Legend />
              {pessoa.series.map((s) => (
                <Bar key={s} dataKey={s} stackId="p" fill={corPessoa(s)} />
              ))}
            </BarChart>
          </ResponsiveContainer>
        </div>
      </div>

      {selecionado && (
        <div className="colunas larga-direita">
          <div className="cartao grafico">
            <h3>
              Fatura de {nomeMes(selecionado.competencia)}: {reais(selecionado.total)}
            </h3>
            {selecionado.total === 0 ? (
              <p className="vazio">Nada nesta fatura.</p>
            ) : (
              <ResponsiveContainer width="100%" height={260}>
                <PieChart>
                  <Pie
                    data={selecionado.porCategoria.map((t) => ({ nome: t.nome, valor: t.valor / 100 }))}
                    dataKey="valor"
                    nameKey="nome"
                    innerRadius={60}
                    outerRadius={100}
                    paddingAngle={selecionado.porCategoria.length > 1 ? 2 : 0}
                    stroke={selecionado.porCategoria.length > 1 ? "#fff" : "none"}
                  >
                    {selecionado.porCategoria.map((t) => (
                      <Cell key={t.nome} fill={corCategoria(t.nome)} />
                    ))}
                  </Pie>
                  <Tooltip formatter={emReais} />
                  <Legend />
                </PieChart>
              </ResponsiveContainer>
            )}
          </div>
          <div className="cartao">
            <h3>Mês a mês</h3>
            <table className="sem-borda">
              <thead>
                <tr>
                  <th>Mês</th>
                  <th>Vence</th>
                  <th className="num">Fatura</th>
                  <th>Situação</th>
                </tr>
              </thead>
              <tbody>
                {detalhes.map((d, i) => {
                  const st = statusDe(d.competencia);
                  return (
                    <tr
                      key={d.competencia}
                      className={i === mesSelecionado ? "selecionada" : "clicavel"}
                      onClick={() => setMesSelecionado(i)}
                    >
                      <td>{nomeMes(d.competencia)}</td>
                      <td className="pequeno">{st?.vencimento ? dataBR(st.vencimento).slice(0, 5) : "—"}</td>
                      <td className="num">{reais(d.total)}</td>
                      <td onClick={(e) => e.stopPropagation()}>
                        {st?.paga ? (
                          <span className="positivo">
                            Paga em {dataBR(st.paga.data).slice(0, 5)}{" "}
                            <button className="link suave" onClick={() => desmarcar(d.competencia)}>
                              desfazer
                            </button>
                          </span>
                        ) : pagando?.competencia === d.competencia ? (
                          <span className="cancelar-inline">
                            <input
                              type="date"
                              value={pagando.data}
                              onChange={(e) => setPagando({ ...pagando, data: e.target.value })}
                            />
                            <button className="link" onClick={marcarPaga}>
                              OK
                            </button>
                            <button className="link suave" onClick={() => setPagando(null)}>
                              Voltar
                            </button>
                          </span>
                        ) : (
                          <>
                            {st?.atrasada && <span className="negativo">Atrasada · </span>}
                            {d.total > 0 ? (
                              <button
                                className="link"
                                onClick={() => setPagando({ competencia: d.competencia, data: hoje() })}
                              >
                                Marcar como paga
                              </button>
                            ) : (
                              <span className="pequeno">—</span>
                            )}
                          </>
                        )}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </section>
  );
}
