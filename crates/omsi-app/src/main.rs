//! `openomsi` - the game and its launcher (the game itself is the `openomsi_game` library).

// a game, not a console program: no console window opens beside it on Windows (a start
// from a terminal still prints there, see attach_parent_console)
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

/// Laptops with two graphics chips (NVIDIA Optimus, AMD's switchable graphics): these two
/// exports ask their drivers for the graphics card rather than the processor's graphics,
/// as a game asks (build.rs exports them from the executable). Without them such a driver
/// could hand the game the weaker chip, or a card it then would not open.
#[cfg(windows)]
#[no_mangle]
#[used]
pub static NvOptimusEnablement: u32 = 1;

#[cfg(windows)]
#[no_mangle]
#[used]
pub static AmdPowerXpressRequestHighPerformance: u32 = 1;

/// [ROLLBACK alloc-1] The desktop builds allocate through mimalloc: the game makes and frees
/// a great many small blocks every frame (draw lists, script values, streamed tiles), and
/// this allocator serves them from per-thread pages with less contention between the render,
/// simulation and loader threads than the system's. A phone keeps its own allocator.
#[cfg(not(target_os = "android"))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> anyhow::Result<()> {
    openomsi_game::run()
}
