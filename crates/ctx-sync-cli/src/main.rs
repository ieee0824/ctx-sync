use clap::Parser;

#[derive(Parser)]
#[command(name = "ctx-sync", version, about)]
struct Cli {}

fn main() {
    let _cli = Cli::parse();
}
