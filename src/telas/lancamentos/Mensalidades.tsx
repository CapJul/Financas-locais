import { FormEvent, useEffect, useState } from "react";
import type { Base } from "../../App";
import { api, Mensalidade } from "../../api";
import { competenciaAtual, hoje, lerReais, mensagemErro, nomeMes, reais } from "../../util";

/** Gastos que se repetem todo mês (ex.: assinatura do Claude, Spotify, academia). */
export default function Mensalidades({ base }: { base: Base }) {
  const { contas, categorias, pessoas } = base;
  const [contaId, setContaId] = useState<number | "">("");
  const [descricao, setDescricao] = useState("");
  const [valor, setValor] = useState("");
  const [primeira, setPrimeira] = useState(hoje());
  const [categoriaId, setCategoriaId] = useState<number | "">("");
  const [pessoaId, setPessoaId] = useState<number>(1);

  const [lista, setLista] = useState<Mensalidade[]>([]);
  const [erro, setErro] = useState<string | null>(null);
  const [ok, setOk] = useState<string | null>(null);

  const carregar = () =>
    api.listarMensalidades().then(setLista).catch((e) => setErro(mensagemErro(e)));

  useEffect(() => {
    carregar();
  }, []);

  useEffect(() => {
    if (contaId === "" && contas.length > 0) {
      setContaId((contas.find((c) => c.tipo === "cartao") ?? contas[0]).id);
    }
  }, [contas, contaId]);

  async function salvar(e: FormEvent) {
    e.preventDefault();
    setErro(null);
    setOk(null);
    const v = lerReais(valor);
    if (contaId === "") return setErro("Escolha uma conta.");
    if (v === null || v <= 0) return setErro("Valor inválido. Ex.: 110,00");
    try {
      await api.criarMensalidade({
        contaId,
        descricao,
        valor: v,
        primeiraCobranca: primeira,
        categoriaId: categoriaId === "" ? null : categoriaId,
        pessoaId,
      });
      setOk(`Mensalidade "${descricao}" cadastrada.`);
      setDescricao("");
      setValor("");
      carregar();
    } catch (e) {
      setErro(mensagemErro(e));
    }
  }

  async function acao(f: () => Promise<unknown>) {
    setErro(null);
    try {
      await f();
      carregar();
    } catch (e) {
      setErro(mensagemErro(e));
    }
  }

  // Cancelamento inline: escolhe o último mês cobrado na própria linha.
  const [cancelando, setCancelando] = useState<{ id: number; mes: string } | null>(null);

  function confirmarCancelamento() {
    if (!cancelando) return;
    const { id, mes } = cancelando;
    setCancelando(null);
    acao(() => api.cancelarMensalidade(id, mes));
  }

  const ativas = lista.filter((m) => !m.fim);
  const totalAtivas = ativas.reduce((s, m) => s + m.valor, 0);

  return (
    <>
      <form className="cartao formulario" onSubmit={salvar}>
        <div className="grade">
          <label className="largo">
            Descrição
            <input
              value={descricao}
              onChange={(e) => setDescricao(e.target.value)}
              placeholder="Ex.: Assinatura do Claude"
              required
            />
          </label>
          <label>
            Valor por mês (R$)
            <input
              value={valor}
              onChange={(e) => setValor(e.target.value)}
              placeholder="0,00"
              inputMode="decimal"
              required
            />
          </label>
          <label>
            Conta
            <select value={contaId} onChange={(e) => setContaId(Number(e.target.value))}>
              {contas.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.nome} {c.tipo === "cartao" ? "(cartão)" : ""}
                </option>
              ))}
            </select>
          </label>
          <label>
            Primeira cobrança
            <input type="date" value={primeira} onChange={(e) => setPrimeira(e.target.value)} required />
          </label>
          <label>
            Categoria
            <select
              value={categoriaId}
              onChange={(e) => setCategoriaId(e.target.value ? Number(e.target.value) : "")}
            >
              <option value="">Sem categoria</option>
              {categorias.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.nome}
                </option>
              ))}
            </select>
          </label>
          <label>
            De quem é
            <select value={pessoaId} onChange={(e) => setPessoaId(Number(e.target.value))}>
              {pessoas.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.id === 1 ? "Minha" : p.nome}
                </option>
              ))}
            </select>
          </label>
        </div>
        <p className="dica">
          A cobrança se repete todo mês no mesmo dia, até você cancelar. No cartão, cobrança no
          dia do fechamento ou depois entra na fatura seguinte.
        </p>
        {erro && <div className="aviso erro">{erro}</div>}
        {ok && <div className="aviso ok">{ok}</div>}
        <button type="submit" className="primario" disabled={contas.length === 0}>
          Cadastrar mensalidade
        </button>
      </form>

      <h3>
        Mensalidades{" "}
        {ativas.length > 0 && (
          <span className="pequeno">
            · {ativas.length} {ativas.length === 1 ? "ativa soma" : "ativas somam"} {reais(totalAtivas)} por mês
          </span>
        )}
      </h3>
      {lista.length === 0 ? (
        <p className="vazio">Nenhuma mensalidade cadastrada.</p>
      ) : (
        <table>
          <thead>
            <tr>
              <th>Descrição</th>
              <th>Conta</th>
              <th>Dia</th>
              <th>Categoria</th>
              <th>De quem</th>
              <th>Período</th>
              <th className="num">Valor/mês</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {lista.map((m) => (
              <tr key={m.id} className={m.fim ? "inativa" : ""}>
                <td>{m.descricao}</td>
                <td>{m.contaNome}</td>
                <td>{m.diaCobranca}</td>
                <td>{m.categoriaNome ?? "—"}</td>
                <td>{m.pessoaNome}</td>
                <td className="pequeno">
                  desde {nomeMes(m.inicio)}
                  {m.fim ? ` até ${nomeMes(m.fim)}` : " · ativa"}
                </td>
                <td className="num">{reais(m.valor)}</td>
                <td className="acoes">
                  {cancelando?.id === m.id ? (
                    <span className="cancelar-inline">
                      último mês
                      <input
                        type="month"
                        value={cancelando.mes}
                        onChange={(e) => setCancelando({ id: m.id, mes: e.target.value })}
                      />
                      <button className="link" onClick={confirmarCancelamento}>
                        Confirmar
                      </button>
                      <button className="link" onClick={() => setCancelando(null)}>
                        Voltar
                      </button>
                    </span>
                  ) : m.fim ? (
                    <button className="link" onClick={() => acao(() => api.reativarMensalidade(m.id))}>
                      Reativar
                    </button>
                  ) : (
                    <button
                      className="link"
                      onClick={() => setCancelando({ id: m.id, mes: competenciaAtual() })}
                    >
                      Cancelar
                    </button>
                  )}
                  <button
                    className="perigo"
                    onClick={() =>
                      window.confirm(`Excluir "${m.descricao}" de todo o histórico?`) &&
                      acao(() => api.excluirMensalidade(m.id))
                    }
                  >
                    Excluir
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </>
  );
}
