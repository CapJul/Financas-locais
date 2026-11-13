// Chamadas ao núcleo em Rust. Os nomes batem com os comandos de src-tauri/src/lib.rs.
// Todos os valores monetários são em centavos (inteiros).
import { invoke } from "@tauri-apps/api/core";

export type TipoConta = "corrente" | "cartao";

export interface Pessoa {
  id: number;
  nome: string;
}

export interface Conta {
  id: number;
  nome: string;
  tipo: TipoConta;
  diaFechamento: number | null;
  diaVencimento: number | null;
}

export interface NovaConta {
  nome: string;
  tipo: TipoConta;
  diaFechamento: number | null;
  diaVencimento: number | null;
}

export interface Categoria {
  id: number;
  nome: string;
  cor: string;
}

export interface Divisao {
  pessoaId: number;
  valor: number;
}

export interface NovaTransacao {
  contaId: number;
  descricao: string;
  dataCompra: string; // AAAA-MM-DD
  valorTotal: number;
  numParcelas: number;
  categoriaId: number | null;
  divisoes: Divisao[];
}

export interface Transacao extends NovaTransacao {
  id: number;
  contaNome: string;
  categoriaNome: string | null;
}

export interface LancamentoDoMes {
  transacaoId: number;
  descricao: string;
  contaNome: string;
  categoriaNome: string | null;
  numero: number;
  numParcelas: number;
  valor: number;
  dataCompra: string;
}

export interface FaturaMes {
  competencia: string; // AAAA-MM
  valor: number;
}

export interface ComparacaoMes {
  competencia: string;
  atual: number;
  simulado: number;
}

export interface SimulacaoCompra {
  contaId: number;
  valorTotal: number;
  numParcelas: number;
  dataCompra: string;
}

export interface TotalNomeado {
  nome: string;
  valor: number;
}

export interface NovaMensalidade {
  contaId: number;
  descricao: string;
  valor: number;
  primeiraCobranca: string; // AAAA-MM-DD
  categoriaId: number | null;
  pessoaId: number | null;
}

export interface Mensalidade {
  id: number;
  contaId: number;
  contaNome: string;
  descricao: string;
  valor: number;
  diaCobranca: number;
  categoriaId: number | null;
  categoriaNome: string | null;
  pessoaId: number;
  pessoaNome: string;
  inicio: string; // AAAA-MM
  fim: string | null; // último mês cobrado; null = ativa
}

export type TipoEntrada = "pix" | "salario" | "pagamento" | "reembolso" | "outro";

export const TIPOS_ENTRADA: { id: TipoEntrada; nome: string }[] = [
  { id: "pix", nome: "Pix recebido" },
  { id: "salario", nome: "Salário / bolsa" },
  { id: "pagamento", nome: "Pagamento recebido" },
  { id: "reembolso", nome: "Reembolso (alguém me devolveu)" },
  { id: "outro", nome: "Outro" },
];

export interface NovaEntrada {
  contaId: number | null;
  descricao: string;
  data: string; // AAAA-MM-DD
  valor: number;
  tipo: TipoEntrada;
  pessoaId: number | null;
}

export interface Entrada extends NovaEntrada {
  id: number;
  contaNome: string | null;
  pessoaNome: string | null;
}

export interface DetalheMes {
  competencia: string;
  total: number;
  porCategoria: TotalNomeado[];
  porPessoa: TotalNomeado[];
  porOrigem: TotalNomeado[]; // "À vista" | "Parcelado" | "Mensalidades"
}

export interface Acerto {
  pessoaId: number;
  nome: string;
  gasto: number;
  pago: number;
}

export interface ResumoMes {
  competencia: string;
  total: number;
  porCategoria: TotalNomeado[];
  porPessoa: TotalNomeado[];
  porOrigem: TotalNomeado[];
  totalEntradas: number;
  saldo: number;
  entradas: Entrada[];
  acertos: Acerto[];
}

export interface StatusBloqueio {
  temSenha: boolean;
  desbloqueado: boolean;
}

export const api = {
  statusBloqueio: () => invoke<StatusBloqueio>("status_bloqueio"),
  desbloquear: (senha: string) => invoke<void>("desbloquear", { senha }),
  bloquear: () => invoke<void>("bloquear"),
  definirSenha: (atual: string | null, nova: string | null) =>
    invoke<void>("definir_senha", { atual, nova }),

  listarPessoas: () => invoke<Pessoa[]>("listar_pessoas"),
  criarPessoa: (nome: string) => invoke<Pessoa>("criar_pessoa", { nome }),
  excluirPessoa: (id: number) => invoke<void>("excluir_pessoa", { id }),

  listarContas: () => invoke<Conta[]>("listar_contas"),
  criarConta: (conta: NovaConta) => invoke<Conta>("criar_conta", { conta }),
  excluirConta: (id: number) => invoke<void>("excluir_conta", { id }),

  listarCategorias: () => invoke<Categoria[]>("listar_categorias"),
  criarCategoria: (nome: string, cor: string) =>
    invoke<Categoria>("criar_categoria", { nome, cor }),
  excluirCategoria: (id: number) => invoke<void>("excluir_categoria", { id }),

  criarTransacao: (transacao: NovaTransacao) =>
    invoke<Transacao>("criar_transacao", { transacao }),
  listarTransacoes: () => invoke<Transacao[]>("listar_transacoes"),
  excluirTransacao: (id: number) => invoke<void>("excluir_transacao", { id }),
  lancamentosDoMes: (competencia: string) =>
    invoke<LancamentoDoMes[]>("lancamentos_do_mes", { competencia }),

  criarMensalidade: (mensalidade: NovaMensalidade) =>
    invoke<Mensalidade>("criar_mensalidade", { mensalidade }),
  listarMensalidades: () => invoke<Mensalidade[]>("listar_mensalidades"),
  cancelarMensalidade: (id: number, ultimoMes: string) =>
    invoke<Mensalidade>("cancelar_mensalidade", { id, ultimoMes }),
  reativarMensalidade: (id: number) => invoke<Mensalidade>("reativar_mensalidade", { id }),
  excluirMensalidade: (id: number) => invoke<void>("excluir_mensalidade", { id }),

  criarEntrada: (entrada: NovaEntrada) => invoke<Entrada>("criar_entrada", { entrada }),
  listarEntradas: () => invoke<Entrada[]>("listar_entradas"),
  excluirEntrada: (id: number) => invoke<void>("excluir_entrada", { id }),

  detalharFaturas: (contaId: number, inicio: string, meses: number) =>
    invoke<DetalheMes[]>("detalhar_faturas", { contaId, inicio, meses }),
  projetarFaturas: (contaId: number, inicio: string, meses: number) =>
    invoke<FaturaMes[]>("projetar_faturas", { contaId, inicio, meses }),
  simularCompra: (simulacao: SimulacaoCompra, inicio: string, meses: number) =>
    invoke<ComparacaoMes[]>("simular_compra", { simulacao, inicio, meses }),
  resumoMes: (competencia: string) => invoke<ResumoMes>("resumo_mes", { competencia }),
};
