use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};

pub fn client(addr: SocketAddr, num_conns: u32) {
    let mut conns = Vec::new();
    const WORK_AMT: u32 = 500_000;
    for t in 0..num_conns {
        let start = t * num_conns;
        let end = t * num_conns + (WORK_AMT / num_conns);
        let cn = TcpStream::connect(addr).expect("connect");
        conns.push((start, end, cn));
    }

    let mut jhs = Vec::new();
    for (start, end, mut cn) in conns {
        let jh = std::thread::spawn(move || {
            let mut durs = Vec::with_capacity((WORK_AMT / num_conns) as usize);
            let mut buf = [0u8; std::mem::size_of::<u64>()];
            for i in start..end {
                let req_start = std::time::Instant::now();
                cn.write_all(&i.to_le_bytes()).expect("write");
                cn.read_exact(&mut buf).expect("read");
                let req_end = std::time::Instant::now();
                let req_duration = req_end - req_start;
                durs.push(req_duration);
            }

            durs.sort();
            let p95 = durs[durs.len() * 95 / 100];
            (u64::from_le_bytes(buf), p95)
        });
        jhs.push(jh);
    }

    let mut sum = 0;
    let mut p95s = Vec::with_capacity(num_conns as _);
    for c in jhs {
        let (conn_sum, p95) = c.join().expect("join");
        sum = conn_sum.max(sum);
        p95s.push(p95);
    }

    p95s.sort();
    let median_conn_p95 = p95s[p95s.len() / 2];
    println!(
        "sum: {} median per-connection p95 latency: {:?}",
        sum, median_conn_p95
    );
}
