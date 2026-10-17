//! Dedicated-server-only binary for the openOMSI 0.4.2 fork.
//! Accept exactly --root and --server; never enter the launcher or client UI.
#[cfg(not(target_os = "android"))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn usage() -> ! {
    eprintln!("Dedicated server only. Usage: openomsi-server --root <OMSI2 folder> --server <server.cfg>");
    std::process::exit(2)
}

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let mut root = false;
    let mut server = false;
    while let Some(argument) = args.next() {
        let flag = argument.to_string_lossy();
        match flag.as_ref() {
            "--root" => { if args.next().is_none() { usage(); } root = true; }
            "--server" => { if args.next().is_none() { usage(); } server = true; }
            s if s.starts_with("--root=") && s.len() > 7 => root = true,
            s if s.starts_with("--server=") && s.len() > 9 => server = true,
            _ => usage(),
        }
    }
    if !root || !server { usage(); }
    openomsi_game::run()
}
