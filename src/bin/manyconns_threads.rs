use perf_examples::sum_client::client;
use std::{
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener},
    sync::{atomic::AtomicU64, Arc},
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

    let sum = Arc::new(AtomicU64::new(0));
    for cn in srv.incoming() {
        let mut cn = cn.expect("accept new connection");
        //let peer = cn.peer_addr().expect("cn peer addr");
        let sum = Arc::clone(&sum);
        //println!("connected to {:?}", peer);

        std::thread::spawn(move || {
            let mut buf = [0u8; 4];
            loop {
                match cn.read_exact(&mut buf) {
                    Ok(_) => (),
                    Err(e) => {
                        println!("exiting on error {:?}", e);
                        break;
                    }
                }

                let curr = sum.fetch_add(
                    u32::from_le_bytes(buf) as u64,
                    std::sync::atomic::Ordering::AcqRel,
                );
                cn.write_all(&curr.to_le_bytes()).expect("write");
            }

            // 2: the original one and the clone in this thread
            if Arc::strong_count(&sum) == 2 {
                println!("exiting");
                std::process::exit(0);
            }
        });
    }
}
