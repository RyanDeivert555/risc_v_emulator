mod cpu;
mod data_memory;
mod instruction_memory;
mod register_file;

use crate::cpu::{Cpu, ExecutionResult};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let elf = std::fs::read("elf")?;

    let mut cpu = Cpu::from_elf(&elf, 1 << 12)?;

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
