//! nexus-installer: detecta o sistema, valida requisitos, mostra o plano e (com --yes) executa.
//! Nunca instala "tudo": cada provedor entra só se pedido. Autologin é opcional e reversível.
mod autologin;
#[path = "../../core/crates/nexus-core/src/glibc_compat.rs"]
mod glibc_compat;
mod detect;
mod plan;

use std::process::ExitCode;

fn usage() {
    println!(
        "nexus-installer <comando>\n\n  detect                 mostra o que foi detectado (JSON com --json)\n  check                  valida os requisitos e sai com erro se faltar algo\n  plan [--steam] [--emulation]   mostra o que seria feito (não altera nada)\n  install [--steam] [--emulation] [--autologin USUARIO] --yes   executa o plano\n  revert-autologin       desfaz o autologin configurado pelo Nexus\n  doctor                 diagnostica a instalação"
    );
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let json = args.iter().any(|a| a == "--json");
    let sys = detect::System::probe();
    match args.first().map(String::as_str) {
        Some("detect") => {
            if json {
                println!("{}", serde_json::to_string_pretty(&sys).unwrap());
            } else {
                println!("{}", sys.describe());
            }
            ExitCode::SUCCESS
        }
        Some("check") => {
            let problems = sys.requirement_problems();
            if problems.is_empty() {
                println!("Requisitos ok.");
                ExitCode::SUCCESS
            } else {
                for p in problems {
                    eprintln!("✗ {p}");
                }
                ExitCode::FAILURE
            }
        }
        Some("plan") | Some("install") => {
            let opts = plan::Options { steam: args.iter().any(|a| a == "--steam"), emulation: args.iter().any(|a| a == "--emulation"), autologin_user: args.iter().position(|a| a == "--autologin").and_then(|i| args.get(i + 1)).cloned() };
            let problems = sys.requirement_problems();
            if !problems.is_empty() {
                for p in &problems {
                    eprintln!("✗ {p}");
                }
                return ExitCode::FAILURE;
            }
            let steps = plan::build(&sys, &opts);
            for (i, s) in steps.iter().enumerate() {
                println!("{}. {}\n     $ {}", i + 1, s.title, s.display());
            }
            if args.first().map(String::as_str) == Some("install") {
                if !args.iter().any(|a| a == "--yes") {
                    eprintln!("\nNada foi executado. Rode de novo com --yes para aplicar.");
                    return ExitCode::FAILURE;
                }
                if unsafe { libc_geteuid() } != 0 {
                    eprintln!("\nExecute como root (sudo).");
                    return ExitCode::FAILURE;
                }
                for s in &steps {
                    println!("→ {}", s.title);
                    if let Err(e) = s.run() {
                        eprintln!("✗ falhou: {e}");
                        return ExitCode::FAILURE;
                    }
                }
                println!("Pronto. Escolha a sessão \"Nexus\" na tela de login.");
            }
            ExitCode::SUCCESS
        }
        Some("revert-autologin") => match autologin::revert(std::path::Path::new("/")) {
            Ok(n) => {
                println!("{n} arquivo(s) restaurado(s).");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("✗ {e}");
                ExitCode::FAILURE
            }
        },
        Some("doctor") => {
            let ok = plan::doctor(&sys);
            if ok { ExitCode::SUCCESS } else { ExitCode::FAILURE }
        }
        _ => {
            usage();
            ExitCode::FAILURE
        }
    }
}

extern "C" {
    #[link_name = "geteuid"]
    fn libc_geteuid() -> u32;
}
