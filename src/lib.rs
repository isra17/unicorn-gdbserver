extern crate unicorn;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

const HEXCHARS: &'static [u8] = b"0123456789abcdef";

pub struct GDBServer<'a> {
    uc: &'a unicorn::Unicorn,
}

impl<'a> GDBServer<'a> {
    pub fn attach(uc: &unicorn::Unicorn) -> GDBServer {
        GDBServer { uc: uc }
    }

    pub fn listen(&self) {
        println!("GDB server listening on port 9999");
        let listener = TcpListener::bind("127.0.0.1:8080").unwrap();

        match listener.accept() {
            Ok((socket, addr)) => {
                println!("new client: {:?}", addr);
                self.handle_connection(socket);
            }
            Err(e) => println!("couldn't get client: {:?}", e),
        }

        self.uc.emu_stop().expect("Failed to stop engine.");
    }

    fn handle_connection(&self, mut socket: TcpStream) -> () {
        loop {
            let packet = self.read_packet(&mut socket);
            println!("{:?}", String::from_utf8(packet.clone()));
            self.handle_packet(&mut socket, &packet)
        }
    }

    fn read_packet(&self, socket: &mut TcpStream) -> Vec<u8> {
        for b in socket.bytes() {
            let b = b.unwrap();
            if b == '$' as u8 {
                break;
            } else if b == 3 {
                println!("Interrupt requested!");
            } else if b & 0x80 != 0 {
                panic!("Unexpected byte: {}", b);
            } else {
                println!("Ignoring byte: {}", b as char);
            }
        }

        let mut buffer = Vec::<u8>::new();
        for b in socket.bytes() {
            let b = b.unwrap();
            if b == '#' as u8 {
                break;
            } else if b & 0x80 != 0 {
                panic!("Unexpected byte: {}", b);
            } else {
                buffer.push(b);
            }
        }

        let mut checksum = [0; 2];
        socket.read_exact(&mut checksum).expect("Failed to read checksum");

        socket.write(&['+' as u8]).expect("Failed to write ack");

        buffer
    }

    fn write_packet(&self, socket: &mut TcpStream, packet: &[u8]) {
        let mut buffer = Vec::<u8>::with_capacity(packet.len());
        let mut checksum: u8 = 0;
        buffer.push('$' as u8);
        for b in packet {
            if *b == '$' as u8 || *b == '#' as u8 || *b == '*' as u8 || *b == '}' as u8 {
                buffer.push('}' as u8);
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
        socket.write_all(&buffer).expect("Failed to send response");
        socket.flush().expect("Failed to flush socket");
    }

    fn handle_packet(&self, socket: &mut TcpStream, packet: &[u8]) {
        let cmd = packet[0];
        let response: Vec<u8> = match cmd {
            b'?' => b"S05".to_vec(),
            b'c' => b"S05".to_vec(),
            b'g' => self.read_all_regs(),
            b'H' => b"OK".to_vec(),
            b'm' => self.handle_read_memory(packet),
            b'q' => self.handle_query(String::from_utf8(packet.into()).unwrap()).to_vec(),
            b's' => b"S05".to_vec(),
            _ => b"".to_vec(),
        };

        println!("Response: {:?}", String::from_utf8(response.clone()));
        self.write_packet(socket, &response);
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
        let regs = std::iter::repeat("0").take(16 * 8).collect::<String>();
        return regs.into_bytes();
    }

    fn handle_read_memory(&self, packet: &[u8]) -> Vec<u8> {
        let cmd = String::from_utf8(packet[1..].to_vec()).expect("Cannot decode packet");
        let mut split = cmd.split(',');
        let _address = u64::from_str_radix(split.next().unwrap(), 16).unwrap();
        let length = u64::from_str_radix(split.next().unwrap(), 16).unwrap();
        return std::iter::repeat(b'0').take(length as usize * 2).collect();
    }
}
