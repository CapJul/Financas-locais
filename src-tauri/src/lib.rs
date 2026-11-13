//! Camada fina entre a interface e o núcleo: cada comando abre o repositório,
//! chama o núcleo e devolve o erro como texto para a tela mostrar.
//!
//! Bloqueio: se houver senha, o app abre bloqueado e TODO comando de dados é
//! recusado aqui no Rust até `desbloquear` receber a senha certa. Assim, mesmo
//! alguém mexendo na interface não lê nada sem a senha.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use financas_core::modelos::*;
use financas_core::{Competencia, Repositorio};
use serde::Serialize;
use tauri::{Manager, State};

struct Estado {
    repo: Mutex<Repositorio>,
    desbloqueado: AtomicBool,
}

type Resposta<T> = Result<T, String>;

const BLOQUEADO: &str = "o app está bloqueado; digite a senha";

fn travar<'a>(estado: &'a State<'_, Estado>) -> Resposta<std::sync::MutexGuard<'a, Repositorio>> {
    estado.repo.lock().map_err(|_| "banco indisponível".to_string())
}

/// Executa `f` com o repositório, só se o app estiver desbloqueado.
fn com_repo<T>(
    estado: &State<Estado>,
    f: impl FnOnce(&mut Repositorio) -> financas_core::Resultado<T>,
) -> Resposta<T> {
    if !estado.desbloqueado.load(Ordering::SeqCst) {
        return Err(BLOQUEADO.into());
    }
    let mut repo = travar(estado)?;
    f(&mut repo).map_err(|e| e.to_string())
}

fn competencia(texto: &str) -> Resposta<Competencia> {
    texto.parse().map_err(|e: financas_core::Erro| e.to_string())
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
    let tem_senha = travar(&estado)?.tem_senha().map_err(|e| e.to_string())?;
    Ok(StatusBloqueio { tem_senha, desbloqueado: estado.desbloqueado.load(Ordering::SeqCst) })
}

// Roda fora da thread principal: o atraso de senha errada não trava a janela.
#[tauri::command(async)]
fn desbloquear(estado: State<Estado>, senha: String) -> Resposta<()> {
    let certa = travar(&estado)?.conferir_senha(&senha).map_err(|e| e.to_string())?;
    if !certa {
        // Atraso para dificultar tentativa e erro.
        std::thread::sleep(Duration::from_millis(800));
        return Err("senha incorreta".into());
    }
    estado.desbloqueado.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
fn bloquear(estado: State<Estado>) -> Resposta<()> {
    if travar(&estado)?.tem_senha().map_err(|e| e.to_string())? {
        estado.desbloqueado.store(false, Ordering::SeqCst);
    }
    Ok(())
}

/// Define, troca ou remove (`nova` vazia) a senha. Exige a atual se já houver uma.
#[tauri::command]
fn definir_senha(estado: State<Estado>, atual: Option<String>, nova: Option<String>) -> Resposta<()> {
    if !estado.desbloqueado.load(Ordering::SeqCst) {
        return Err(BLOQUEADO.into());
    }
    let nova = nova.filter(|n| !n.is_empty());
    travar(&estado)?
        .definir_senha(atual.as_deref(), nova.as_deref())
        .map_err(|e| e.to_string())
}

// ---------- Pessoas ----------

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

// ---------- Contas ----------

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

// ---------- Categorias ----------

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

// ---------- Entradas ----------

#[tauri::command]
fn criar_entrada(estado: State<Estado>, entrada: NovaEntrada) -> Resposta<Entrada> {
    com_repo(&estado, |r| r.criar_entrada(entrada))
}

#[tauri::command]
fn listar_entradas(estado: State<Estado>) -> Resposta<Vec<Entrada>> {
    com_repo(&estado, |r| r.listar_entradas())
}

#[tauri::command]
fn excluir_entrada(estado: State<Estado>, id: i64) -> Resposta<()> {
    com_repo(&estado, |r| r.excluir_entrada(id))
}

// ---------- Faturas, simulação e resumo ----------

#[tauri::command]
fn projetar_faturas(
    estado: State<Estado>,
    conta_id: i64,
    inicio: String,
    meses: u32,
) -> Resposta<Vec<FaturaMes>> {
    let inicio = competencia(&inicio)?;
    com_repo(&estado, |r| r.projetar_faturas(conta_id, inicio, meses))
}

#[tauri::command]
fn detalhar_faturas(
    estado: State<Estado>,
    conta_id: i64,
    inicio: String,
    meses: u32,
) -> Resposta<Vec<DetalheMes>> {
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Windows: %APPDATA%\io.github.financaslocais\financas.db
            // Linux:   ~/.local/share/io.github.financaslocais/financas.db
            let pasta = app.path().app_data_dir()?;
            std::fs::create_dir_all(&pasta)?;
            let repo = Repositorio::abrir(pasta.join("financas.db"))?;
            let desbloqueado = !repo.tem_senha()?;
            app.manage(Estado { repo: Mutex::new(repo), desbloqueado: AtomicBool::new(desbloqueado) });
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
            listar_transacoes,
            excluir_transacao,
            lancamentos_do_mes,
            criar_mensalidade,
            listar_mensalidades,
            cancelar_mensalidade,
            reativar_mensalidade,
            excluir_mensalidade,
            criar_entrada,
            listar_entradas,
            excluir_entrada,
            projetar_faturas,
            detalhar_faturas,
            simular_compra,
            resumo_mes,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o Finanças Locais");
}
