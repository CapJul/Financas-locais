import { useEffect, useState } from "react";
import { api, LancamentoDoMes, ResumoMes, Seguro, StatusFatura } from "../api";
import { avisoVencimento } from "./Seguros";
import { competenciaAtual, dataBR, mensagemErro, nomeMes, reais, somarMeses } from "../util";

/** Responde "quanto eu gastei no mês", por categoria e por pessoa. */
export default function Resumo({ irPara }: { irPara: (aba: "seguros") => void }) {
  const [vencendo, setVencendo] = useState<Seguro[]>([]);
  useEffect(() => {
    api.segurosVencendo().then(setVencendo).catch(() => setVencendo([]));
  }, []);

  const [competencia, setCompetencia] = useState(competenciaAtual());
  const [resumo, setResumo] = useState<ResumoMes | null>(null);
  const [lancamentos, setLancamentos] = useState<LancamentoDoMes[]>([]);
  const [faturas, setFaturas] = useState<StatusFatura[]>([]);
  const [erro, setErro] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([api.resumoMes(competencia), api.lancamentosDoMes(competencia), api.faturasDoMes(competencia)])
      .then(([r, l, f]) => {
        setResumo(r);
        setLancamentos(l);
        setFaturas(f);
        setErro(null);
      })
      .catch((e) => setErro(mensagemErro(e)));
  }, [competencia]);

  return (
    <section>
      <header className="cabecalho">
        <h2>Resumo de {nomeMes(competencia)}</h2>
        <div className="navegacao-mes">
          <button onClick={() => setCompetencia(somarMeses(competencia, -1))}>‹</button>
          <button onClick={() => setCompetencia(competenciaAtual())}>Hoje</button>
          <button onClick={() => setCompetencia(somarMeses(competencia, 1))}>›</button>
        </div>
      </header>
      {erro && <div className="aviso erro">{erro}</div>}
      {vencendo.map((s) => (
        <div key={s.id} className="aviso alerta">
          Seguro <strong>{s.descricao}</strong> {avisoVencimento(s.diasParaVencer)}.{" "}
          <button className="link" onClick={() => irPara("seguros")}>
            Ver seguros
          </button>
        </div>
      ))}

      {resumo && (
        <>
          <div className="tres-colunas">
            <div className="destaque">
              <span>Entradas</span>
              <strong className="positivo">{reais(resumo.totalEntradas)}</strong>
            </div>
            <div className="destaque">
              <span>Gastos</span>
              <strong>{reais(resumo.total)}</strong>
            </div>
            <div className="destaque">
              <span>Saldo do mês</span>
              <strong className={resumo.saldo >= 0 ? "positivo" : "negativo"}>
                {reais(resumo.saldo)}
              </strong>
            </div>
          </div>
          <p className="dica">
            Gastos contam pelo mês da fatura; entradas, pela data em que o dinheiro entrou.
          </p>
          {faturas.some((f) => f.valor > 0) && (
            <div className="cartao">
              <h3>Faturas de {nomeMes(competencia)}</h3>
              <ul className="itens">
                {faturas
                  .filter((f) => f.valor > 0)
                  .map((f) => (
                    <li key={f.contaId}>
                      <span>
                        <strong>{f.contaNome}</strong> · {reais(f.valor)}
                        {f.vencimento && <span className="pequeno"> · vence {dataBR(f.vencimento)}</span>}
                      </span>
                      {f.paga ? (
                        <span className="positivo">Paga em {dataBR(f.paga.data)}</span>
                      ) : f.atrasada ? (
                        <span className="negativo">Atrasada</span>
                      ) : (
                        <span className="pequeno">Em aberto</span>
                      )}
                    </li>
                  ))}
              </ul>
            </div>
          )}
          <div className="tres-colunas">
            <TabelaTotais titulo="Por categoria" itens={resumo.porCategoria} total={resumo.total} />
            <TabelaTotais titulo="Por pessoa" itens={resumo.porPessoa} total={resumo.total} />
            <TabelaTotais titulo="Por tipo de gasto" itens={resumo.porOrigem} total={resumo.total} />
          </div>

          {resumo.acertos.length > 0 && (
            <>
              <h3>Acerto com outras pessoas</h3>
              <p className="dica">Acumulado até {nomeMes(competencia)}: tudo o que cada um gastou menos tudo o que já pagou.</p>
              <table>
                <thead>
                  <tr>
                    <th>Pessoa</th>
                    <th className="num">Gastou</th>
                    <th className="num">Já pagou</th>
                    <th className="num">Falta</th>
                  </tr>
                </thead>
                <tbody>
                  {resumo.acertos.map((a) => {
                    const falta = a.gasto - a.pago;
                    return (
                      <tr key={a.pessoaId}>
                        <td>{a.nome}</td>
                        <td className="num">{reais(a.gasto)}</td>
                        <td className="num">{reais(a.pago)}</td>
                        <td className={`num ${falta > 0 ? "negativo" : "positivo"}`}>
                          {falta > 0 ? reais(falta) : falta < 0 ? `adiantou ${reais(-falta)}` : "quitado"}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </>
          )}

          {resumo.entradas.length > 0 && (
            <>
              <h3>Entradas do mês</h3>
              <table>
                <thead>
                  <tr>
                    <th>Data</th>
                    <th>Descrição</th>
                    <th>Quem pagou</th>
                    <th className="num">Valor</th>
                  </tr>
                </thead>
                <tbody>
                  {resumo.entradas.map((e) => (
                    <tr key={e.id}>
                      <td>{dataBR(e.data)}</td>
                      <td>{e.descricao}</td>
                      <td>{e.pessoaNome ?? "—"}</td>
                      <td className="num positivo">+{reais(e.valor)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </>
          )}
        </>
      )}

      <h3>Compras que caem neste mês</h3>
      {lancamentos.length === 0 ? (
        <p className="vazio">Nenhum lançamento cai em {nomeMes(competencia)}.</p>
      ) : (
        <table>
          <thead>
            <tr>
              <th>Compra</th>
              <th>Descrição</th>
              <th>Conta</th>
              <th>Categoria</th>
              <th>Parcela</th>
              <th className="num">Valor</th>
            </tr>
          </thead>
          <tbody>
            {lancamentos.map((l) => (
              <tr key={`${l.transacaoId}-${l.numero}`}>
                <td>{dataBR(l.dataCompra)}</td>
                <td>{l.descricao}</td>
                <td>{l.contaNome}</td>
                <td>{l.categoriaNome ?? "—"}</td>
                <td>
                  {l.numero}/{l.numParcelas}
                </td>
                <td className="num">{reais(l.valor)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}

function TabelaTotais(props: {
  titulo: string;
  itens: { nome: string; valor: number }[];
  total: number;
}) {
  return (
    <div className="cartao">
      <h3>{props.titulo}</h3>
      {props.itens.length === 0 ? (
        <p className="vazio">Sem gastos.</p>
      ) : (
        <ul className="barras">
          {props.itens.map((i) => (
            <li key={i.nome}>
              <div className="linha">
                <span>{i.nome}</span>
                <span>{reais(i.valor)}</span>
              </div>
              <div className="trilho">
                <div
                  className="preenchido"
                  style={{ width: `${props.total ? (i.valor / props.total) * 100 : 0}%` }}
                />
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
