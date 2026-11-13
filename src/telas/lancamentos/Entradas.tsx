import { FormEvent, useEffect, useState } from "react";
import type { Base } from "../../App";
import { api, Entrada, TIPOS_ENTRADA, TipoEntrada } from "../../api";
import { dataBR, hoje, lerReais, mensagemErro, paraCampo, reais } from "../../util";

const nomeTipo = (t: TipoEntrada) => TIPOS_ENTRADA.find((x) => x.id === t)?.nome ?? t;

/** Dinheiro que entrou: pix, salário, pagamentos e reembolsos. */
export default function Entradas({ base }: { base: Base }) {
  const { contas, pessoas } = base;
  const [descricao, setDescricao] = useState("");
  const [valor, setValor] = useState("");
  const [data, setData] = useState(hoje());
  const [tipo, setTipo] = useState<TipoEntrada>("pix");
  const [contaId, setContaId] = useState<number | "">("");
  const [pessoaId, setPessoaId] = useState<number | "">("");

  const [editando, setEditando] = useState<Entrada | null>(null);
  const [lista, setLista] = useState<Entrada[]>([]);
  const [erro, setErro] = useState<string | null>(null);
  const [ok, setOk] = useState<string | null>(null);

  const carregar = () => api.listarEntradas().then(setLista).catch((e) => setErro(mensagemErro(e)));

  useEffect(() => {
    carregar();
  }, []);

  function limpar() {
    setEditando(null);
    setDescricao("");
    setValor("");
    setData(hoje());
    setTipo("pix");
    setContaId("");
    setPessoaId("");
  }

  function editar(en: Entrada) {
    setEditando(en);
    setErro(null);
    setOk(null);
    setDescricao(en.descricao);
    setValor(paraCampo(en.valor));
    setData(en.data);
    setTipo(en.tipo);
    setContaId(en.contaId ?? "");
    setPessoaId(en.pessoaId ?? "");
    window.scrollTo({ top: 0, behavior: "smooth" });
  }

  async function salvar(e: FormEvent) {
    e.preventDefault();
    setErro(null);
    setOk(null);
    const v = lerReais(valor);
    if (v === null || v <= 0) return setErro("Valor inválido. Ex.: 50,00");
    const dados = {
      descricao,
      valor: v,
      data,
      tipo,
      contaId: contaId === "" ? null : contaId,
      pessoaId: pessoaId === "" ? null : pessoaId,
    };
    try {
      if (editando) {
        await api.atualizarEntrada(editando.id, dados);
        setOk(`Entrada "${descricao}" atualizada.`);
      } else {
        await api.criarEntrada(dados);
        setOk(`Entrada de ${reais(v)} registrada.`);
      }
      limpar();
      carregar();
    } catch (e) {
      setErro(mensagemErro(e));
    }
  }

  async function excluir(en: Entrada) {
    if (!window.confirm(`Excluir a entrada "${en.descricao}"?`)) return;
    try {
      await api.excluirEntrada(en.id);
      if (editando?.id === en.id) limpar();
      carregar();
    } catch (e) {
      setErro(mensagemErro(e));
    }
  }

  const outrasPessoas = pessoas.filter((p) => p.id !== 1);

  return (
    <>
      <form className={`cartao formulario ${editando ? "editando" : ""}`} onSubmit={salvar}>
        {editando && <div className="faixa-edicao">Editando "{editando.descricao}"</div>}
        <div className="grade">
          <label>
            Tipo
            <select value={tipo} onChange={(e) => setTipo(e.target.value as TipoEntrada)}>
              {TIPOS_ENTRADA.map((t) => (
                <option key={t.id} value={t.id}>
                  {t.nome}
                </option>
              ))}
            </select>
          </label>
          <label className="largo">
            Descrição
            <input
              value={descricao}
              onChange={(e) => setDescricao(e.target.value)}
              placeholder="Ex.: Pix do freela"
              required
            />
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
            Data
            <input type="date" value={data} onChange={(e) => setData(e.target.value)} required />
          </label>
          <label>
            Entrou na conta
            <select
              value={contaId}
              onChange={(e) => setContaId(e.target.value ? Number(e.target.value) : "")}
            >
              <option value="">Não informar</option>
              {contas.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.nome}
                </option>
              ))}
            </select>
          </label>
          {outrasPessoas.length > 0 && (
            <label>
              Quem pagou
              <select
                value={pessoaId}
                onChange={(e) => setPessoaId(e.target.value ? Number(e.target.value) : "")}
              >
                <option value="">Ninguém da lista</option>
                {outrasPessoas.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.nome}
                  </option>
                ))}
              </select>
            </label>
          )}
        </div>
        <p className="dica">
          Se a pessoa está devolvendo uma compra feita no seu cartão, escolha quem pagou: o valor
          abate do que ela deve no Resumo do mês.
        </p>
        {erro && <div className="aviso erro">{erro}</div>}
        {ok && <div className="aviso ok">{ok}</div>}
        <div className="botoes">
          <button type="submit" className="primario">
            {editando ? "Salvar alterações" : "Registrar entrada"}
          </button>
          {editando && (
            <button type="button" onClick={limpar}>
              Cancelar edição
            </button>
          )}
        </div>
      </form>

      <h3>Entradas registradas</h3>
      {lista.length === 0 ? (
        <p className="vazio">Nenhuma entrada registrada.</p>
      ) : (
        <table>
          <thead>
            <tr>
              <th>Data</th>
              <th>Descrição</th>
              <th>Tipo</th>
              <th>Conta</th>
              <th>Quem pagou</th>
              <th className="num">Valor</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {lista.map((en) => (
              <tr key={en.id}>
                <td>{dataBR(en.data)}</td>
                <td>{en.descricao}</td>
                <td>{nomeTipo(en.tipo)}</td>
                <td>{en.contaNome ?? "—"}</td>
                <td>{en.pessoaNome ?? "—"}</td>
                <td className="num positivo">+{reais(en.valor)}</td>
                <td className="acoes">
                  <button className="link" onClick={() => editar(en)}>
                    Editar
                  </button>
                  <button className="perigo" onClick={() => excluir(en)}>
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
