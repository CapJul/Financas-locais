import { FormEvent, useEffect, useState } from "react";
import type { Base } from "../../App";
import { api, Divisao, Transacao } from "../../api";
import { dataBR, hoje, lerReais, mensagemErro, paraCampo, reais } from "../../util";

/** Compras à vista ou parceladas, de uma pessoa ou divididas. */
export default function Compras({ base }: { base: Base }) {
  const { contas, categorias, pessoas } = base;

  const [contaId, setContaId] = useState<number | "">("");
  const [descricao, setDescricao] = useState("");
  const [data, setData] = useState(hoje());
  const [valor, setValor] = useState("");
  const [parcelas, setParcelas] = useState(1);
  const [categoriaId, setCategoriaId] = useState<number | "">("");
  // "eu" | id de uma pessoa | "dividir"
  const [dono, setDono] = useState<string>("eu");
  const dividir = dono === "dividir";
  const [partes, setPartes] = useState<Record<number, string>>({});

  const [lista, setLista] = useState<Transacao[]>([]);
  const [erro, setErro] = useState<string | null>(null);
  const [ok, setOk] = useState<string | null>(null);

  const carregar = () =>
    api.listarTransacoes().then(setLista).catch((e) => setErro(mensagemErro(e)));

  useEffect(() => {
    carregar();
  }, []);

  // Seleciona a primeira conta assim que existir.
  useEffect(() => {
    if (contaId === "" && contas.length > 0) setContaId(contas[0].id);
  }, [contas, contaId]);

  const total = lerReais(valor);
  const somaPartes = Object.values(partes).reduce((s, t) => s + (lerReais(t) ?? 0), 0);
  const falta = (total ?? 0) - somaPartes;

  function dividirIgualmente() {
    if (!total || pessoas.length === 0) return;
    const base = Math.floor(total / pessoas.length);
    const sobra = total - base * pessoas.length;
    const novo: Record<number, string> = {};
    pessoas.forEach((p, i) => (novo[p.id] = paraCampo(base + (i === 0 ? sobra : 0))));
    setPartes(novo);
  }

  async function salvar(e: FormEvent) {
    e.preventDefault();
    setErro(null);
    setOk(null);
    if (contaId === "") return setErro("Escolha uma conta.");
    if (total === null || total <= 0) return setErro("Valor inválido. Ex.: 120,50");

    let divisoes: Divisao[] = [];
    if (dono !== "eu" && !dividir) {
      // Compra inteira de outra pessoa (ex.: a mãe usou meu cartão).
      divisoes = [{ pessoaId: Number(dono), valor: total }];
    }
    if (dividir) {
      for (const p of pessoas) {
        const t = (partes[p.id] ?? "").trim();
        if (!t) continue;
        const v = lerReais(t);
        if (v === null) return setErro(`Valor inválido para ${p.nome}.`);
        if (v > 0) divisoes.push({ pessoaId: p.id, valor: v });
      }
    }

    try {
      await api.criarTransacao({
        contaId,
        descricao,
        dataCompra: data,
        valorTotal: total,
        numParcelas: parcelas,
        categoriaId: categoriaId === "" ? null : categoriaId,
        divisoes,
      });
      setOk(`"${descricao}" lançado.`);
      setDescricao("");
      setValor("");
      setParcelas(1);
      setPartes({});
      setDono("eu");
      carregar();
    } catch (e) {
      setErro(mensagemErro(e));
    }
  }

  async function excluir(t: Transacao) {
    if (!window.confirm(`Excluir "${t.descricao}" e todas as suas parcelas?`)) return;
    try {
      await api.excluirTransacao(t.id);
      carregar();
    } catch (e) {
      setErro(mensagemErro(e));
    }
  }

  const nomePessoa = (id: number) => pessoas.find((p) => p.id === id)?.nome ?? `#${id}`;

  return (
    <>
      <form className="cartao formulario" onSubmit={salvar}>
        <div className="grade">
          <label>
            Conta
            <select
              value={contaId}
              onChange={(e) => setContaId(e.target.value ? Number(e.target.value) : "")}
            >
              {contas.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.nome} {c.tipo === "cartao" ? "(cartão)" : ""}
                </option>
              ))}
            </select>
          </label>
          <label className="largo">
            Descrição
            <input value={descricao} onChange={(e) => setDescricao(e.target.value)} required />
          </label>
          <label>
            Data da compra
            <input type="date" value={data} onChange={(e) => setData(e.target.value)} required />
          </label>
          <label>
            Valor total (R$)
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
        </div>

        {total !== null && parcelas > 1 && (
          <p className="dica">
            {parcelas}x de {reais(Math.floor(total / parcelas))}
            {total % parcelas ? ` (a 1ª com ${reais(total % parcelas)} a mais)` : ""}
          </p>
        )}

        <label className="campo-dono">
          De quem é a compra
          <select value={dono} onChange={(e) => setDono(e.target.value)}>
            <option value="eu">Minha</option>
            {pessoas
              .filter((p) => p.id !== 1)
              .map((p) => (
                <option key={p.id} value={p.id}>
                  Só de {p.nome}
                </option>
              ))}
            <option value="dividir">Dividir entre pessoas…</option>
          </select>
        </label>

        {dividir && (
          <div className="divisao">
            {pessoas.map((p) => (
              <label key={p.id}>
                {p.nome}
                <input
                  value={partes[p.id] ?? ""}
                  onChange={(e) => setPartes({ ...partes, [p.id]: e.target.value })}
                  placeholder="0,00"
                  inputMode="decimal"
                />
              </label>
            ))}
            <div className="acoes-divisao">
              <button type="button" onClick={dividirIgualmente}>
                Dividir igualmente
              </button>
              <span className={falta === 0 ? "certo" : "falta"}>
                {falta === 0 ? "Divisão fecha com o total" : `Falta distribuir ${reais(falta)}`}
              </span>
            </div>
          </div>
        )}

        {erro && <div className="aviso erro">{erro}</div>}
        {ok && <div className="aviso ok">{ok}</div>}
        <button type="submit" className="primario" disabled={contas.length === 0}>
          Lançar
        </button>
      </form>

      <h3>Compras lançadas</h3>
      {lista.length === 0 ? (
        <p className="vazio">Nenhuma compra lançada ainda.</p>
      ) : (
        <table>
          <thead>
            <tr>
              <th>Data</th>
              <th>Descrição</th>
              <th>Conta</th>
              <th>Categoria</th>
              <th>Parcelas</th>
              <th>De quem</th>
              <th className="num">Total</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {lista.map((t) => (
              <tr key={t.id}>
                <td>{dataBR(t.dataCompra)}</td>
                <td>{t.descricao}</td>
                <td>{t.contaNome}</td>
                <td>{t.categoriaNome ?? "—"}</td>
                <td>{t.numParcelas}x</td>
                <td className="pequeno">
                  {t.divisoes.length === 1
                    ? nomePessoa(t.divisoes[0].pessoaId)
                    : t.divisoes.map((d) => `${nomePessoa(d.pessoaId)}: ${reais(d.valor)}`).join(" · ")}
                </td>
                <td className="num">{reais(t.valorTotal)}</td>
                <td>
                  <button className="perigo" onClick={() => excluir(t)}>
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
