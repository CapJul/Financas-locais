//! Camada fina entre a interface e o núcleo: cada comando pega o repositório,
//! chama o núcleo e devolve o erro como texto para a tela mostrar.
//!
//! Bloqueio: com senha, o arquivo do banco é cifrado (SQLCipher). O app abre sem
//! conexão nenhuma; `desbloquear` abre o arquivo com a senha. `bloquear` fecha a
//! conexão, então a chave sai da memória até a senha ser digitada de novo.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use financas_core::extrato::MapaColunas;
use financas_core::modelos::*;
use financas_core::repositorio::*;
use financas_core::{Competencia, Erro, Repositorio};
use serde::Serialize;
use tauri::{Manager, State};

struct Estado {
    caminho: PathBuf,
    /// None = bloqueado (banco cifrado ainda não aberto).
    repo: Mutex<Option<Repositorio>>,
}

type Resposta<T> = Result<T, String>;

const BLOQUEADO: &str = "o app está bloqueado; digite a senha";

fn travar<'a>(estado: &'a State<'_, Estado>) -> Resposta<MutexGuard<'a, Option<Repositorio>>> {
    estado.repo.lock().map_err(|_| "banco indisponível".to_string())
}

/// Executa `f` com o repositório, só se o app estiver desbloqueado.
fn com_repo<T>(
    estado: &State<Estado>,
    f: impl FnOnce(&mut Repositorio) -> financas_core::Resultado<T>,
) -> Resposta<T> {
    let mut guarda = travar(estado)?;
    let repo = guarda.as_mut().ok_or_else(|| BLOQUEADO.to_string())?;
    f(repo).map_err(|e| e.to_string())
}

fn competencia(texto: &str) -> Resposta<Competencia> {
    texto.parse().map_err(|e: Erro| e.to_string())
}

/// Backup automático na pasta escolhida (no máximo 1 a cada 20h). Falha não impede o uso.
fn backup_silencioso(repo: &Repositorio) {
    if let Err(e) = repo.backup_automatico(false) {
        eprintln!("backup automático falhou: {e}");
    }
}

// ---------- Bloqueio por senha ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusBloqueio {
    tem_senha: bool,
    desbloqueado: bool,
}

#[tauri::command]
fn status_bloqueio(estado: State<Estado>) -> Resposta<StatusBloqueio> {
    let guarda = travar(&estado)?;
    Ok(match guarda.as_ref() {
        Some(repo) => StatusBloqueio { tem_senha: repo.cifrado(), desbloqueado: true },
        None => StatusBloqueio { tem_senha: true, desbloqueado: false },
    })
}

