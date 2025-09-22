use crate::protocol::Message;
use minstant::Instant;
use rand::{seq::SliceRandom, Rng};
use std::{
    net::{SocketAddrV4, TcpStream},
    thread,
    time::Duration,
};

#[derive(Debug, Copy, Clone)]
pub enum Workload {
    Ping,
    Mixed,
    File,
    TouchRandom(u64),
    TouchSequential(u64),
    TouchRepeated(u64),
}

impl std::str::FromStr for Workload {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let sp: Vec<_> = s.split(':').collect();
        match &sp[..] {
            [variant] if *variant == "ping" => Ok(Workload::Ping),
            [variant] if *variant == "mixed" => Ok(Workload::Mixed),
            [variant] if *variant == "file" => Ok(Workload::File),
            [variant, amt] if *variant == "touch-random" => {
                Ok(Workload::TouchRandom(amt.parse().unwrap()))
            }
            [variant, amt] if *variant == "touch-sequential" => {
                Ok(Workload::TouchSequential(amt.parse().unwrap()))
            }
            [variant, amt] if *variant == "touch-repeated" => {
                Ok(Workload::TouchRepeated(amt.parse().unwrap()))
            }
            _ => panic!("Cant parse {}", s),
        }
    }
}

impl std::fmt::Display for Workload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Workload::Ping => write!(f, "ping"),
            Workload::File => write!(f, "file"),
            Workload::Mixed => write!(f, "mixed"),
            Workload::TouchRandom(x) => write!(f, "touch-random:{}", x),
            Workload::TouchSequential(x) => write!(f, "touch-sequential:{}", x),
            Workload::TouchRepeated(x) => write!(f, "touch-repeated:{}", x),
        }
    }
}

#[derive(Copy, Clone)]
pub struct Latency {
    query_latency: u64,
    work_duration: u64,
    touched: u64,
}

fn client(
    mut stream: TcpStream,
    dur: Duration,
    keyspace: u64,
    _value_sz: usize,
    workload: Workload,
) -> Result<Vec<Latency>, anyhow::Error> {
    let start = Instant::now();
    let mut v = Vec::new();
    let mut rng = rand::rng();

    let mut shuffled_keyspace: Vec<u64> = (0..keyspace).collect();
    shuffled_keyspace.shuffle(&mut rng);

    while start.elapsed() < dur {
        let msg = match workload {
            Workload::Ping => Message::Noop,
            Workload::File => Message::File,
            Workload::TouchRandom(amt) => {
                let s = rng.random_range(0..keyspace - amt) as usize;
                let e = s + amt as usize;
                let v: Vec<_> = shuffled_keyspace[s..e].into();
                Message::Touch(v)
            }
            Workload::TouchRepeated(amt) => {
                let e = amt as usize;
                let v: Vec<_> = shuffled_keyspace[..e].into();
                Message::Touch(v)
            }
            Workload::TouchSequential(amt) => {
                let start: u64 = rng.random_range(0..keyspace - amt);
                Message::TouchSeq((start, amt as usize))
            }
            _ => unimplemented!(),
        };
        let now = Instant::now();
        if msg.write(&mut stream).is_err() {
            break;
        }
        let ack = match Message::read(&mut stream) {
            Err(_) => break,
            Ok(x) => x,
        };
        let query_latency = now.elapsed().as_micros() as u64;
        let work_duration = ack.work_duration().unwrap();
        let touched = ack.touched().unwrap();
        v.push(Latency {
            query_latency,
            work_duration,
            touched,
        })
    }
    Ok(v)
}

pub fn run(
    server_addr: SocketAddrV4,
    n_clients: u32,
    dur: Duration,
    k: u64,
    v: usize,
    w: Workload,
) {
    let mut handles = Vec::new();
    for _ in 0..n_clients {
        // Closed loop
        let stream = TcpStream::connect(&server_addr).unwrap();
        let h = thread::spawn(move || client(stream, dur, k, v, w));
        handles.push(h);
    }

    let now = Instant::now();
    let mut latencies: Vec<Vec<Latency>> = Vec::new();
    for h in handles {
        latencies.push(h.join().unwrap().unwrap());
    }
    let dur = now.elapsed();

    let mut touched = 0;
    let mut requests = 0;
    for v in latencies.iter() {
        for e in v {
            touched += e.touched;
        }
        requests += v.len();
    }

    let requests_per_sec = requests / dur.as_secs() as usize;
    let touched_per_sec = touched / dur.as_secs() as u64;

    println!();
    println!();
    println!();
    println!("==================================");
    println!("Requests per sec: {requests_per_sec}");
    println!("Lookups per sec: {touched_per_sec}");
    println!("==================================");
    println!();
    println!();
    println!();

    // Calculate some stats
    println!("Query latency stats");
    println!("==================================");
    let stats = Stats::calc(&mut latencies);
    println!(
        "{0: <10} | {1: <5} | {2: <5} | {3: <5} | {4: <5}",
        " ", "p50", "p75", "p90", "p99"
    );
    println!(
        "{0: <10} | {1: <5} | {2: <5} | {3: <5} | {4: <5}",
        "query lat",
        stats.p50.query_latency,
        stats.p75.query_latency,
        stats.p90.query_latency,
        stats.p99.query_latency,
    );

    println!(
        "{0: <10} | {1: <5} | {2: <5} | {3: <5} | {4: <5}",
        "work time",
        stats.p50.work_duration,
        stats.p75.work_duration,
        stats.p90.work_duration,
        stats.p99.work_duration,
    );
}

