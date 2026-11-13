// Evita abrir uma janela de console junto com o app no Windows (só em release).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    financas_locais_lib::run()
}
