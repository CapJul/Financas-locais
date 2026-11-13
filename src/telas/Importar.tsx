import { useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import type { Base } from "../App";
import { api, LinhaPrevia, MapaColunas, PreviaImportacao } from "../api";
import { dataBR, mensagemErro, nomeMes, reais } from "../util";

/** Importa extrato de conta ou fatura de cartão (OFX ou CSV) com prévia. */
export default function Importar({ base }: { base: Base }) {
  const { contas, categorias } = base;
  const [contaId, setContaId] = useState<number | "">("");
  const [caminho, setCaminho] = useState<string | null>(null);
  const [previa, setPrevia] = useState<PreviaImportacao | null>(null);
  const [marcadas, setMarcadas] = useState<Set<string>>(new Set());
  const [categoriasEscolhidas, setCategoriasEscolhidas] = useState<Record<string, number | null>>({});
  const [fatura, setFatura] = useState("");
  const [carregando, setCarregando] = useState(false);
  const [erro, setErro] = useState<string | null>(null);
  const [ok, setOk] = useState<string | null>(null);

  useEffect(() => {
    if (contaId === "" && contas.length > 0) setContaId(contas[0].id);
  }, [contas, contaId]);

  const conta = contas.find((c) => c.id === contaId);

  async function carregarPrevia(arquivo: string, mapa: MapaColunas | null, inverter: boolean | null) {
    if (contaId === "") return;
    setCarregando(true);
    setErro(null);
    try {
      const p = await api.previaImportacao(arquivo, contaId, mapa, inverter);
      setPrevia(p);
      setFatura(p.competenciaSugerida ?? "");
      // Vem marcado o que não é duplicado nem tem motivo para ignorar.
      setMarcadas(new Set(p.linhas.filter((l) => !l.duplicada && !l.motivoIgnorar).map((l) => l.hash)));
      setCategoriasEscolhidas(Object.fromEntries(p.linhas.map((l) => [l.hash, l.categoriaId])));
    } catch (e) {
      setPrevia(null);
      setErro(mensagemErro(e));
    } finally {
      setCarregando(false);
    }
  }

  async function escolherArquivo() {
    setOk(null);
    const arquivo = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "Extrato ou fatura", extensions: ["ofx", "csv", "txt"] }],
    });
    if (typeof arquivo === "string") {
      setCaminho(arquivo);
      carregarPrevia(arquivo, null, null);
    }
  }

  function trocarColuna(campo: keyof MapaColunas, valor: string) {
    if (!previa?.mapa || !caminho) return;
    const novo = { ...previa.mapa, [campo]: valor === "" ? null : Number(valor) };
    carregarPrevia(caminho, novo, previa.sinalInvertido);
  }

  function alternar(hash: string) {
    const novo = new Set(marcadas);
    if (novo.has(hash)) novo.delete(hash);
    else novo.add(hash);
    setMarcadas(novo);
  }

  const selecionadas = useMemo(
    () => (previa?.linhas ?? []).filter((l) => marcadas.has(l.hash) && !l.duplicada),
    [previa, marcadas],
  );
  const somar = (tipo: LinhaPrevia["tipo"]) =>
    selecionadas.filter((l) => l.tipo === tipo).reduce((s, l) => s + l.valor, 0);
  const qtd = (tipo: LinhaPrevia["tipo"]) => selecionadas.filter((l) => l.tipo === tipo).length;

  async function importar() {
    if (!previa || contaId === "") return;
    setErro(null);
    try {
      const r = await api.confirmarImportacao(
        contaId,
        previa.contaCartao && fatura ? fatura : null,
        selecionadas.map((l) => ({
          data: l.data,
          descricao: l.descricao,
          valor: l.valor,
          tipo: l.tipo,
          hash: l.hash,
          categoriaId: l.tipo === "gasto" ? (categoriasEscolhidas[l.hash] ?? null) : null,
        })),
      );
      setOk(
        `Importado: ${r.compras} compra(s) e ${r.entradas} entrada(s)` +
          (r.duplicadas ? `; ${r.duplicadas} já existiam e foram puladas.` : "."),
      );
      setPrevia(null);
      setCaminho(null);
    } catch (e) {
      setErro(mensagemErro(e));
    }
  }

  return (
    <section>
      <h2>Importar extrato ou fatura</h2>
      <div className="cartao formulario">
        <p className="dica">
          No app ou site do banco, exporte o extrato da conta ou a fatura do cartão em <strong>OFX</strong>{" "}
          (melhor) ou <strong>CSV</strong>. Banco do Brasil e PicPay funcionam; outros bancos com colunas de
          data, descrição e valor também.
        </p>
        <div className="linha-form">
          <label className="inline">
            Importar para
            <select value={contaId} onChange={(e) => setContaId(Number(e.target.value))}>
              {contas.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.nome} {c.tipo === "cartao" ? "(cartão)" : ""}
                </option>
              ))}
            </select>
          </label>
          <button className="primario" onClick={escolherArquivo} disabled={contas.length === 0 || carregando}>
            {carregando ? "Lendo…" : "Escolher arquivo"}
          </button>
          {caminho && <span className="pequeno">{caminho.split(/[\\/]/).pop()}</span>}
        </div>
        {erro && <div className="aviso erro">{erro}</div>}
        {ok && <div className="aviso ok">{ok}</div>}
      </div>

      {previa && (
        <>
          <div className="cartao formulario">
            <p className="dica">
              {previa.formato.toUpperCase()} com {previa.linhas.length} lançamento(s)
              {previa.ilegiveis > 0 && `; ${previa.ilegiveis} linha(s) sem data ou valor foram puladas`}.
            </p>

            {previa.formato === "csv" && previa.mapa && (
              <div className="linha-form">
                {(["data", "descricao", "valor", "tipo"] as const).map((campo) => (
                  <label key={campo} className="inline">
                    {{ data: "Data", descricao: "Descrição", valor: "Valor", tipo: "Crédito/débito" }[campo]}
                    <select
                      value={previa.mapa?.[campo] ?? ""}
                      onChange={(e) => trocarColuna(campo, e.target.value)}
                    >
                      {campo === "tipo" && <option value="">Não tem</option>}
                      {previa.cabecalho.map((h, i) => (
                        <option key={i} value={i}>
                          {h || `Coluna ${i + 1}`}
                        </option>
                      ))}
                    </select>
                  </label>
                ))}
                <label className="checkbox">
                  <input
                    type="checkbox"
                    checked={previa.sinalInvertido}
                    onChange={(e) => caminho && carregarPrevia(caminho, previa.mapa, e.target.checked)}
                  />
                  Valores positivos são gastos
                </label>
              </div>
            )}

            {previa.contaCartao && (
              <label className="inline">
                Colocar os gastos na fatura de
                <input type="month" value={fatura} onChange={(e) => setFatura(e.target.value)} />
                {fatura && <span className="pequeno">({nomeMes(fatura)})</span>}
              </label>
            )}
          </div>

          <div className="cabecalho">
            <div className="botoes">
              <button
                onClick={() =>
                  setMarcadas(new Set(previa.linhas.filter((l) => !l.duplicada).map((l) => l.hash)))
                }
              >
                Marcar todas
              </button>
              <button onClick={() => setMarcadas(new Set())}>Desmarcar todas</button>
            </div>
            <span>
              {qtd("gasto")} gasto(s) {reais(somar("gasto"))} · {qtd("entrada")} entrada(s){" "}
              <span className="positivo">{reais(somar("entrada"))}</span>
            </span>
          </div>

          <table>
            <thead>
              <tr>
                <th />
                <th>Data</th>
                <th>Descrição</th>
                <th>Tipo</th>
                <th>Categoria</th>
                <th className="num">Valor</th>
              </tr>
            </thead>
            <tbody>
              {previa.linhas.map((l) => (
                <tr key={l.hash} className={l.duplicada || !marcadas.has(l.hash) ? "inativa" : ""}>
                  <td>
                    <input
                      type="checkbox"
                      checked={marcadas.has(l.hash) && !l.duplicada}
                      disabled={l.duplicada}
                      onChange={() => alternar(l.hash)}
                    />
                  </td>
                  <td>{dataBR(l.data)}</td>
                  <td>
                    {l.descricao}
                    {l.parcela && <span className="etiqueta">parcela {l.parcela}</span>}
                    {l.duplicada && <span className="etiqueta">já importada</span>}
                    {l.motivoIgnorar && !l.duplicada && <div className="pequeno">{l.motivoIgnorar}</div>}
                  </td>
                  <td>{l.tipo === "gasto" ? "Gasto" : "Entrada"}</td>
                  <td>
                    {l.tipo === "gasto" ? (
                      <select
                        value={categoriasEscolhidas[l.hash] ?? ""}
                        onChange={(e) =>
                          setCategoriasEscolhidas({
                            ...categoriasEscolhidas,
                            [l.hash]: e.target.value ? Number(e.target.value) : null,
                          })
                        }
                      >
                        <option value="">Sem categoria</option>
                        {categorias.map((c) => (
                          <option key={c.id} value={c.id}>
                            {c.nome}
                          </option>
                        ))}
                      </select>
                    ) : (
                      "—"
                    )}
                  </td>
                  <td className={`num ${l.tipo === "entrada" ? "positivo" : ""}`}>
                    {l.tipo === "entrada" ? "+" : ""}
                    {reais(l.valor)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>

          <div className="botoes rodape">
            <button className="primario" onClick={importar} disabled={selecionadas.length === 0}>
              Importar {selecionadas.length} lançamento(s)
            </button>
            {conta?.tipo === "cartao" && (
              <span className="pequeno">
                Parcelas da fatura entram só no mês escolhido; as próximas virão na próxima importação.
              </span>
            )}
          </div>
        </>
      )}
    </section>
  );
}
