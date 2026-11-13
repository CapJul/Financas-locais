import { FormEvent, useEffect, useState } from "react";
import type { Base } from "../../App";
import { api, Mensalidade, SugestaoMensalidade } from "../../api";
import { competenciaAtual, dataBR, hoje, lerReais, mensagemErro, nomeMes, paraCampo, reais } from "../../util";

/** Gastos que se repetem todo mês (ex.: assinatura do Claude, Spotify, academia). */
export default function Mensalidades({ base }: { base: Base }) {
  const { contas, categorias, pessoas } = base;
  const [contaId, setContaId] = useState<number | "">("");
  const [descricao, setDescricao] = useState("");
  const [valor, setValor] = useState("");
  const [primeira, setPrimeira] = useState(hoje());
  const [categoriaId, setCategoriaId] = useState<number | "">("");
  const [pessoaId, setPessoaId] = useState<number>(1);

  const [editando, setEditando] = useState<Mensalidade | null>(null);
  const [sugestoes, setSugestoes] = useState<SugestaoMensalidade[]>([]);
  const [lista, setLista] = useState<Mensalidade[]>([]);
  const [erro, setErro] = useState<string | null>(null);
  const [ok, setOk] = useState<string | null>(null);

  const carregar = () =>
    Promise.all([api.listarMensalidades(), api.sugerirMensalidades()])
      .then(([l, s]) => {
        setLista(l);
        setSugestoes(s);
      })
      .catch((e) => setErro(mensagemErro(e)));

  function limpar() {
    setEditando(null);
    setDescricao("");
    setValor("");
    setPrimeira(hoje());
    setCategoriaId("");
    setPessoaId(1);
  }

  function editar(m: Mensalidade) {
    setEditando(m);
    setErro(null);
    setOk(null);
    setContaId(m.contaId);
    setDescricao(m.descricao);
    setValor(paraCampo(m.valor));
    const dia = String(Math.min(m.diaCobranca, 28)).padStart(2, "0");
    setPrimeira(`${m.inicio}-${dia}`);
    setCategoriaId(m.categoriaId ?? "");
    setPessoaId(m.pessoaId);
    window.scrollTo({ top: 0, behavior: "smooth" });
  }

  /** Preenche o formulário com a sugestão para você conferir e cadastrar. */
  function usarSugestao(s: SugestaoMensalidade) {
    limpar();
    setContaId(s.contaId);
    setDescricao(s.descricao);
    setValor(paraCampo(s.valor));
    setPrimeira(s.proximaCobranca);
    setCategoriaId(s.categoriaId ?? "");
    setOk(null);
    window.scrollTo({ top: 0, behavior: "smooth" });
  }

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
    const dados = {
      contaId,
      descricao,
      valor: v,
      primeiraCobranca: primeira,
      categoriaId: categoriaId === "" ? null : categoriaId,
      pessoaId,
    };
    try {
      if (editando) {
        await api.atualizarMensalidade(editando.id, dados);
        setOk(`Mensalidade "${descricao}" atualizada.`);
      } else {
        await api.criarMensalidade(dados);
        setOk(`Mensalidade "${descricao}" cadastrada.`);
      }
      limpar();
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
      {sugestoes.length > 0 && !editando && (
        <div className="cartao sugestoes">
          <h3>Parecem mensalidades</h3>
          <p className="dica">
            Cobranças que se repetiram em 3 meses ou mais nas suas compras. Cadastre para elas
            entrarem nas próximas faturas.
          </p>
          <ul className="itens">
            {sugestoes.map((s) => (
              <li key={s.chave}>
                <span>
                  <strong>{s.descricao}</strong> · {reais(s.valor)} · {s.contaNome} · dia{" "}
                  {s.diaCobranca} · {s.meses} meses (última em {dataBR(s.ultimaData)})
                </span>
                <span className="acoes">
                  <button className="link" onClick={() => usarSugestao(s)}>
                    Cadastrar
                  </button>
                  <button
                    className="link suave"
                    onClick={() => api.ignorarSugestao(s.chave).then(carregar)}
                  >
                    Não é mensalidade
                  </button>
                </span>
              </li>
            ))}
          </ul>
        </div>
      )}

      <form className={`cartao formulario ${editando ? "editando" : ""}`} onSubmit={salvar}>
        {editando && <div className="faixa-edicao">Editando "{editando.descricao}"</div>}
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
        <div className="botoes">
          <button type="submit" className="primario" disabled={contas.length === 0}>
            {editando ? "Salvar alterações" : "Cadastrar mensalidade"}
          </button>
          {editando && (
            <button type="button" onClick={limpar}>
              Cancelar edição
            </button>
          )}
        </div>
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
                  ) : (
                    <button className="link" onClick={() => editar(m)}>
                      Editar
                    </button>
                  )}
                  {cancelando?.id === m.id ? null : m.fim ? (
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
