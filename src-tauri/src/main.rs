// EN: Prevents an additional console window on Windows in release. Do not remove.
// RU: Убирает лишнее окно консоли в релизной сборке Windows. Не удалять.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // EN: Launch with --privileged: perform one action as administrator and exit without a window.
    // RU: Запуск с --privileged: выполнить одно действие с правами администратора и выйти без окна.
    if let Some(code) = netpilot_lib::handle_privileged_args() {
        std::process::exit(code);
    }
    netpilot_lib::run()
}
