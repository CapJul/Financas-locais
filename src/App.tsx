import { useCallback, useEffect, useState } from "react";
import { api, Categoria, Conta, Pessoa, StatusBloqueio } from "./api";
import { mensagemErro } from "./util";
import Resumo from "./telas/Resumo";
import Lancamentos from "./telas/Lancamentos";
import Faturas from "./telas/Faturas";
import Simulador from "./telas/Simulador";
import Cadastros from "./telas/Cadastros";
import Bloqueio from "./telas/Bloqueio";
import Importar from "./telas/Importar";
import Seguros from "./telas/Seguros";

/** Dados de cadastro e estado que várias telas usam. */
export interface Base {
  pessoas: Pessoa[];
  contas: Conta[];
  categorias: Categoria[];
  recarregar: () => Promise<void>;
  temSenha: boolean;
  atualizarBloqueio: () => Promise<void>;
}

const ABAS = [
  { id: "resumo", nome: "Resumo do mês" },
  { id: "lancamentos", nome: "Lançamentos" },
  { id: "importar", nome: "Importar extrato" },
  { id: "faturas", nome: "Faturas" },
  { id: "simulador", nome: "Simulador" },
  { id: "seguros", nome: "Seguros e veículos" },
  { id: "cadastros", nome: "Cadastros" },
] as const;

type Aba = (typeof ABAS)[number]["id"];

export default function App() {
  const [bloqueio, setBloqueio] = useState<StatusBloqueio | null>(null);
  const [aba, setAba] = useState<Aba>("resumo");
  const [pessoas, setPessoas] = useState<Pessoa[]>([]);
  const [contas, setContas] = useState<Conta[]>([]);
  const [categorias, setCategorias] = useState<Categoria[]>([]);
  const [erro, setErro] = useState<string | null>(null);

  const atualizarBloqueio = useCallback(async () => {
    try {
      setBloqueio(await api.statusBloqueio());
    } catch (e) {
      setErro(mensagemErro(e));
    }
  }, []);

  const recarregar = useCallback(async () => {
    try {
      const [p, c, cat] = await Promise.all([
        api.listarPessoas(),
        api.listarContas(),
        api.listarCategorias(),
      ]);
      setPessoas(p);
      setContas(c);
      setCategorias(cat);
      setErro(null);
    } catch (e) {
      setErro(mensagemErro(e));
    }
  }, []);

  useEffect(() => {
    atualizarBloqueio();
  }, [atualizarBloqueio]);

  const liberado = bloqueio !== null && bloqueio.desbloqueado;

  useEffect(() => {
    if (liberado) recarregar();
  }, [liberado, recarregar]);

  async function bloquear() {
    await api.bloquear();
    setAba("resumo");
    await atualizarBloqueio();
  }

  if (bloqueio === null) return null;
  if (!bloqueio.desbloqueado) return <Bloqueio aoDesbloquear={atualizarBloqueio} />;

  const base: Base = {
    pessoas,
    contas,
    categorias,
    recarregar,
    temSenha: bloqueio.temSenha,
    atualizarBloqueio,
  };
  const semContas = contas.length === 0;

  return (
    <div className="app">
      <nav className="menu">
        <h1>Finanças Locais</h1>
        {ABAS.map((a) => (
          <button
            key={a.id}
            className={aba === a.id ? "ativa" : ""}
            onClick={() => setAba(a.id)}
          >
            {a.nome}
          </button>
        ))}
        {bloqueio.temSenha && (
          <button className="bloquear" onClick={bloquear}>
            Bloquear
          </button>
        )}
      </nav>
      <main className="conteudo">
        {erro && <div className="aviso erro">{erro}</div>}
        {semContas && aba !== "cadastros" && (
          <div className="aviso">
            Comece cadastrando uma conta ou cartão em{" "}
            <button className="link" onClick={() => setAba("cadastros")}>
              Cadastros
            </button>
            .
          </div>
        )}
        {aba === "resumo" && <Resumo irPara={(a) => setAba(a)} />}
        {aba === "lancamentos" && <Lancamentos base={base} />}
        {aba === "importar" && <Importar base={base} />}
        {aba === "seguros" && <Seguros base={base} />}
        {aba === "faturas" && <Faturas base={base} />}
        {aba === "simulador" && <Simulador base={base} />}
        {aba === "cadastros" && <Cadastros base={base} />}
      </main>
    </div>
  );
}
