import { FormEvent, useState } from "react";
import { api } from "../api";
import { mensagemErro } from "../util";

/** Tela mostrada quando o app tem senha e está bloqueado. */
export default function Bloqueio({ aoDesbloquear }: { aoDesbloquear: () => void }) {
  const [senha, setSenha] = useState("");
  const [erro, setErro] = useState<string | null>(null);
  const [aguardando, setAguardando] = useState(false);

  async function entrar(e: FormEvent) {
    e.preventDefault();
    setAguardando(true);
    setErro(null);
    try {
      await api.desbloquear(senha);
      aoDesbloquear();
    } catch (err) {
      setErro(mensagemErro(err));
      setSenha("");
    } finally {
      setAguardando(false);
    }
  }

  return (
    <div className="tela-bloqueio">
      <form className="cartao caixa-bloqueio" onSubmit={entrar}>
        <h1>Finanças Locais</h1>
        <p className="dica">Digite a senha para abrir.</p>
        <input
          type="password"
          value={senha}
          onChange={(e) => setSenha(e.target.value)}
          placeholder="Senha"
          autoFocus
          required
        />
        {erro && <div className="aviso erro">{erro}</div>}
        <button type="submit" className="primario" disabled={aguardando}>
          {aguardando ? "Conferindo…" : "Entrar"}
        </button>
      </form>
    </div>
  );
}
