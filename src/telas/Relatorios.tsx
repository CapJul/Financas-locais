import { useEffect, useMemo, useRef, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import {
  Bar,
  CartesianGrid,
  Cell,
  ComposedChart,
  Legend,
  Line,
  Pie,
  PieChart,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type { Base } from "../App";
import {
  api,
  LancamentoDoMes,
  Mensalidade,
  ResumoMes,
  Seguro,
  StatusFatura,
  TIPOS_SEGURO,
  TotalNomeado,
} from "../api";
import { competenciaAtual, dataBR, hoje, mensagemErro, nomeMes, reais, somarMeses } from "../util";
import { avisoVencimento } from "./Seguros";

const LARGURA = 700;
const SEM_CATEGORIA = "#9aa5b1";
const eixoReais = (v: number) => `R$ ${v}`;
const emReais = (v: number) => reais(Math.round(v * 100));

interface Dados {
  meses: string[];
  resumos: ResumoMes[];
  lancamentos: Record<string, LancamentoDoMes[]>;
  faturas: { conta: string; status: StatusFatura[] }[];
  mensalidades: Mensalidade[];
  seguros: Seguro[];
}

/** Soma listas de totais por nome, do maior para o menor. */
function somarTotais(listas: TotalNomeado[][]): TotalNomeado[] {
  const mapa = new Map<string, number>();
  for (const lista of listas) for (const t of lista) mapa.set(t.nome, (mapa.get(t.nome) ?? 0) + t.valor);
  return [...mapa.entries()].map(([nome, valor]) => ({ nome, valor })).sort((a, b) => b.valor - a.valor);
}

function mesesEntre(inicio: string, fim: string): string[] {
  const lista: string[] = [];
  for (let c = inicio; c <= fim && lista.length < 36; c = somarMeses(c, 1)) lista.push(c);
  return lista;
}

/** Valor para CSV no formato que o Excel em português entende (vírgula decimal). */
const valorCsv = (centavos: number) => (centavos / 100).toFixed(2).replace(".", ",");
const textoCsv = (t: string) => `"${t.replace(/"/g, '""')}"`;

export default function Relatorios({ base }: { base: Base }) {
  const [inicio, setInicio] = useState(somarMeses(competenciaAtual(), -5));
  const [fim, setFim] = useState(competenciaAtual());
  const [comLancamentos, setComLancamentos] = useState(true);
  const [dados, setDados] = useState<Dados | null>(null);
  const [carregando, setCarregando] = useState(false);
  const [erro, setErro] = useState<string | null>(null);
  const [ok, setOk] = useState<string | null>(null);
  const relatorio = useRef<HTMLDivElement>(null);

  async function gerar() {
    if (fim < inicio) return setErro("O mês final precisa ser depois do inicial.");
    setCarregando(true);
    setErro(null);
    setOk(null);
    try {
      const meses = mesesEntre(inicio, fim);
      const cartoes = base.contas.filter((c) => c.tipo === "cartao");
      const [resumos, lancs, faturas, mensalidades, seguros] = await Promise.all([
        Promise.all(meses.map((m) => api.resumoMes(m))),
        Promise.all(meses.map((m) => api.lancamentosDoMes(m))),
        Promise.all(cartoes.map((c) => api.statusFaturas(c.id, competenciaAtual(), 6))),
        api.listarMensalidades(),
        api.listarSeguros(),
      ]);
      setDados({
        meses,
        resumos,
        lancamentos: Object.fromEntries(meses.map((m, i) => [m, lancs[i]])),
        faturas: cartoes.map((c, i) => ({ conta: c.nome, status: faturas[i] })),
        mensalidades: mensalidades.filter((m) => !m.fim),
        seguros,
      });
    } catch (e) {
      setErro(mensagemErro(e));
    } finally {
      setCarregando(false);
    }
  }

  // Gera o relatório dos últimos 6 meses ao abrir a aba.
  useEffect(() => {
    gerar();
  }, []);

  const corCategoria = useMemo(() => {
    const mapa = new Map(base.categorias.map((c) => [c.nome, c.cor]));
    return (nome: string) => mapa.get(nome) ?? SEM_CATEGORIA;
  }, [base.categorias]);

  const nomeArquivo = `relatorio-financas-${inicio}_a_${fim}`;

  async function exportarHtml() {
    if (!relatorio.current) return;
    const destino = await save({
      defaultPath: `${nomeArquivo}.html`,
      filters: [{ name: "Página HTML", extensions: ["html"] }],
    });
    if (!destino) return;
    // Leva junto o CSS do app, para o arquivo abrir igual em qualquer navegador.
    const css = Array.from(document.styleSheets)
      .map((folha) => {
        try {
          return Array.from(folha.cssRules)
            .map((r) => r.cssText)
            .join("\n");
        } catch {
          return "";
        }
      })
      .join("\n");
    const html = `<!doctype html>
<html lang="pt-BR"><head><meta charset="utf-8">
<title>Relatório Finanças Locais — ${nomeMes(inicio)} a ${nomeMes(fim)}</title>
<style>${css}</style></head>
<body class="tema-claro"><main class="relatorio-arquivo">${relatorio.current.outerHTML}</main></body></html>`;
    try {
      await api.salvarTexto(destino, html);
      setOk(`Relatório salvo em ${destino}`);
    } catch (e) {
      setErro(mensagemErro(e));
    }
  }

  async function exportarCsv() {
    if (!dados) return;
    const destino = await save({
      defaultPath: `${nomeArquivo}.csv`,
      filters: [{ name: "Planilha CSV", extensions: ["csv"] }],
    });
    if (!destino) return;
    const linhas = ["Tipo;Mês;Data;Descrição;Conta;Categoria;Parcela;Valor (R$)"];
    dados.meses.forEach((m, i) => {
      for (const l of dados.lancamentos[m] ?? []) {
        linhas.push(
          [
            "Gasto",
            m,
            dataBR(l.dataCompra),
            textoCsv(l.descricao),
            textoCsv(l.contaNome),
            textoCsv(l.categoriaNome ?? ""),
            `${l.numero}/${l.numParcelas}`,
            valorCsv(l.valor),
          ].join(";"),
        );
      }
      for (const e of dados.resumos[i].entradas) {
        linhas.push(
          ["Entrada", m, dataBR(e.data), textoCsv(e.descricao), textoCsv(e.contaNome ?? ""), "", "", valorCsv(e.valor)].join(
            ";",
          ),
        );
      }
    });
    try {
      // BOM para o Excel reconhecer acentos.
      await api.salvarTexto(destino, "﻿" + linhas.join("\r\n"));
      setOk(`Planilha salva em ${destino}`);
    } catch (e) {
      setErro(mensagemErro(e));
    }
  }

  async function exportarPdf() {
    try {
      await api.imprimir();
    } catch {
      window.print();
    }
  }

  return (
    <section>
      <div className="nao-imprimir">
        <h2>Relatórios</h2>
        <div className="cartao formulario">
          <div className="linha-form">
            <label className="inline">
              De
              <input type="month" value={inicio} onChange={(e) => setInicio(e.target.value)} />
            </label>
            <label className="inline">
              até
              <input type="month" value={fim} onChange={(e) => setFim(e.target.value)} />
            </label>
            <label className="checkbox">
              <input type="checkbox" checked={comLancamentos} onChange={(e) => setComLancamentos(e.target.checked)} />
              Incluir lista de lançamentos
            </label>
            <button className="primario" onClick={gerar} disabled={carregando}>
              {carregando ? "Gerando…" : "Gerar relatório"}
            </button>
          </div>
          {dados && (
            <div className="linha-form">
              <span className="pequeno">Exportar:</span>
              <button onClick={exportarPdf}>PDF / imprimir</button>
              <button onClick={exportarHtml}>Página HTML</button>
              <button onClick={exportarCsv}>Planilha CSV (Excel)</button>
            </div>
          )}
          <p className="dica">
            Em "PDF / imprimir", escolha a impressora <strong>Microsoft Print to PDF</strong> (ou "Salvar como PDF").
          </p>
          {erro && <div className="aviso erro">{erro}</div>}
          {ok && <div className="aviso ok">{ok}</div>}
        </div>
      </div>

      {dados && (
        <div ref={relatorio} className="relatorio">
          <Conteudo dados={dados} comLancamentos={comLancamentos} corCategoria={corCategoria} />
        </div>
      )}
    </section>
  );
}

function Conteudo({
  dados,
  comLancamentos,
  corCategoria,
}: {
  dados: Dados;
  comLancamentos: boolean;
  corCategoria: (nome: string) => string;
}) {
  const { meses, resumos } = dados;
  const entradas = resumos.reduce((s, r) => s + r.totalEntradas, 0);
  const gastos = resumos.reduce((s, r) => s + r.total, 0);
  const saldo = entradas - gastos;
  const media = Math.round(gastos / Math.max(1, meses.length));
  const maiorMes = resumos.reduce<ResumoMes | null>((m, r) => (!m || r.total > m.total ? r : m), null);
  const categorias = somarTotais(resumos.map((r) => r.porCategoria));
  const pessoas = somarTotais(resumos.map((r) => r.porPessoa));
  const origem = somarTotais(resumos.map((r) => r.porOrigem));
  const acertos = resumos[resumos.length - 1]?.acertos ?? [];
  const totalMensalidades = dados.mensalidades.reduce((s, m) => s + m.valor, 0);

  const evolucao = resumos.map((r) => ({
    mes: nomeMes(r.competencia),
    Entradas: r.totalEntradas / 100,
    Gastos: r.total / 100,
    Saldo: r.saldo / 100,
  }));

  const mesesFaturas = dados.faturas[0]?.status.map((s) => s.competencia) ?? [];
  const periodo = meses.length === 1 ? nomeMes(meses[0]) : `${nomeMes(meses[0])} a ${nomeMes(meses[meses.length - 1])}`;

  return (
    <>
      <header className="relatorio-topo">
        <h1>Relatório financeiro · {periodo}</h1>
        <p className="pequeno">Finanças Locais · gerado em {dataBR(hoje())}</p>
      </header>

      <h2>Resumo do período</h2>
      <div className="tres-colunas">
        <div className="destaque">
          <span>Entradas</span>
          <strong className="positivo">{reais(entradas)}</strong>
        </div>
        <div className="destaque">
          <span>Gastos</span>
          <strong>{reais(gastos)}</strong>
        </div>
        <div className="destaque">
          <span>Saldo</span>
          <strong className={saldo >= 0 ? "positivo" : "negativo"}>{reais(saldo)}</strong>
        </div>
      </div>
      <p>
        Média de gastos de <strong>{reais(media)}</strong> por mês
        {maiorMes && maiorMes.total > 0 && (
          <>
            ; o mês mais caro foi <strong>{nomeMes(maiorMes.competencia)}</strong> ({reais(maiorMes.total)})
          </>
        )}
        {categorias[0] && (
          <>
            . A maior categoria foi <strong>{categorias[0].nome}</strong>, com{" "}
            {Math.round((categorias[0].valor / Math.max(1, gastos)) * 100)}% dos gastos
          </>
        )}
        .
      </p>

      {meses.length > 1 && (
        <div className="bloco-relatorio">
          <h3>Entradas, gastos e saldo por mês</h3>
          <ComposedChart width={LARGURA} height={260} data={evolucao}>
            <CartesianGrid strokeDasharray="3 3" vertical={false} />
            <XAxis dataKey="mes" />
            <YAxis tickFormatter={eixoReais} width={80} />
            <Tooltip formatter={emReais} />
            <Legend />
            <Bar dataKey="Entradas" fill="#59a14f" isAnimationActive={false} />
            <Bar dataKey="Gastos" fill="#e15759" isAnimationActive={false} />
            <Line dataKey="Saldo" stroke="#4e79a7" strokeWidth={2} isAnimationActive={false} />
          </ComposedChart>
        </div>
      )}

      <div className="bloco-relatorio">
        <h3>Gastos por categoria</h3>
        {categorias.length === 0 ? (
          <p className="vazio">Sem gastos no período.</p>
        ) : (
          <div className="lado-a-lado">
            <PieChart width={260} height={240}>
              <Pie
                data={categorias.map((c) => ({ nome: c.nome, valor: c.valor / 100 }))}
                dataKey="valor"
                nameKey="nome"
                innerRadius={55}
                outerRadius={100}
                isAnimationActive={false}
                stroke={categorias.length > 1 ? "#fff" : "none"}
              >
                {categorias.map((c) => (
                  <Cell key={c.nome} fill={corCategoria(c.nome)} />
                ))}
              </Pie>
            </PieChart>
            <TabelaTotais itens={categorias} total={gastos} cor={corCategoria} />
          </div>
        )}
      </div>

      <div className="colunas bloco-relatorio">
        <div>
          <h3>Gastos por pessoa</h3>
          <TabelaTotais itens={pessoas} total={gastos} />
        </div>
        <div>
          <h3>Gastos por tipo</h3>
          <TabelaTotais itens={origem} total={gastos} />
        </div>
      </div>

      {acertos.length > 0 && (
        <div className="bloco-relatorio">
          <h3>Acerto com outras pessoas (até {nomeMes(meses[meses.length - 1])})</h3>
          <table>
            <thead>
              <tr>
                <th>Pessoa</th>
                <th className="num">Gastou</th>
                <th className="num">Pagou</th>
                <th className="num">Falta</th>
              </tr>
            </thead>
            <tbody>
              {acertos.map((a) => (
                <tr key={a.pessoaId}>
                  <td>{a.nome}</td>
                  <td className="num">{reais(a.gasto)}</td>
                  <td className="num">{reais(a.pago)}</td>
                  <td className="num">{a.gasto > a.pago ? reais(a.gasto - a.pago) : "quitado"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {dados.faturas.length > 0 && (
        <div className="bloco-relatorio">
          <h3>Próximas faturas dos cartões</h3>
          <table>
            <thead>
              <tr>
                <th>Mês</th>
                {dados.faturas.map((f) => (
                  <th key={f.conta} className="num">
                    {f.conta}
                  </th>
                ))}
                <th className="num">Total</th>
              </tr>
            </thead>
            <tbody>
              {mesesFaturas.map((m, i) => {
                const total = dados.faturas.reduce((s, f) => s + (f.status[i]?.valor ?? 0), 0);
                return (
                  <tr key={m}>
                    <td>{nomeMes(m)}</td>
                    {dados.faturas.map((f) => (
                      <td key={f.conta} className="num">
                        {reais(f.status[i]?.valor ?? 0)}
                        {f.status[i]?.paga && <span className="etiqueta">paga</span>}
                      </td>
                    ))}
                    <td className="num">
                      <strong>{reais(total)}</strong>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}

      {dados.mensalidades.length > 0 && (
        <div className="bloco-relatorio">
          <h3>Mensalidades ativas · {reais(totalMensalidades)} por mês</h3>
          <table>
            <thead>
              <tr>
                <th>Descrição</th>
                <th>Conta</th>
                <th>Dia</th>
                <th>De quem</th>
                <th className="num">Valor</th>
              </tr>
            </thead>
            <tbody>
              {dados.mensalidades.map((m) => (
                <tr key={m.id}>
                  <td>{m.descricao}</td>
                  <td>{m.contaNome}</td>
                  <td>{m.diaCobranca}</td>
                  <td>{m.pessoaNome}</td>
                  <td className="num">{reais(m.valor)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {dados.seguros.length > 0 && (
        <div className="bloco-relatorio">
          <h3>Seguros</h3>
          <table>
            <thead>
              <tr>
                <th>Seguro</th>
                <th>Tipo</th>
                <th>Vigência</th>
                <th className="num">Por mês</th>
              </tr>
            </thead>
            <tbody>
              {dados.seguros.map((s) => (
                <tr key={s.id}>
                  <td>{s.descricao}</td>
                  <td>{TIPOS_SEGURO.find((t) => t.id === s.tipo)?.nome}</td>
                  <td>
                    {s.vigenciaFim ? dataBR(s.vigenciaFim) : "—"} {avisoVencimento(s.diasParaVencer) ?? ""}
                  </td>
                  <td className="num">{s.valorMensal !== null ? reais(s.valorMensal) : "—"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {comLancamentos && (
        <div className="bloco-relatorio quebra-antes">
          <h3>Lançamentos do período</h3>
          {meses.map((m, i) => {
            const lista = dados.lancamentos[m] ?? [];
            const ents = resumos[i].entradas;
            if (lista.length === 0 && ents.length === 0) return null;
            return (
              <div key={m} className="mes-relatorio">
                <h4>
                  {nomeMes(m)} · gastos {reais(resumos[i].total)} · entradas {reais(resumos[i].totalEntradas)}
                </h4>
                <table>
                  <thead>
                    <tr>
                      <th>Data</th>
                      <th>Descrição</th>
                      <th>Conta</th>
                      <th>Categoria</th>
                      <th className="num">Valor</th>
                    </tr>
                  </thead>
                  <tbody>
                    {lista.map((l) => (
                      <tr key={`${l.transacaoId}-${l.numero}`}>
                        <td>{dataBR(l.dataCompra)}</td>
                        <td>
                          {l.descricao}
                          {l.numParcelas > 1 && ` (${l.numero}/${l.numParcelas})`}
                        </td>
                        <td>{l.contaNome}</td>
                        <td>{l.categoriaNome ?? "—"}</td>
                        <td className="num">{reais(l.valor)}</td>
                      </tr>
                    ))}
                    {ents.map((e) => (
                      <tr key={`e${e.id}`}>
                        <td>{dataBR(e.data)}</td>
                        <td>{e.descricao}</td>
                        <td>{e.contaNome ?? "—"}</td>
                        <td>Entrada</td>
                        <td className="num positivo">+{reais(e.valor)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            );
          })}
          <p className="pequeno">Mensalidades entram nos totais do mês, mas não aparecem nesta lista.</p>
        </div>
      )}
    </>
  );
}

function TabelaTotais({
  itens,
  total,
  cor,
}: {
  itens: TotalNomeado[];
  total: number;
  cor?: (nome: string) => string;
}) {
  if (itens.length === 0) return <p className="vazio">Sem gastos.</p>;
  return (
    <table>
      <tbody>
        {itens.map((t) => (
          <tr key={t.nome}>
            <td>
              {cor && <span className="bolinha" style={{ background: cor(t.nome) }} />} {t.nome}
            </td>
            <td className="num">{reais(t.valor)}</td>
            <td className="num pequeno">{Math.round((t.valor / Math.max(1, total)) * 100)}%</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
