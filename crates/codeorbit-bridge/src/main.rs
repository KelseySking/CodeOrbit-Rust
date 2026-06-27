//! CodeOrbit Bridge — CLI hook 桥接进程

mod bridge_client;
mod environment_collector;
mod event_classifier;
mod field_normalizer;
mod payload_serializer;
mod process_ancestry;
mod program;
mod source_resolver;
mod tracked_process_resolver;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let code = program::run(args).await;
    std::process::exit(code);
}
