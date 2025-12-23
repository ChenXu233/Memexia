pub mod cli;
pub mod core;
pub mod storage;

pub fn init() -> anyhow::Result<()> {
    // Initialize global state if needed (e.g. logging)
    tracing_subscriber::fmt::init();
    Ok(())
}
