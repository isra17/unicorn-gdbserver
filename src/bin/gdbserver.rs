extern crate unicorn_gdbserver;
extern crate unicorn;

use unicorn_gdbserver::GDBServer;
use unicorn::{CpuX86, Cpu};

fn main() {
    let emu = CpuX86::new(unicorn::Mode::MODE_32).expect("failed to instantiate emulator");
    let gdbserver = GDBServer::new(emu.emu(), "127.0.0.1:9999")
        .expect("Failed to create GDBServer");
    let mut gdbsession = gdbserver.accept().expect("Failed to accept a client");
    gdbsession.handle_commands().unwrap();
}
