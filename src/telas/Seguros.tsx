import { FormEvent, useEffect, useState } from "react";
import type { Base } from "../App";
import { api, Seguro, TIPOS_SEGURO, TipoSeguro, Veiculo } from "../api";
import { dataBR, hoje, lerReais, mensagemErro, reais } from "../util";

const nomeTipo = (t: TipoSeguro) => TIPOS_SEGURO.find((x) => x.id === t)?.nome ?? t;

/** Texto do aviso de vencimento da apólice. */
export function avisoVencimento(dias: number | null): string | null {
  if (dias === null) return null;
  if (dias < 0) return `venceu há ${-dias} dia(s)`;
  if (dias === 0) return "vence hoje";
  if (dias <= 30) return `vence em ${dias} dia(s)`;
  return null;
}

export default function Seguros({ base }: { base: Base }) {
  const [veiculos, setVeiculos] = useState<Veiculo[]>([]);
  const [seguros, setSeguros] = useState<Seguro[]>([]);
  const [erro, setErro] = useState<string | null>(null);

  const carregar = () =>
    Promise.all([api.listarVeiculos(), api.listarSeguros()])
      .then(([v, s]) => {
        setVeiculos(v);
        setSeguros(s);
      })
      .catch((e) => setErro(mensagemErro(e)));

  useEffect(() => {
    carregar();
  }, []);

  return (
    <section>
      <h2>Seguros e veículos</h2>
      {erro && <div className="aviso erro">{erro}</div>}
      <FormSeguros base={base} veiculos={veiculos} seguros={seguros} recarregar={carregar} setErro={setErro} />
      <FormVeiculos base={base} veiculos={veiculos} recarregar={carregar} setErro={setErro} />
    </section>
  );
}

type Comum = { base: Base; recarregar: () => Promise<void>; setErro: (e: string | null) => void };

