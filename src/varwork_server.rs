use crate::protocol::Message;
use crate::utils::*;
use minstant::Instant;
use rand::Rng;
use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::PathBuf;
use std::{
    net::{SocketAddrV4, TcpListener, TcpStream},
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(clap::ValueEnum, Debug, Copy, Clone)]
pub enum StorageKind {
    None,
    HashMap,
    BTreeMap,
}

pub fn run(
    addr: SocketAddrV4,
    storage_kind: StorageKind,
    keyspace: u64,
    value_sz: usize,
    dur: Duration,
) {
    let now = Instant::now();
    let storage = Storage::from_kind(&storage_kind, keyspace, value_sz);
    println!("Initing storage: {:?}", now.elapsed());
    listener(addr, storage, dur);
}

fn listener(addr: SocketAddrV4, storage: Storage, dur: Duration) {
    let listener = TcpListener::bind(&addr).unwrap();
    println!("Server listening");
    let stream = listener.accept().unwrap();
    let result = handle_conn(stream.0, storage.clone(), dur).unwrap();

    let mut percents: Vec<(f64, u64, u64)> = result
        .iter()
        .map(|(x, y)| (*y as f64 / *x as f64, *y, *x))
        .collect();

    percents.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let l = result.len() as f64;
    let p50_idx = (l * 0.5).floor() as usize;
    let p90_idx = (l * 0.9).floor() as usize;

    println!();
    println!();
    println!();
    println!("=======================================");
    println!("Work / Total Time per Request");
    println!("=======================================");
    println!();

    println!(
        "{0: <5} | {1: <10} | {2: <10} | {3: <10} ",
        " ", "work time", "total time", "work %"
    );
    let qtile = percents[p50_idx];
    println!(
        "{0: <5} | {1: <10} | {2: <10} | {3:0.2} ",
        "p50",
        qtile.1,
        qtile.2,
        qtile.0 * 100.
    );
    let qtile = percents[p90_idx];
    println!(
        "{0: <5} | {1: <10} | {2: <10} | {3:0.2} ",
        "p90",
        qtile.1,
        qtile.2,
        qtile.0 * 100.
    );
    println!();
    println!();
    println!();
}

fn handle_conn(
    mut stream: TcpStream,
    s: Storage,
    dur: Duration,
) -> Result<Vec<(u64, u64)>, anyhow::Error> {
    let mut timing = Vec::new();
    #[cfg(feature = "perf")]
    let mut counters = L1Counters::new()?;
    let start = Instant::now();
    while start.elapsed() < dur {
        /*
         * Keep track of the entire time it takes to receive a message,
         * do work, and send a response
         */
        let top = Instant::now();

        let msg = match Message::read(&mut stream) {
            Ok(x) => x,
            Err(_) => break,
        };

        /*
         * Perform work and keep track of how much time it takes to do
         * this work.
         *
         * Update the work response message with this time.
         */
        #[cfg(feature = "perf")]
        counters.start()?;
        let now = Instant::now();
        let mut response = do_work(msg, &s);
        let work_d = now.elapsed().as_micros() as u64;
        #[cfg(feature = "perf")]
        counters.stop()?;

        match &mut response {
            Message::Ok { work_duration, .. } => {
                *work_duration = work_d;
            }
            _ => {}
        }

        if response.write(&mut stream).is_err() {
            break;
        };

        /*
         * Record how much time this iteration took
         */
        let total_time = top.elapsed().as_micros() as u64;
        timing.push((total_time, work_d));
    }

    #[cfg(feature = "perf")]
    counters.report()?;

    Ok(timing)
}

#[cfg(feature = "perf")]
struct L1Counters {
    group: perf_event::Group,
    access: perf_event::Counter,
    miss: perf_event::Counter,
    calls: u64,
}

#[cfg(feature = "perf")]
impl L1Counters {
    fn new() -> Result<Self, anyhow::Error> {
        use perf_event::events::{Cache, CacheOp, CacheResult, WhichCache};
        use perf_event::{Builder, Group};

        const ACCESS: Cache = Cache {
            which: WhichCache::L1D,
            operation: CacheOp::READ,
            result: CacheResult::ACCESS,
        };
        const MISS: Cache = Cache {
            result: CacheResult::MISS,
            ..ACCESS
        };

        let mut group = Group::new()?;
        let access = Builder::new().group(&mut group).kind(ACCESS).build()?;
        let miss = Builder::new().group(&mut group).kind(MISS).build()?;
        Ok(Self {
            group,
            access,
            miss,
            calls: 0,
        })
    }

    fn start(&mut self) -> Result<(), anyhow::Error> {
        self.group.enable()?;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), anyhow::Error> {
        self.group.disable()?;
        self.calls += 1;
        Ok(())
    }

    fn report(&mut self) -> Result<(), anyhow::Error> {
        let counts = self.group.read()?;
        let access = counts[&self.access];
        let miss = counts[&self.miss];
        let calls = self.calls.max(1) as f64;

        println!();
        println!("=======================================");
        println!("L1 dcache loads in do_work");
        println!("=======================================");
        println!();
        println!("{0: <12} | {1: <14} | {2: <10}", " ", "total", "per call");
        println!("{0: <12} | {1: <14} | {2: <10}", "calls", self.calls, 1);
        println!(
            "{0: <12} | {1: <14} | {2:0.2}",
            "accesses",
            access,
            access as f64 / calls
        );
        println!(
            "{0: <12} | {1: <14} | {2:0.2}",
            "misses",
            miss,
            miss as f64 / calls
        );
        println!(
            "{0: <12} | {1:0.2}%",
            "miss rate",
            miss as f64 / access.max(1) as f64 * 100.
        );
        if counts.time_running() < counts.time_enabled() {
            println!(
                "warning: counters were multiplexed (running {} of {} ns enabled)",
                counts.time_running(),
                counts.time_enabled()
            );
        }
        Ok(())
    }
}

