#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--probe") {
        match m3vpn_lib::cli_probe() {
            Ok((ok, total)) => {
                println!("probed {total} servers, {ok} responded");
            }
            Err(error) => {
                eprintln!("probe failed: {error:#}");
                std::process::exit(1);
            }
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("--import") {
        let Some(url) = args.get(2) else {
            eprintln!("usage: m3vpn --import <subscription-url> [name]");
            std::process::exit(2);
        };
        match m3vpn_lib::cli_import(url, args.get(3).cloned()) {
            Ok(summary) => {
                println!(
                    "imported {} servers into {} (skipped {})",
                    summary.servers, summary.name, summary.skipped
                );
            }
            Err(error) => {
                eprintln!("import failed: {error:#}");
                std::process::exit(1);
            }
        }
        return;
    }
    m3vpn_lib::run()
}
