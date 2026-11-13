import { useState } from "react";
import type { Base } from "../App";
import Compras from "./lancamentos/Compras";
import Mensalidades from "./lancamentos/Mensalidades";
import Entradas from "./lancamentos/Entradas";

const TIPOS = [
  { id: "compra", nome: "Compra" },
  { id: "mensalidade", nome: "Mensalidade" },
  { id: "entrada", nome: "Entrada (dinheiro recebido)" },
] as const;

type Tipo = (typeof TIPOS)[number]["id"];

export default function Lancamentos({ base }: { base: Base }) {
  const [tipo, setTipo] = useState<Tipo>("compra");
  return (
    <section>
      <h2>Lançamentos</h2>
      <div className="segmentos" role="tablist">
        {TIPOS.map((t) => (
          <button
            key={t.id}
            role="tab"
            aria-selected={tipo === t.id}
            className={tipo === t.id ? "ativo" : ""}
            onClick={() => setTipo(t.id)}
          >
            {t.nome}
          </button>
        ))}
      </div>
      {tipo === "compra" && <Compras base={base} />}
      {tipo === "mensalidade" && <Mensalidades base={base} />}
      {tipo === "entrada" && <Entradas base={base} />}
    </section>
  );
}
