//! stdio transport for the minimal ctx-sync MCP server.

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

mod protocol;
mod server;
mod tools;

use server::Server;

fn project_dir() -> Result<PathBuf, String> {
    let mut args = std::env::args_os().skip(1);
    match (args.next(), args.next(), args.next()) {
        (None, None, None) => std::env::current_dir().map_err(|error| error.to_string()),
        (Some(flag), Some(path), None) if flag == "--project" => Ok(PathBuf::from(path)),
        _ => Err("usage: ctx-sync-mcp [--project <DIR>]".into()),
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let project = project_dir().map_err(io::Error::other)?;
    let mut server = Server::new(project);
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut output = stdout.lock();
    for line in stdin.lock().lines() {
        if let Some(response) = server.handle_line(&line?) {
            writeln!(output, "{response}")?;
            output.flush()?;
        }
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
