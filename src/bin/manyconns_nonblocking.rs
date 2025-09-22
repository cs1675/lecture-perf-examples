use perf_examples::sum_client::client;
use std::{
    io::{ErrorKind, Read, Write},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener},
};

fn main() {
    let mut args = std::env::args().skip(1);
    let mode = args.next().expect("want address argument");

    println!("{}", mode);

    if let Ok(addr) = mode.parse() {
        let num_conns = args
            .next()
            .expect("want num_conns argument")
            .parse()
            .expect("want number");
        client(addr, num_conns);
        return;
    }

    if let Ok(port) = mode.parse() {
        server(port);
        return;
    }
}

fn server(port: u16) {
    let srv = TcpListener::bind(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)))
        .expect("bind");
    srv.set_nonblocking(true).expect("set nonblocking");

    let mut sum = 0;
    let mut cns = Vec::new();

    enum State {
        Read,
        Write(u64),
        Errored,
    }

    loop {
        // check for a new conn
        match srv.accept() {
            Ok((cn, _)) => {
                cn.set_nonblocking(true).expect("set nonblocking");
                cns.push((cn, State::Read));
            }
            Err(e) if e.kind() != ErrorKind::WouldBlock => {
                println!("exiting on error {:?}", e);
                break;
            }
            _ => (), // EWOULDBLOCK
        }

        let mut buf = [0u8; 4];
        for (cn, state) in &mut cns {
            match state {
                State::Read => {
                    match cn.read_exact(&mut buf) {
                        Ok(()) => {
                            sum += u32::from_le_bytes(buf) as u64;
                            *state = State::Write(sum);
                        }
                        Err(e) if e.kind() != ErrorKind::WouldBlock => {
                            *state = State::Errored;
                            continue;
                        }
                        _ => (), // EWOULDBLOCK
                    }
                }
                State::Write(sum) => match cn.write_all(&sum.to_le_bytes()) {
                    Ok(()) => {
                        *state = State::Read;
                    }
                    Err(e) if e.kind() != ErrorKind::WouldBlock => {
                        println!("exiting on error {:?}", e);
                        *state = State::Errored;
                        continue;
                    }
                    _ => (), // EWOULDBLOCK
                },
                _ => (),
            }
        }

        cns.retain(|(_, state)| match state {
            State::Errored => false,
            _ => true,
        });
    }
}
