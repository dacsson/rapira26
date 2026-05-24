use clap::Parser;
use std::fs::File;
use std::io::Read;
use vm_core::bytefile::Bytefile;
use vm_core::decoder::Decoder;
use vm_interp::interpreter::{Interpreter, InterpreterError};

use crate::verifyer::Verifier;

mod verifyer;

/// Lama VM bytecode interpreter
#[derive(Parser, Debug)]
#[command(about, long_about = None)]
struct Args {
    /// Source bytecode file
    #[arg(short, long)]
    lama_file: String,

    /// Dump parsed bytefile metadata
    #[arg(long, default_value_t = false)]
    dump_bytefile: bool,
}

const MAX_FILE_SIZE: u64 = 1024 * 1024 * 1024; // 1GB

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Check file size
    let metadata = std::fs::metadata(&args.lama_file).map_err(|err| {
        eprintln!("{}", err);
        err
    })?;
    if metadata.len() >= MAX_FILE_SIZE {
        panic!("File is too large: {} > {}", metadata.len(), MAX_FILE_SIZE);
    }

    let mut file: File = File::open(args.lama_file).map_err(|err| {
        eprintln!("{}", err);
        err
    })?;
    let mut content = Vec::new();
    file.read_to_end(&mut content).map_err(|err| {
        eprintln!("{}", err);
        err
    })?;

    let bytefile = Bytefile::parse(content.clone()).map_err(|err| {
        eprintln!("{}", err);
        err
    })?;

    if args.dump_bytefile {
        println!("{}", bytefile);
    }

    let decoder = Decoder::new(bytefile);

    let mut verifier = verifyer::Verifier::new(decoder);
    verifier.verify().map_err(|err| {
        eprintln!("{}", err);
        err
    })?;

    // Move decoder from verifier to interpreter
    let Verifier { mut decoder, .. } = verifier;

    // Reset IP to main offset
    decoder.ip = decoder.bf.main_offset as usize;

    let mut interp = Interpreter::new(decoder);
    let _ = interp.run().map_err(|err| {
        eprintln!("{}", err);
        err
    });

    Ok(())
}