#[allow(dead_code)]
struct Stats {
    min: Latency,
    p50: Latency,
    p75: Latency,
    p90: Latency,
    p99: Latency,
    p999: Latency,
    max: Latency,
}

impl Stats {
    fn calc(data: &mut [Vec<Latency>]) -> Self {
        let l = data.len() as f64;

        for d in data.iter_mut() {
            d.sort_by_key(|x| x.query_latency);
        }

        let mut buf = Vec::new();

        // min
        buf.clear();
        for d in data.iter() {
            buf.push(d[0]);
        }
        buf.sort_by_key(|x| x.query_latency);
        let idx = (l * 0.5) as usize;
        let min = buf[idx];

        // max
        buf.clear();
        for d in data.iter() {
            buf.push(*d.last().unwrap());
        }
        buf.sort_by_key(|x| x.query_latency);
        let idx = (l * 0.5) as usize;
        let max = buf[idx];

        // quantiles
        let mut tiles = Vec::new();
        for i in [0.5, 0.75, 0.90, 0.99, 0.999] {
            buf.clear();
            for d in data.iter() {
                let idx = (d.len() as f64 * i) as usize;
                buf.push(d[idx]);
            }
            buf.sort_by_key(|x| x.query_latency);
            let idx = (l * 0.5) as usize;
            tiles.push(buf[idx]);
        }

        Stats {
            min,
            max,
            p50: tiles[0],
            p75: tiles[1],
            p90: tiles[2],
            p99: tiles[3],
            p999: tiles[4],
        }
    }
}

//fn receiver(
//	mut stream: TcpStream,
//	done: Arc<AtomicBool>,
//	start: Instant,
//) -> Result<Vec<Latency>, anyhow::Error> {
//	let mut v = Vec::new();
//	while !done.load(SeqCst) {
//		let ack = match Message::read(&mut stream) {
//			Err(_) => break,
//			Ok(x) => x,
//		};
//		let now = start.elapsed().as_micros() as u64;
//		let query_ts = ack.query_ts().unwrap();
//		let query_latency = now - query_ts;
//		let work_duration = ack.work_duration().unwrap();
//		v.push(Latency {
//			query_latency,
//			work_duration,
//		})
//	}
//	Ok(v)
//}
//
//fn sender(
//	mut stream: TcpStream,
//	dur: Duration,
//	interval: Duration,
//	done: Arc<AtomicBool>,
//	start: Instant,
//) -> Result<(), anyhow::Error> {
//	let lambda = 1. / interval.as_nanos() as f64;
//	let exp = rand_distr::Exp::new(lambda).unwrap();
//	let mut excess_duration = Duration::from_secs(0);
//	let mut rng = rand::thread_rng();
//
//	while start.elapsed() < dur {
//		let mut top_of_loop = Instant::now();
//
//		// Write
//		let query_ts = start.elapsed().as_micros() as u64;
//		let q = Message::new_query(query_ts, 524);
//		if q.write(&mut stream).is_err() {
//			break;
//		}
//
//		// And then wait, and keep track of duration
//		let d_nanos = exp.sample(&mut rng) as u64;
//		let wait_duration = Duration::from_nanos(d_nanos);
//		while top_of_loop.elapsed() < wait_duration {
//			// Spin or yield
//		}
//		let excess = top_of_loop.elapsed() - wait_duration;
//		if excess > Duration::from_secs(0) {
//			excess_duration += excess;
//		}
//
//		// If the accumulated excess > interval, send a request
//		while excess_duration >= interval {
//			excess_duration -= interval;
//			let query_ts = start.elapsed().as_micros() as u64;
//			let q = Message::new_query(query_ts, 524);
//			if q.write(&mut stream).is_err() {
//				break;
//			}
//		}
//	}
//	done.store(true, SeqCst);
//	Ok(())
//}
