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
  /** Fatura da 1ª parcela escolhida à mão (AAAA-MM). null = pelo dia de fechamento. */
  competenciaFatura?: string | null;
}

export interface Transacao extends NovaTransacao {
  id: number;
  contaNome: string;
  categoriaNome: string | null;
  importada: boolean;
  primeiraFatura: string; // AAAA-MM
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
  /** Divisão entre pessoas; vazia = toda de pessoaId. */
  divisoes: Divisao[];
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
  divisoes: Divisao[];
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

export interface SugestaoMensalidade {
  chave: string;
  descricao: string;
  contaId: number;
  contaNome: string;
  valor: number;
  diaCobranca: number;
  meses: number;
  ultimaData: string;
  proximaCobranca: string;
  categoriaId: number | null;
}

// ---------- Importação ----------

export interface MapaColunas {
  data: number;
  descricao: number;
  valor: number;
  tipo: number | null;
}

export type TipoLinha = "gasto" | "entrada";

export interface LinhaPrevia {
  data: string;
  descricao: string;
  valor: number;
  tipo: TipoLinha;
  hash: string;
  duplicada: boolean;
  motivoIgnorar: string | null;
  categoriaId: number | null;
  parcela: string | null;
}

export interface PreviaImportacao {
  formato: "ofx" | "csv";
  cabecalho: string[];
  mapa: MapaColunas | null;
  contaCartao: boolean;
  sinalInvertido: boolean;
  competenciaSugerida: string | null;
  linhas: LinhaPrevia[];
  ilegiveis: number;
}

export interface ItemImportacao {
  data: string;
  descricao: string;
  valor: number;
  tipo: TipoLinha;
  hash: string;
  categoriaId: number | null;
}

export interface ResultadoImportacao {
  compras: number;
  entradas: number;
  duplicadas: number;
}

// ---------- Backup ----------

export interface InfoBackup {
  pasta: string | null;
  ultimo: string | null;
  cifrado: boolean;
}

// ---------- Veículos e seguros ----------

export interface NovoVeiculo {
  nome: string;
  placa: string | null;
  ano: number | null;
  pessoaId: number | null;
}

export interface Veiculo {
  id: number;
  nome: string;
  placa: string | null;
  ano: number | null;
  pessoaId: number;
  pessoaNome: string;
}

export type TipoSeguro = "auto" | "saude" | "vida" | "residencial" | "outro";

export const TIPOS_SEGURO: { id: TipoSeguro; nome: string }[] = [
  { id: "auto", nome: "Automóvel" },
  { id: "saude", nome: "Saúde" },
  { id: "vida", nome: "Vida" },
  { id: "residencial", nome: "Residencial" },
  { id: "outro", nome: "Outro" },
];

export interface PagamentoSeguro {
  contaId: number;
  valor: number;
  primeiraCobranca: string;
  categoriaId: number | null;
}

export interface NovoSeguro {
  tipo: TipoSeguro;
  descricao: string;
  seguradora: string | null;
  pessoaId: number | null;
  veiculoId: number | null;
  vigenciaFim: string | null;
  pagamento: PagamentoSeguro | null;
}

export interface Seguro {
  id: number;
  tipo: TipoSeguro;
  descricao: string;
  seguradora: string | null;
  pessoaId: number;
  pessoaNome: string;
  veiculoId: number | null;
  veiculoNome: string | null;
  vigenciaFim: string | null;
  diasParaVencer: number | null;
  mensalidadeId: number | null;
  valorMensal: number | null;
}

export interface PagamentoFatura {
  data: string;
  valor: number;
}

export interface StatusFatura {
  contaId: number;
  contaNome: string;
  competencia: string;
  valor: number;
  vencimento: string | null;
  paga: PagamentoFatura | null;
  atrasada: boolean;
}

export interface ComparacaoGeral {
  competencia: string;
  porConta: TotalNomeado[];
  atual: number;
  simulado: number;
  pagas: string[];
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
  atualizarTransacao: (id: number, transacao: NovaTransacao) =>
    invoke<Transacao>("atualizar_transacao", { id, transacao }),
  listarTransacoes: () => invoke<Transacao[]>("listar_transacoes"),
  excluirTransacao: (id: number) => invoke<void>("excluir_transacao", { id }),
  lancamentosDoMes: (competencia: string) =>
    invoke<LancamentoDoMes[]>("lancamentos_do_mes", { competencia }),

