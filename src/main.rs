mod cpu;
mod data_memory;
mod instruction_memory;
mod register_file;

use crate::cpu::{Cpu, ExecutionResult};

use std::{ffi::OsStr, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = {
        let mut args = std::env::args();
        let _ = args.next();
        let filename = args.next().expect("no path provided");

        PathBuf::from(filename)
    };

    let elf = std::fs::read(&path)?;

    let mut cpu = match path.extension().and_then(OsStr::to_str) {
        None | Some("elf") => Cpu::from_elf(&elf, 1 << 20)?,
        Some("bin") => Cpu::from_flat_binary(&elf, 1 << 20)?,
        other => panic!("invalid file extension: {:?}", other),
    };

    match cpu.run()? {
        ExecutionResult::Exit(n) => {
            println!("Emulating finished with status: {}", n);

            Ok(())
        }
        ExecutionResult::Breakpoint => {
            println!("Emulator has encountered a breakpoint");

            Ok(())
        }
        ExecutionResult::Continue => unreachable!(),
    }
}
