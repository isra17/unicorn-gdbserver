extern crate unicorn_gdbserver;
extern crate unicorn;

use unicorn_gdbserver::GDBServer;
use unicorn::{CpuX86, Cpu};

fn main() {
    let emu = CpuX86::new(unicorn::Mode::MODE_32).expect("failed to instantiate emulator");
    let gdbserver = GDBServer::attach(emu.emu());
    gdbserver.listen();
}