// Roda fora da thread principal: abrir com a senha e o atraso de senha errada não travam a janela.
#[tauri::command(async)]
fn desbloquear(estado: State<Estado>, senha: String) -> Resposta<()> {
    let mut guarda = travar(&estado)?;
    if guarda.is_some() {
        return Ok(());
    }
    let situacao = Repositorio::situacao(&estado.caminho).map_err(|e| e.to_string())?;
    let aberto = match situacao {
        SituacaoArquivo::LegadoComSenha => Repositorio::abrir_legado(&estado.caminho, &senha),
        _ => Repositorio::abrir(&estado.caminho, Some(&senha)),
    };
    match aberto {
        Ok(repo) => {
            backup_silencioso(&repo);
            *guarda = Some(repo);
            Ok(())
        }
        Err(Erro::SenhaIncorreta) => {
            drop(guarda);
            std::thread::sleep(Duration::from_millis(800));
            Err("senha incorreta".into())
        }
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
fn bloquear(estado: State<Estado>) -> Resposta<()> {
    let mut guarda = travar(&estado)?;
    if guarda.as_ref().is_some_and(|r| r.cifrado()) {
        *guarda = None; // fecha a conexão
    }
    Ok(())
}

/// Define, troca ou remove (`nova` vazia) a senha. Exige a atual se já houver uma.
#[tauri::command(async)]
fn definir_senha(estado: State<Estado>, atual: Option<String>, nova: Option<String>) -> Resposta<()> {
    let nova = nova.filter(|n| !n.is_empty());
    com_repo(&estado, |r| r.definir_senha(atual.as_deref(), nova.as_deref()))
}

// ---------- Pessoas, contas, categorias ----------

#[tauri::command]
fn listar_pessoas(estado: State<Estado>) -> Resposta<Vec<Pessoa>> {
    com_repo(&estado, |r| r.listar_pessoas())
}

#[tauri::command]
fn criar_pessoa(estado: State<Estado>, nome: String) -> Resposta<Pessoa> {
    com_repo(&estado, |r| r.criar_pessoa(&nome))
}

#[tauri::command]
fn excluir_pessoa(estado: State<Estado>, id: i64) -> Resposta<()> {
    com_repo(&estado, |r| r.excluir_pessoa(id))
}

#[tauri::command]
fn listar_contas(estado: State<Estado>) -> Resposta<Vec<Conta>> {
    com_repo(&estado, |r| r.listar_contas())
}

#[tauri::command]
fn criar_conta(estado: State<Estado>, conta: NovaConta) -> Resposta<Conta> {
    com_repo(&estado, |r| r.criar_conta(conta))
}

#[tauri::command]
fn excluir_conta(estado: State<Estado>, id: i64) -> Resposta<()> {
    com_repo(&estado, |r| r.excluir_conta(id))
}

#[tauri::command]
fn listar_categorias(estado: State<Estado>) -> Resposta<Vec<Categoria>> {
    com_repo(&estado, |r| r.listar_categorias())
}

#[tauri::command]
fn criar_categoria(estado: State<Estado>, nome: String, cor: String) -> Resposta<Categoria> {
    com_repo(&estado, |r| r.criar_categoria(&nome, &cor))
}

#[tauri::command]
fn excluir_categoria(estado: State<Estado>, id: i64) -> Resposta<()> {
    com_repo(&estado, |r| r.excluir_categoria(id))
}

// ---------- Compras ----------

#[tauri::command]
fn criar_transacao(estado: State<Estado>, transacao: NovaTransacao) -> Resposta<Transacao> {
    com_repo(&estado, |r| r.criar_transacao(transacao))
}

#[tauri::command]
fn atualizar_transacao(estado: State<Estado>, id: i64, transacao: NovaTransacao) -> Resposta<Transacao> {
    com_repo(&estado, |r| r.atualizar_transacao(id, transacao))
}

#[tauri::command]
fn listar_transacoes(estado: State<Estado>) -> Resposta<Vec<Transacao>> {
    com_repo(&estado, |r| r.listar_transacoes())
}

#[tauri::command]
fn excluir_transacao(estado: State<Estado>, id: i64) -> Resposta<()> {
    com_repo(&estado, |r| r.excluir_transacao(id))
}

#[tauri::command]
fn lancamentos_do_mes(estado: State<Estado>, competencia: String) -> Resposta<Vec<LancamentoDoMes>> {
    let c = self::competencia(&competencia)?;
    com_repo(&estado, |r| r.lancamentos_do_mes(c))
}

// ---------- Mensalidades ----------

#[tauri::command]
fn criar_mensalidade(estado: State<Estado>, mensalidade: NovaMensalidade) -> Resposta<Mensalidade> {
    com_repo(&estado, |r| r.criar_mensalidade(mensalidade))
}

#[tauri::command]
fn atualizar_mensalidade(estado: State<Estado>, id: i64, mensalidade: NovaMensalidade) -> Resposta<Mensalidade> {
    com_repo(&estado, |r| r.atualizar_mensalidade(id, mensalidade))
}

#[tauri::command]
fn listar_mensalidades(estado: State<Estado>) -> Resposta<Vec<Mensalidade>> {
    com_repo(&estado, |r| r.listar_mensalidades())
}

#[tauri::command]
fn cancelar_mensalidade(estado: State<Estado>, id: i64, ultimo_mes: String) -> Resposta<Mensalidade> {
    let c = competencia(&ultimo_mes)?;
    com_repo(&estado, |r| r.cancelar_mensalidade(id, c))
}

#[tauri::command]
fn reativar_mensalidade(estado: State<Estado>, id: i64) -> Resposta<Mensalidade> {
    com_repo(&estado, |r| r.reativar_mensalidade(id))
}

#[tauri::command]
fn excluir_mensalidade(estado: State<Estado>, id: i64) -> Resposta<()> {
    com_repo(&estado, |r| r.excluir_mensalidade(id))
}

#[tauri::command]
fn sugerir_mensalidades(estado: State<Estado>) -> Resposta<Vec<SugestaoMensalidade>> {
    com_repo(&estado, |r| r.sugerir_mensalidades())
}

#[tauri::command]
fn ignorar_sugestao(estado: State<Estado>, chave: String) -> Resposta<()> {
    com_repo(&estado, |r| r.ignorar_sugestao(&chave))
}

// ---------- Entradas ----------

#[tauri::command]
fn criar_entrada(estado: State<Estado>, entrada: NovaEntrada) -> Resposta<Entrada> {
    com_repo(&estado, |r| r.criar_entrada(entrada))
}

#[tauri::command]
fn atualizar_entrada(estado: State<Estado>, id: i64, entrada: NovaEntrada) -> Resposta<Entrada> {
    com_repo(&estado, |r| r.atualizar_entrada(id, entrada))
}

#[tauri::command]
fn listar_entradas(estado: State<Estado>) -> Resposta<Vec<Entrada>> {
    com_repo(&estado, |r| r.listar_entradas())
}

#[tauri::command]
fn excluir_entrada(estado: State<Estado>, id: i64) -> Resposta<()> {
    com_repo(&estado, |r| r.excluir_entrada(id))
}

// ---------- Importação ----------

#[tauri::command(async)]
fn previa_importacao(
    estado: State<Estado>,
    caminho: String,
    conta_id: i64,
    mapa: Option<MapaColunas>,
    inverter_sinal: Option<bool>,
) -> Resposta<PreviaImportacao> {
    let bytes = std::fs::read(&caminho).map_err(|e| format!("não consegui ler o arquivo: {e}"))?;
    com_repo(&estado, |r| r.previa_importacao(&bytes, conta_id, mapa, inverter_sinal))
}

#[tauri::command(async)]
fn confirmar_importacao(
    estado: State<Estado>,
    conta_id: i64,
    competencia_fatura: Option<String>,
    itens: Vec<ItemImportacao>,
) -> Resposta<ResultadoImportacao> {
    let c = competencia_fatura.as_deref().map(competencia).transpose()?;
    com_repo(&estado, |r| r.confirmar_importacao(conta_id, c, itens))
}

// ---------- Faturas, simulação e resumo ----------

#[tauri::command]
fn projetar_faturas(estado: State<Estado>, conta_id: i64, inicio: String, meses: u32) -> Resposta<Vec<FaturaMes>> {
    let inicio = competencia(&inicio)?;
    com_repo(&estado, |r| r.projetar_faturas(conta_id, inicio, meses))
}

#[tauri::command]
fn detalhar_faturas(estado: State<Estado>, conta_id: i64, inicio: String, meses: u32) -> Resposta<Vec<DetalheMes>> {
    let inicio = competencia(&inicio)?;
    com_repo(&estado, |r| r.detalhar_faturas(conta_id, inicio, meses))
}

#[tauri::command]
fn simular_compra(
    estado: State<Estado>,
    simulacao: SimulacaoCompra,
    inicio: String,
    meses: u32,
) -> Resposta<Vec<ComparacaoMes>> {
    let inicio = competencia(&inicio)?;
    com_repo(&estado, |r| r.simular_compra(&simulacao, inicio, meses))
}

#[tauri::command]
fn resumo_mes(estado: State<Estado>, competencia: String) -> Resposta<ResumoMes> {
    let c = self::competencia(&competencia)?;
    com_repo(&estado, |r| r.resumo_mes(c))
}

// ---------- Backup ----------

#[tauri::command]
fn info_backup(estado: State<Estado>) -> Resposta<InfoBackup> {
    com_repo(&estado, |r| r.info_backup())
}

#[tauri::command(async)]
fn fazer_backup(estado: State<Estado>, destino: String) -> Resposta<()> {
    com_repo(&estado, |r| r.fazer_backup(&destino))
}

#[tauri::command(async)]
fn restaurar_backup(estado: State<Estado>, origem: String, senha: Option<String>) -> Resposta<()> {
    com_repo(&estado, |r| r.restaurar_backup(&origem, senha.as_deref()))
}

#[tauri::command]
fn definir_pasta_backup(estado: State<Estado>, pasta: Option<String>) -> Resposta<()> {
    com_repo(&estado, |r| r.definir_pasta_backup(pasta.as_deref()))
}

/// Faz o backup automático agora, mesmo que o último seja recente. Devolve o arquivo criado.
#[tauri::command(async)]
fn backup_agora(estado: State<Estado>) -> Resposta<Option<String>> {
    com_repo(&estado, |r| r.backup_automatico(true))
        .map(|p| p.map(|p| p.to_string_lossy().to_string()))
}

// ---------- Veículos e seguros ----------

#[tauri::command]
fn listar_veiculos(estado: State<Estado>) -> Resposta<Vec<Veiculo>> {
    com_repo(&estado, |r| r.listar_veiculos())
}

#[tauri::command]
fn criar_veiculo(estado: State<Estado>, veiculo: NovoVeiculo) -> Resposta<Veiculo> {
    com_repo(&estado, |r| r.criar_veiculo(veiculo))
}

#[tauri::command]
fn atualizar_veiculo(estado: State<Estado>, id: i64, veiculo: NovoVeiculo) -> Resposta<Veiculo> {
    com_repo(&estado, |r| r.atualizar_veiculo(id, veiculo))
}

#[tauri::command]
fn excluir_veiculo(estado: State<Estado>, id: i64) -> Resposta<()> {
    com_repo(&estado, |r| r.excluir_veiculo(id))
}

#[tauri::command]
fn listar_seguros(estado: State<Estado>) -> Resposta<Vec<Seguro>> {
    com_repo(&estado, |r| r.listar_seguros())
}

#[tauri::command]
fn seguros_vencendo(estado: State<Estado>) -> Resposta<Vec<Seguro>> {
    com_repo(&estado, |r| r.seguros_vencendo(30))
}

#[tauri::command]
fn criar_seguro(estado: State<Estado>, seguro: NovoSeguro) -> Resposta<Seguro> {
    com_repo(&estado, |r| r.criar_seguro(seguro))
}

#[tauri::command]
fn atualizar_seguro(estado: State<Estado>, id: i64, seguro: NovoSeguro) -> Resposta<Seguro> {
    com_repo(&estado, |r| r.atualizar_seguro(id, seguro))
}

#[tauri::command]
fn excluir_seguro(estado: State<Estado>, id: i64, excluir_pagamento: bool) -> Resposta<()> {
    com_repo(&estado, |r| r.excluir_seguro(id, excluir_pagamento))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Windows: %APPDATA%\io.github.financaslocais\financas.db
            // Linux:   ~/.local/share/io.github.financaslocais/financas.db
            let pasta = app.path().app_data_dir()?;
            std::fs::create_dir_all(&pasta)?;
            let caminho = pasta.join("financas.db");
            let repo = match Repositorio::situacao(&caminho)? {
                SituacaoArquivo::Novo | SituacaoArquivo::Aberto => {
                    let repo = Repositorio::abrir(&caminho, None)?;
                    backup_silencioso(&repo);
                    Some(repo)
                }
                SituacaoArquivo::Cifrado | SituacaoArquivo::LegadoComSenha => None,
            };
            app.manage(Estado { caminho, repo: Mutex::new(repo) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            status_bloqueio,
            desbloquear,
            bloquear,
            definir_senha,
            listar_pessoas,
            criar_pessoa,
            excluir_pessoa,
            listar_contas,
            criar_conta,
            excluir_conta,
            listar_categorias,
            criar_categoria,
            excluir_categoria,
            criar_transacao,
            atualizar_transacao,
            listar_transacoes,
            excluir_transacao,
            lancamentos_do_mes,
            criar_mensalidade,
            atualizar_mensalidade,
            listar_mensalidades,
            cancelar_mensalidade,
            reativar_mensalidade,
            excluir_mensalidade,
            sugerir_mensalidades,
            ignorar_sugestao,
            criar_entrada,
            atualizar_entrada,
            listar_entradas,
            excluir_entrada,
            previa_importacao,
            confirmar_importacao,
            projetar_faturas,
            detalhar_faturas,
            simular_compra,
            resumo_mes,
            info_backup,
            fazer_backup,
            restaurar_backup,
            definir_pasta_backup,
            backup_agora,
            listar_veiculos,
            criar_veiculo,
            atualizar_veiculo,
            excluir_veiculo,
            listar_seguros,
            seguros_vencendo,
            criar_seguro,
            atualizar_seguro,
            excluir_seguro,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o Finanças Locais");
}
