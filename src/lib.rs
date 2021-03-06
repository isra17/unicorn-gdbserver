extern crate unicorn;

use unicorn::x86_const::RegisterX86;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};

const HEXCHARS: &'static [u8] = b"0123456789abcdef";

const REGISTERS: &'static [i32] = &[
    RegisterX86::EAX as i32,
    RegisterX86::ECX as i32,
    RegisterX86::EDX as i32,
    RegisterX86::EBX as i32,
    RegisterX86::ESP as i32,
    RegisterX86::EBP as i32,
    RegisterX86::ESI as i32,
    RegisterX86::EDI as i32,
    RegisterX86::EIP as i32,
    RegisterX86::EFLAGS as i32,
    RegisterX86::CS as i32,
    RegisterX86::SS as i32,
    RegisterX86::DS as i32,
    RegisterX86::ES as i32,
    RegisterX86::FS as i32,
    RegisterX86::GS as i32,
];

pub trait ToHex {
    fn to_hex(&self) -> String;
}

impl ToHex for [u8] {
    fn to_hex(&self) -> String {
        let mut v = Vec::with_capacity(self.len() * 2);
        for &byte in self {
            v.push(HEXCHARS[(byte >> 4) as usize]);
            v.push(HEXCHARS[(byte & 0xf) as usize]);
        }

        unsafe {
            String::from_utf8_unchecked(v)
        }
    }
}

pub struct GDBSession<'a> {
    client : GDBStream,
    uc : &'a unicorn::Unicorn,
}

struct GDBStream {
    socket: TcpStream,
}

pub struct GDBServer<'a> {
    uc: &'a unicorn::Unicorn,
    listener: TcpListener,
}

enum GDBPacket {
    Interrupt,
    MessageReceived,
    MessageFailed,
    Command(Vec<u8>),
}

impl<'a> GDBSession<'a> {
    fn new(uc : &'a unicorn::Unicorn, socket : TcpStream) -> GDBSession<'a> {
        GDBSession {client: GDBStream::from_socket(socket), uc: uc}
    }

    pub fn handle_commands(&mut self) -> std::io::Result<()> {
        loop {
            match self.client.read_packet()? {
                GDBPacket::Command(packet) => {
                    let code = packet[0];
                    let response = match code {
                        b'?' => b"S05".to_vec(),
                        b'c' => b"S05".to_vec(),
                        b'D' => break,
                        b'g' => self.read_all_regs(),
                        b'H' => b"OK".to_vec(),
                        b'm' => self.handle_read_memory(&packet),
                        b'q' => self.handle_query(String::from_utf8(packet.clone().into()).unwrap()).to_vec(),
                        b's' => b"S05".to_vec(),
                        _ => {
                            println!("Unknown command: {:?}", String::from_utf8(packet.clone()));
                            vec![]
                        }
                    };

                    println!("{:?} => {:?}", String::from_utf8(packet), String::from_utf8(response.clone()));
                    self.client.write_packet(&response)?;
                },

                _ => (),
            }
        }

        Ok(())
    }

    fn handle_query(&self, packet: String) -> &[u8] {
        if packet.starts_with("qSupported:") {
            return b"PacketSize=f0";
        } else if packet == "qAttached" {
            return b"";
        } else if packet == "qC" {
            return b"";
        } else if packet.starts_with("qL:") {
            return b"qM001";
        } else if packet == "qfThreadInfo" {
            return b"m1";
        } else if packet == "qsThreadInfo" {
            return b"l";
        } else if packet == "qTStatus" {
            return b"";
        }
        return b"";
    }

    fn read_all_regs(&self) -> Vec<u8> {
        let mut buffer = String::with_capacity(REGISTERS.len()*8);
        for reg in REGISTERS {
            let value = (self.uc.reg_read(*reg).unwrap() as u32).to_be();
            buffer += &format!("{:08x}", value);
        }
        return buffer.into_bytes();
    }

    fn handle_read_memory(&self, packet: &[u8]) -> Vec<u8> {
        let cmd = String::from_utf8(packet[1..].to_vec()).expect("Cannot decode packet");
        let mut split = cmd.split(',');
        let address = u64::from_str_radix(split.next().unwrap(), 16).unwrap();
        let length = usize::from_str_radix(split.next().unwrap(), 16).unwrap();

        match self.uc.mem_read(address, length) {
            Ok(data) =>data.to_hex().into_bytes(),
            Err(_) => b"E01".to_vec(),
        }
    }
}

impl GDBStream {
    fn from_socket(socket : TcpStream) -> GDBStream {
        return GDBStream { socket }
    }

    fn read_packet(&mut self) -> std::io::Result<GDBPacket> {
        let socket = &mut self.socket;
        for b in socket.bytes() {
            let b = b?;
            if b == b'$' {
                // Begin command marker.
                break;
            } else if b == 3 {
                // Interrupt (ctrl+c).
                return Ok(GDBPacket::Interrupt);
            } else if b == b'+' {
                return Ok(GDBPacket::MessageReceived);
            } else if b == b'-' {
                return Ok(GDBPacket::MessageFailed);
            } else {
                panic!("Unexpected byte: {}", b);
            }
        }

        let mut buffer = Vec::<u8>::new();
        for b in socket.bytes() {
            let b = b?;
            if b == '#' as u8 {
                break;
            } else if b & 0x80 != 0 {
                panic!("Unexpected byte: {}", b);
            } else {
                buffer.push(b);
            }
        }

        // Ignore checksum for now.
        let _checksum = socket.read_exact(&mut [0;2])?;
        socket.write(b"+")?;

        Ok(GDBPacket::Command(buffer))
    }

    fn write_packet(&mut self, packet: &[u8]) -> std::io::Result<()>{
        let socket = &mut self.socket;
        let mut buffer = Vec::<u8>::with_capacity(packet.len());
        let mut checksum: u8 = 0;
        buffer.push('$' as u8);
        for b in packet {
            if *b == b'$' || *b == b'#' || *b == b'*' || *b == b'}' {
                buffer.push(b'}');
                buffer.push(*b ^ 0x20);
                checksum = checksum.wrapping_add(b'}');
                checksum = checksum.wrapping_add(*b ^ 0x20);
            } else {
                buffer.push(*b);
                checksum = checksum.wrapping_add(*b);
            }
        }
        buffer.push(b'#');
        buffer.push(HEXCHARS[checksum as usize >> 4]);
        buffer.push(HEXCHARS[checksum as usize & 0xf]);
        socket.write_all(&buffer)?;
        socket.flush()?;

        Ok(())
    }
}

impl<'a> GDBServer<'a> {
    pub fn new<A: ToSocketAddrs>(uc: &unicorn::Unicorn, address: A) -> std::io::Result<GDBServer> {
        Ok(GDBServer {
            uc: uc,
            listener: TcpListener::bind(address)?,
        })
    }

    pub fn accept(&self) -> std::io::Result<GDBSession> {
        let (socket, _addr) = self.listener.accept()?;
        Ok(GDBSession::new(self.uc, socket))
    }
}
