mod cpu;
mod data_memory;
mod instruction_memory;
mod register_file;

use crate::cpu::Cpu;

fn main() {
    let mut cpu = Cpu::new(&[], 1 << 12).unwrap();
    cpu.run().unwrap();
}