fn do_work(msg: Message, s: &Storage) -> Message {
    match msg {
        Message::Noop => Message::Ok {
            touched: 0,
            work_duration: 0,
        },

        Message::Touch(k) => {
            let touched = s.touch(&k).len() as u64;
            Message::Ok {
                touched,
                work_duration: 0,
            }
        }

        Message::TouchSeq((start, c)) => {
            let touched = s.touch_seq(start, c).len() as u64;
            Message::Ok {
                touched,
                work_duration: 0,
            }
        }

        Message::File => {
            let p = PathBuf::from("tmp.file");
            let mut f = new_direct_file(&p, BLOCK_SZ);
            let block = AlignedBlock::new_boxed();
            f.write_all(&**block).unwrap();
            f.flush().unwrap();
            Message::Ok {
                touched: 0,
                work_duration: 0,
            }
        }

        Message::Ok { .. } => Message::Err,
        Message::Err => Message::Err,
        Message::Put((key, val)) => {
            s.put(key, &val);
            Message::Ok {
                work_duration: 0,
                touched: 1,
            }
        }
    }
}

#[derive(Clone)]
pub enum Storage {
    None,
    HashMap(Arc<Mutex<HashMap<u64, Arc<Vec<u8>>>>>),
    BTreeMap(Arc<Mutex<BTreeMap<u64, Arc<Vec<u8>>>>>),
}

impl Storage {
    fn from_kind(kind: &StorageKind, k: u64, v: usize) -> Self {
        match kind {
            StorageKind::None => Self::None,
            StorageKind::HashMap => {
                let mut rng = rand::rng();
                let mut map = HashMap::new();
                for i in 0..k {
                    let val = rng.random();
                    let v = Arc::new(vec![val; v]);
                    map.insert(i, v.clone());
                }
                Self::HashMap(Arc::new(Mutex::new(map)))
            }
            StorageKind::BTreeMap => {
                let mut rng = rand::rng();
                let mut map = BTreeMap::new();
                for i in 0..k {
                    let val = rng.random();
                    let v = Arc::new(vec![val; v]);
                    map.insert(i, v.clone());
                }
                Self::BTreeMap(Arc::new(Mutex::new(map)))
            }
        }
    }

    fn put(&self, k: u64, v: &[u8]) {
        match self {
            Self::None => {}
            Self::BTreeMap(x) => x.put(k, v),
            Self::HashMap(x) => x.put(k, v),
        }
    }

    fn touch(&self, keys: &[u64]) -> HashMap<u64, Arc<Vec<u8>>> {
        match self {
            Self::None => HashMap::new(),
            Self::HashMap(x) => x.touch(keys),
            Self::BTreeMap(x) => x.touch(keys),
        }
    }

    fn touch_seq(&self, s: u64, c: usize) -> HashMap<u64, Arc<Vec<u8>>> {
        match self {
            Self::None => HashMap::new(),
            Self::HashMap(x) => x.touch_seq(s, c),
            Self::BTreeMap(x) => x.touch_seq(s, c),
        }
    }
}

pub trait StorageTrait {
    fn put(&self, k: u64, v: &[u8]);
    fn touch(&self, k: &[u64]) -> HashMap<u64, Arc<Vec<u8>>>;
    fn touch_seq(&self, s: u64, c: usize) -> HashMap<u64, Arc<Vec<u8>>>;
}

impl StorageTrait for Arc<Mutex<HashMap<u64, Arc<Vec<u8>>>>> {
    fn put(&self, k: u64, v: &[u8]) {
        let mut buf: Vec<u8> = Vec::new();
        buf.resize(v.len(), 0u8);
        buf.copy_from_slice(v);
        self.lock().unwrap().insert(k, Arc::new(buf));
    }

    fn touch(&self, keys: &[u64]) -> HashMap<u64, Arc<Vec<u8>>> {
        let mut map = HashMap::new();
        let guard = self.lock().unwrap();
        for k in keys {
            if let Some(x) = guard.get(k) {
                map.insert(*k, x.clone());
            }
        }
        map
    }

    fn touch_seq(&self, s: u64, c: usize) -> HashMap<u64, Arc<Vec<u8>>> {
        let mut map = HashMap::new();
        let guard = self.lock().unwrap();
        for k in s..s + c as u64 {
            if let Some(x) = guard.get(&k) {
                map.insert(k, x.clone());
            }
        }
        map
    }
}

impl StorageTrait for Arc<Mutex<BTreeMap<u64, Arc<Vec<u8>>>>> {
    fn put(&self, k: u64, v: &[u8]) {
        let mut buf: Vec<u8> = Vec::new();
        buf.resize(v.len(), 0u8);
        buf.copy_from_slice(v);
        self.lock().unwrap().insert(k, Arc::new(buf));
    }

    fn touch(&self, keys: &[u64]) -> HashMap<u64, Arc<Vec<u8>>> {
        let mut map = HashMap::new();
        let guard = self.lock().unwrap();
        for k in keys {
            if let Some(x) = guard.get(k) {
                map.insert(*k, x.clone());
            }
        }
        map
    }

    fn touch_seq(&self, s: u64, c: usize) -> HashMap<u64, Arc<Vec<u8>>> {
        let mut map = HashMap::new();
        let guard = self.lock().unwrap();
        let x = guard.range(s..s + 1 + c as u64);
        for (k, v) in x {
            map.insert(*k, v.clone());
        }
        map
    }
}
