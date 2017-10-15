extern crate unicorn_gdbserver;
extern crate unicorn;

use unicorn_gdbserver::GDBServer;
use unicorn::{CpuX86, Cpu};

fn main() {
    let x86_code32: Vec<u8> = vec![0x41, 0x4a]; // INC ecx; DEC edx

    let emu = CpuX86::new(unicorn::Mode::MODE_32).expect("failed to instantiate emulator");
    emu.mem_map(0x1000, 0x4000, unicorn::PROT_ALL).unwrap();
    emu.mem_write(0x1000, &x86_code32).unwrap();
    emu.reg_write_i32(unicorn::RegisterX86::ECX, -10).unwrap();
    emu.reg_write_i32(unicorn::RegisterX86::EDX, -50).unwrap();
    emu.reg_write_i32(unicorn::RegisterX86::EIP, 0x1000).unwrap();

    let gdbserver = GDBServer::new(emu.emu(), "127.0.0.1:9999")
        .expect("Failed to create GDBServer");
    let mut gdbsession = gdbserver.accept().expect("Failed to accept a client");
    gdbsession.handle_commands().unwrap();
}
