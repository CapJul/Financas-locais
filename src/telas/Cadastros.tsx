import { FormEvent, useState } from "react";
import type { Base } from "../App";
import { api, TipoConta } from "../api";
import { mensagemErro } from "../util";

/** Contas/cartões, pessoas (perfis) e categorias. */
export default function Cadastros({ base }: { base: Base }) {
  const [erro, setErro] = useState<string | null>(null);

  // Executa uma ação, recarrega os cadastros e mostra o erro, se houver.
  async function executar(acao: () => Promise<unknown>): Promise<boolean> {
    setErro(null);
    try {
      await acao();
      await base.recarregar();
      return true;
    } catch (e) {
      setErro(mensagemErro(e));
      return false;
    }
  }

  return (
    <section>
      <h2>Cadastros</h2>
      {erro && <div className="aviso erro">{erro}</div>}
      <Contas base={base} executar={executar} />
      <div className="colunas">
        <Pessoas base={base} executar={executar} />
        <Categorias base={base} executar={executar} />
      </div>
      <Seguranca base={base} />
    </section>
  );
}

type Props = { base: Base; executar: (acao: () => Promise<unknown>) => Promise<boolean> };

function Contas({ base, executar }: Props) {
  const [nome, setNome] = useState("");
  const [tipo, setTipo] = useState<TipoConta>("cartao");
  const [fechamento, setFechamento] = useState(10);
  const [vencimento, setVencimento] = useState(17);

  async function salvar(e: FormEvent) {
    e.preventDefault();
    const cartao = tipo === "cartao";
    const ok = await executar(() =>
      api.criarConta({
        nome,
        tipo,
        diaFechamento: cartao ? fechamento : null,
        diaVencimento: cartao ? vencimento : null,
      }),
    );
    if (ok) setNome("");
  }

  return (
    <div className="cartao">
      <h3>Contas e cartões</h3>
      <form className="linha-form" onSubmit={salvar}>
        <input placeholder="Nome (ex.: Nubank)" value={nome} onChange={(e) => setNome(e.target.value)} required />
        <select value={tipo} onChange={(e) => setTipo(e.target.value as TipoConta)}>
          <option value="cartao">Cartão de crédito</option>
          <option value="corrente">Conta corrente</option>
        </select>
        {tipo === "cartao" && (
          <>
            <label className="inline">
              Fecha dia
              <input type="number" min={1} max={31} value={fechamento} onChange={(e) => setFechamento(Number(e.target.value))} />
            </label>
            <label className="inline">
              Vence dia
              <input type="number" min={1} max={31} value={vencimento} onChange={(e) => setVencimento(Number(e.target.value))} />
            </label>
          </>
        )}
        <button type="submit" className="primario">Adicionar</button>
      </form>
      <ul className="itens">
        {base.contas.map((c) => (
          <li key={c.id}>
            <span>
              <strong>{c.nome}</strong>{" "}
              {c.tipo === "cartao"
                ? `· cartão · fecha dia ${c.diaFechamento}, vence dia ${c.diaVencimento}`
                : "· conta corrente"}
            </span>
            <button
              className="perigo"
              onClick={() => window.confirm(`Excluir "${c.nome}"?`) && executar(() => api.excluirConta(c.id))}
            >
              Excluir
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}

function Pessoas({ base, executar }: Props) {
  const [nome, setNome] = useState("");

  async function salvar(e: FormEvent) {
    e.preventDefault();
    if (await executar(() => api.criarPessoa(nome))) setNome("");
  }

  return (
    <div className="cartao">
      <h3>Pessoas</h3>
      <p className="dica">Quem divide as compras com você (ex.: Mãe, Avó, Irmã).</p>
      <form className="linha-form" onSubmit={salvar}>
        <input placeholder="Nome" value={nome} onChange={(e) => setNome(e.target.value)} required />
        <button type="submit" className="primario">Adicionar</button>
      </form>
      <ul className="itens">
        {base.pessoas.map((p) => (
          <li key={p.id}>
            <span>{p.nome}</span>
            {p.id !== 1 && (
              <button
                className="perigo"
                onClick={() => window.confirm(`Excluir "${p.nome}"?`) && executar(() => api.excluirPessoa(p.id))}
              >
                Excluir
              </button>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}

function Categorias({ base, executar }: Props) {
  const [nome, setNome] = useState("");
  const [cor, setCor] = useState("#76b7b2");

  async function salvar(e: FormEvent) {
    e.preventDefault();
    if (await executar(() => api.criarCategoria(nome, cor))) setNome("");
  }

  return (
    <div className="cartao">
      <h3>Categorias</h3>
      <form className="linha-form" onSubmit={salvar}>
        <input placeholder="Nome" value={nome} onChange={(e) => setNome(e.target.value)} required />
        <input type="color" value={cor} onChange={(e) => setCor(e.target.value)} />
        <button type="submit" className="primario">Adicionar</button>
      </form>
      <ul className="itens">
        {base.categorias.map((c) => (
          <li key={c.id}>
            <span>
              <span className="bolinha" style={{ background: c.cor }} /> {c.nome}
            </span>
            <button
              className="perigo"
              onClick={() =>
                window.confirm(`Excluir "${c.nome}"? As compras dela ficam sem categoria.`) &&
                executar(() => api.excluirCategoria(c.id))
              }
            >
              Excluir
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}

/** Senha para abrir o app. */
function Seguranca({ base }: { base: Base }) {
  const [atual, setAtual] = useState("");
  const [nova, setNova] = useState("");
  const [confirmacao, setConfirmacao] = useState("");
  const [erro, setErro] = useState<string | null>(null);
  const [ok, setOk] = useState<string | null>(null);

  async function salvar(e: FormEvent, remover = false) {
    e.preventDefault();
    setErro(null);
    setOk(null);
    if (!remover && nova !== confirmacao) return setErro("A confirmação não bate com a nova senha.");
    try {
      await api.definirSenha(base.temSenha ? atual : null, remover ? null : nova);
      setOk(remover ? "Senha removida. O app abre sem pedir senha." : "Senha salva. Ela será pedida ao abrir o app.");
      setAtual("");
      setNova("");
      setConfirmacao("");
      await base.atualizarBloqueio();
    } catch (err) {
      setErro(mensagemErro(err));
    }
  }

  return (
    <div className="cartao">
      <h3>Segurança</h3>
      <p className="dica">
        {base.temSenha
          ? "O app pede senha ao abrir. Use \"Bloquear\" no menu ao sair do computador."
          : "Defina uma senha para que outras pessoas que usam este computador não abram o app."}{" "}
        Não há como recuperar a senha esquecida.
      </p>
      <form className="linha-form" onSubmit={(e) => salvar(e)}>
        {base.temSenha && (
          <input type="password" placeholder="Senha atual" value={atual} onChange={(e) => setAtual(e.target.value)} required />
        )}
        <input type="password" placeholder="Nova senha (mín. 4)" value={nova} onChange={(e) => setNova(e.target.value)} required />
        <input type="password" placeholder="Repita a nova senha" value={confirmacao} onChange={(e) => setConfirmacao(e.target.value)} required />
        <button type="submit" className="primario">
          {base.temSenha ? "Trocar senha" : "Definir senha"}
        </button>
      </form>
      {base.temSenha && (
        <button className="perigo" onClick={(e) => (atual ? salvar(e, true) : setErro("Digite a senha atual para remover."))}>
          Remover senha
        </button>
      )}
      {erro && <div className="aviso erro">{erro}</div>}
      {ok && <div className="aviso ok">{ok}</div>}
    </div>
  );
}
