use std::{
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream},
};

fn main() {
    let mode = std::env::args()
        .skip(1)
        .next()
        .expect("want address argument");

    println!("{}", mode);

    if let Ok(addr) = mode.parse() {
        client(addr);
        return;
    }

    if let Ok(port) = mode.parse() {
        server(port);
        return;
    }
}

fn client(addr: SocketAddr) {
    let mut cn = TcpStream::connect(addr).expect("connect");
    let mut buf = [0u8; std::mem::size_of::<u64>()];
    for i in 0..100_000u32 {
        cn.write_all(&i.to_le_bytes()).expect("write");
        cn.read_exact(&mut buf).expect("read");
    }

    println!("sum: {}", u64::from_le_bytes(buf));
}

fn server(port: u16) {
    let srv = TcpListener::bind(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)))
        .expect("bind");
    let (mut cn, peer) = srv.accept().expect("accept");

    println!("connected to {:?}", peer);

    let mut sum = 0u64;
    let mut buf = [0u8; 4];
    loop {
        cn.read_exact(&mut buf).expect("read");
        sum += u32::from_le_bytes(buf) as u64;
        cn.write_all(&sum.to_le_bytes()).expect("write");
    }
}