  criarMensalidade: (mensalidade: NovaMensalidade) =>
    invoke<Mensalidade>("criar_mensalidade", { mensalidade }),
  atualizarMensalidade: (id: number, mensalidade: NovaMensalidade) =>
    invoke<Mensalidade>("atualizar_mensalidade", { id, mensalidade }),
  listarMensalidades: () => invoke<Mensalidade[]>("listar_mensalidades"),
  sugerirMensalidades: () => invoke<SugestaoMensalidade[]>("sugerir_mensalidades"),
  ignorarSugestao: (chave: string) => invoke<void>("ignorar_sugestao", { chave }),
  cancelarMensalidade: (id: number, ultimoMes: string) =>
    invoke<Mensalidade>("cancelar_mensalidade", { id, ultimoMes }),
  reativarMensalidade: (id: number) => invoke<Mensalidade>("reativar_mensalidade", { id }),
  excluirMensalidade: (id: number) => invoke<void>("excluir_mensalidade", { id }),

  criarEntrada: (entrada: NovaEntrada) => invoke<Entrada>("criar_entrada", { entrada }),
  atualizarEntrada: (id: number, entrada: NovaEntrada) =>
    invoke<Entrada>("atualizar_entrada", { id, entrada }),
  listarEntradas: () => invoke<Entrada[]>("listar_entradas"),
  excluirEntrada: (id: number) => invoke<void>("excluir_entrada", { id }),

  detalharFaturas: (contaId: number, inicio: string, meses: number) =>
    invoke<DetalheMes[]>("detalhar_faturas", { contaId, inicio, meses }),
  projetarFaturas: (contaId: number, inicio: string, meses: number) =>
    invoke<FaturaMes[]>("projetar_faturas", { contaId, inicio, meses }),
  simularCompra: (simulacao: SimulacaoCompra, inicio: string, meses: number) =>
    invoke<ComparacaoMes[]>("simular_compra", { simulacao, inicio, meses }),
  resumoMes: (competencia: string) => invoke<ResumoMes>("resumo_mes", { competencia }),
  simularCompraTodas: (simulacao: SimulacaoCompra, inicio: string, meses: number) =>
    invoke<ComparacaoGeral[]>("simular_compra_todas", { simulacao, inicio, meses }),
  statusFaturas: (contaId: number, inicio: string, meses: number) =>
    invoke<StatusFatura[]>("status_faturas", { contaId, inicio, meses }),
  faturasDoMes: (competencia: string) => invoke<StatusFatura[]>("faturas_do_mes", { competencia }),
  marcarFaturaPaga: (contaId: number, competencia: string, data: string, valor: number | null) =>
    invoke<StatusFatura>("marcar_fatura_paga", { contaId, competencia, data, valor }),
  desmarcarFaturaPaga: (contaId: number, competencia: string) =>
    invoke<void>("desmarcar_fatura_paga", { contaId, competencia }),
  salvarTexto: (caminho: string, conteudo: string) => invoke<void>("salvar_texto", { caminho, conteudo }),
  imprimir: () => invoke<void>("imprimir"),

  previaImportacao: (
    caminho: string,
    contaId: number,
    mapa: MapaColunas | null,
    inverterSinal: boolean | null,
  ) => invoke<PreviaImportacao>("previa_importacao", { caminho, contaId, mapa, inverterSinal }),
  confirmarImportacao: (contaId: number, competenciaFatura: string | null, itens: ItemImportacao[]) =>
    invoke<ResultadoImportacao>("confirmar_importacao", { contaId, competenciaFatura, itens }),

  infoBackup: () => invoke<InfoBackup>("info_backup"),
  fazerBackup: (destino: string) => invoke<void>("fazer_backup", { destino }),
  restaurarBackup: (origem: string, senha: string | null) =>
    invoke<void>("restaurar_backup", { origem, senha }),
  definirPastaBackup: (pasta: string | null) => invoke<void>("definir_pasta_backup", { pasta }),
  backupAgora: () => invoke<string | null>("backup_agora"),

  listarVeiculos: () => invoke<Veiculo[]>("listar_veiculos"),
  criarVeiculo: (veiculo: NovoVeiculo) => invoke<Veiculo>("criar_veiculo", { veiculo }),
  atualizarVeiculo: (id: number, veiculo: NovoVeiculo) =>
    invoke<Veiculo>("atualizar_veiculo", { id, veiculo }),
  excluirVeiculo: (id: number) => invoke<void>("excluir_veiculo", { id }),
  listarSeguros: () => invoke<Seguro[]>("listar_seguros"),
  segurosVencendo: () => invoke<Seguro[]>("seguros_vencendo"),
  criarSeguro: (seguro: NovoSeguro) => invoke<Seguro>("criar_seguro", { seguro }),
  atualizarSeguro: (id: number, seguro: NovoSeguro) =>
    invoke<Seguro>("atualizar_seguro", { id, seguro }),
  excluirSeguro: (id: number, excluirPagamento: boolean) =>
    invoke<void>("excluir_seguro", { id, excluirPagamento }),
};