function FormSeguros({ base, veiculos, seguros, recarregar, setErro }: Comum & { veiculos: Veiculo[]; seguros: Seguro[] }) {
  const { pessoas, contas, categorias } = base;
  const [editando, setEditando] = useState<Seguro | null>(null);
  const [tipo, setTipo] = useState<TipoSeguro>("auto");
  const [descricao, setDescricao] = useState("");
  const [seguradora, setSeguradora] = useState("");
  const [pessoaId, setPessoaId] = useState(1);
  const [veiculoId, setVeiculoId] = useState<number | "">("");
  const [vigencia, setVigencia] = useState("");
  const [comPagamento, setComPagamento] = useState(true);
  const [valor, setValor] = useState("");
  const [contaId, setContaId] = useState<number | "">("");
  const [primeira, setPrimeira] = useState(hoje());
  const [categoriaId, setCategoriaId] = useState<number | "">("");
  const [ok, setOk] = useState<string | null>(null);

  useEffect(() => {
    if (contaId === "" && contas.length > 0) setContaId(contas[0].id);
  }, [contas, contaId]);

  function limpar() {
    setEditando(null);
    setTipo("auto");
    setDescricao("");
    setSeguradora("");
    setPessoaId(1);
    setVeiculoId("");
    setVigencia("");
    setComPagamento(true);
    setValor("");
  }

  function editar(s: Seguro) {
    setEditando(s);
    setOk(null);
    setTipo(s.tipo);
    setDescricao(s.descricao);
    setSeguradora(s.seguradora ?? "");
    setPessoaId(s.pessoaId);
    setVeiculoId(s.veiculoId ?? "");
    setVigencia(s.vigenciaFim ?? "");
    setComPagamento(false);
    window.scrollTo({ top: 0, behavior: "smooth" });
  }

  async function salvar(e: FormEvent) {
    e.preventDefault();
    setErro(null);
    setOk(null);
    let pagamento = null;
    const podePagar = !editando || editando.mensalidadeId === null;
    if (comPagamento && podePagar) {
      const v = lerReais(valor);
      if (v === null || v <= 0) return setErro("Valor mensal inválido. Ex.: 250,00");
      if (contaId === "") return setErro("Escolha a conta do pagamento.");
      pagamento = {
        contaId,
        valor: v,
        primeiraCobranca: primeira,
        categoriaId: categoriaId === "" ? null : categoriaId,
      };
    }
    const dados = {
      tipo,
      descricao,
      seguradora: seguradora || null,
      pessoaId,
      veiculoId: veiculoId === "" ? null : veiculoId,
      vigenciaFim: vigencia || null,
      pagamento,
    };
    try {
      if (editando) await api.atualizarSeguro(editando.id, dados);
      else await api.criarSeguro(dados);
      setOk(editando ? "Seguro atualizado." : "Seguro cadastrado.");
      limpar();
      await recarregar();
    } catch (err) {
      setErro(mensagemErro(err));
    }
  }

  async function excluir(s: Seguro) {
    if (!window.confirm(`Excluir o seguro "${s.descricao}"?`)) return;
    const pagamento =
      s.mensalidadeId !== null &&
      window.confirm("Excluir também a mensalidade do pagamento? (Cancelar = mantém a mensalidade)");
    try {
      await api.excluirSeguro(s.id, pagamento);
      if (editando?.id === s.id) limpar();
      await recarregar();
    } catch (err) {
      setErro(mensagemErro(err));
    }
  }

  const podePagar = !editando || editando.mensalidadeId === null;

  return (
    <>
      <form className={`cartao formulario ${editando ? "editando" : ""}`} onSubmit={salvar}>
        {editando && <div className="faixa-edicao">Editando "{editando.descricao}"</div>}
        <h3>{editando ? "Seguro" : "Novo seguro"}</h3>
        <div className="grade">
          <label>
            Tipo
            <select value={tipo} onChange={(e) => setTipo(e.target.value as TipoSeguro)}>
              {TIPOS_SEGURO.map((t) => (
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
              placeholder="Ex.: Seguro do carro"
              required
            />
          </label>
          <label>
            Seguradora
            <input value={seguradora} onChange={(e) => setSeguradora(e.target.value)} />
          </label>
          <label>
            De quem é
            <select value={pessoaId} onChange={(e) => setPessoaId(Number(e.target.value))}>
              {pessoas.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.id === 1 ? "Meu" : p.nome}
                </option>
              ))}
            </select>
          </label>
          <label>
            Veículo
            <select
              value={veiculoId}
              onChange={(e) => setVeiculoId(e.target.value ? Number(e.target.value) : "")}
            >
              <option value="">Nenhum</option>
              {veiculos.map((v) => (
                <option key={v.id} value={v.id}>
                  {v.nome} {v.placa ?? ""}
                </option>
              ))}
            </select>
          </label>
          <label>
            Vigência até
            <input type="date" value={vigencia} onChange={(e) => setVigencia(e.target.value)} />
          </label>
        </div>

        {podePagar ? (
          <>
            <label className="checkbox">
              <input type="checkbox" checked={comPagamento} onChange={(e) => setComPagamento(e.target.checked)} />
              Pago todo mês (cria uma mensalidade que entra nas faturas)
            </label>
            {comPagamento && (
              <div className="grade">
                <label>
                  Valor mensal (R$)
                  <input value={valor} onChange={(e) => setValor(e.target.value)} placeholder="0,00" inputMode="decimal" />
                </label>
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
                <label>
                  Primeira cobrança
                  <input type="date" value={primeira} onChange={(e) => setPrimeira(e.target.value)} />
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
            )}
          </>
        ) : (
          <p className="dica">O pagamento deste seguro é uma mensalidade: edite valor e conta em Lançamentos → Mensalidade.</p>
        )}

        {ok && <div className="aviso ok">{ok}</div>}
        <div className="botoes">
          <button type="submit" className="primario">
            {editando ? "Salvar alterações" : "Cadastrar seguro"}
          </button>
          {editando && (
            <button type="button" onClick={limpar}>
              Cancelar edição
            </button>
          )}
        </div>
      </form>

      <h3>Seguros</h3>
      {seguros.length === 0 ? (
        <p className="vazio">Nenhum seguro cadastrado.</p>
      ) : (
        <table>
          <thead>
            <tr>
              <th>Seguro</th>
              <th>Tipo</th>
              <th>De quem</th>
              <th>Veículo</th>
              <th>Vigência</th>
              <th className="num">Por mês</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {seguros.map((s) => {
              const aviso = avisoVencimento(s.diasParaVencer);
              return (
                <tr key={s.id}>
                  <td>
                    {s.descricao}
                    {s.seguradora && <div className="pequeno">{s.seguradora}</div>}
                  </td>
                  <td>{nomeTipo(s.tipo)}</td>
                  <td>{s.pessoaNome}</td>
                  <td>{s.veiculoNome ?? "—"}</td>
                  <td>
                    {s.vigenciaFim ? dataBR(s.vigenciaFim) : "—"}
                    {aviso && <div className="negativo pequeno">{aviso}</div>}
                  </td>
                  <td className="num">{s.valorMensal !== null ? reais(s.valorMensal) : "—"}</td>
                  <td className="acoes">
                    <button className="link" onClick={() => editar(s)}>
                      Editar
                    </button>
                    <button className="perigo" onClick={() => excluir(s)}>
                      Excluir
                    </button>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
    </>
  );
}

function FormVeiculos({ base, veiculos, recarregar, setErro }: Comum & { veiculos: Veiculo[] }) {
  const [editando, setEditando] = useState<Veiculo | null>(null);
  const [nome, setNome] = useState("");
  const [placa, setPlaca] = useState("");
  const [ano, setAno] = useState("");
  const [pessoaId, setPessoaId] = useState(1);

  function limpar() {
    setEditando(null);
    setNome("");
    setPlaca("");
    setAno("");
    setPessoaId(1);
  }

  async function salvar(e: FormEvent) {
    e.preventDefault();
    setErro(null);
    const dados = { nome, placa: placa || null, ano: ano ? Number(ano) : null, pessoaId };
    try {
      if (editando) await api.atualizarVeiculo(editando.id, dados);
      else await api.criarVeiculo(dados);
      limpar();
      await recarregar();
    } catch (err) {
      setErro(mensagemErro(err));
    }
  }

  return (
    <div className="cartao">
      <h3>Veículos</h3>
      <form className="linha-form" onSubmit={salvar}>
        <input placeholder="Nome (ex.: Onix)" value={nome} onChange={(e) => setNome(e.target.value)} required />
        <input placeholder="Placa" value={placa} onChange={(e) => setPlaca(e.target.value)} />
        <input
          placeholder="Ano"
          value={ano}
          onChange={(e) => setAno(e.target.value.replace(/\D/g, "").slice(0, 4))}
          style={{ width: 80 }}
        />
        <select value={pessoaId} onChange={(e) => setPessoaId(Number(e.target.value))}>
          {base.pessoas.map((p) => (
            <option key={p.id} value={p.id}>
              {p.id === 1 ? "Meu" : p.nome}
            </option>
          ))}
        </select>
        <button type="submit" className="primario">
          {editando ? "Salvar" : "Adicionar"}
        </button>
        {editando && (
          <button type="button" onClick={limpar}>
            Cancelar
          </button>
        )}
      </form>
      <ul className="itens">
        {veiculos.map((v) => (
          <li key={v.id}>
            <span>
              <strong>{v.nome}</strong> {v.placa && `· ${v.placa}`} {v.ano && `· ${v.ano}`} · {v.pessoaNome}
            </span>
            <span className="acoes">
              <button
                className="link"
                onClick={() => {
                  setEditando(v);
                  setNome(v.nome);
                  setPlaca(v.placa ?? "");
                  setAno(v.ano ? String(v.ano) : "");
                  setPessoaId(v.pessoaId);
                }}
              >
                Editar
              </button>
              <button
                className="perigo"
                onClick={() =>
                  window.confirm(`Excluir "${v.nome}"? Os seguros dele ficam sem veículo.`) &&
                  api.excluirVeiculo(v.id).then(recarregar).catch((e) => setErro(mensagemErro(e)))
                }
              >
                Excluir
              </button>
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}
