//! Dedicated-server-only entrypoint. This fork does not ship the playable client.
#[cfg(not(target_os = "android"))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> anyhow::Result<()> {
    let server_mode = std::env::args_os().skip(1).any(|a| {
        let s = a.to_string_lossy();
        s == "--server" || s.starts_with("--server=")
    });
    if !server_mode {
        eprintln!("Dedicated server only. Usage: openomsi-server --root <OMSI2 folder> --server <server.cfg>");
        std::process::exit(2);
    }
    openomsi_game::run()
}
