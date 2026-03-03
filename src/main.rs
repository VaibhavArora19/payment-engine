pub mod engine;
pub mod error;
pub mod io;
pub mod models;

use std::env;
use std::fs::File;
use std::process;

use crate::engine::processor::Engine;
use crate::io::reader::TransactionReader;
use crate::io::writer::write_accounts;

fn main() {
    env_logger::init();

    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: payments-engine <transactions.csv>");
        process::exit(1);
    });

    let file = File::open(&path).unwrap_or_else(|e| {
        eprintln!("error opening file {}: {}", path, e);
        process::exit(1);
    });

    let mut engine = Engine::new();
    let reader = TransactionReader::new(file);

    for result in reader {
        match result {
            Ok(raw) => {
                if let Err(e) = engine.process(raw) {
                    eprintln!("fatal error: {}", e);
                    process::exit(1);
                }
            }
            Err(e) => {
                // malformed row log and skip, don't crash
                log::warn!("skipping malformed row: {}", e);
            }
        }
    }

    if let Err(e) = write_accounts(std::io::stdout(), engine.accounts()) {
        eprintln!("error writing output: {}", e);
        process::exit(1);
    }
}
