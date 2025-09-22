use clap::{Parser, Subcommand};
use perf_examples::varwork_client::{self, Workload};
use perf_examples::varwork_server::{self, StorageKind};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Duration;

#[derive(Subcommand, Debug)]
pub enum ClientServer {
    Client {
        #[arg(short, long)]
        ip: Ipv4Addr,

        #[arg(short, long)]
        port: u16,

        #[arg(short, long)]
        n_clients: u32,

        #[arg(short, long)]
        workload: Workload,
    },
    Server {
        #[arg(short, long)]
        port: u16,

        #[arg(short, long, value_enum)]
        storage: StorageKind,
    },
}

#[derive(Parser, Debug)]
pub struct Args {
    #[arg(short, long)]
    duration: u64,

    #[arg(short, long)]
    keyspace: u64,

    #[arg(short, long)]
    value_size: usize,

    #[command(subcommand)]
    command: ClientServer,
}

fn main() {
    let args = Args::parse();
    let dur = Duration::from_secs(args.duration);
    let ks = args.keyspace;
    let vs = args.value_size;

    match args.command {
        ClientServer::Client {
            ip,
            port,
            n_clients,
            workload,
        } => {
            let s = SocketAddrV4::new(ip, port);
            let n = n_clients;
            let w = workload;
            varwork_client::run(s, n, dur, ks, vs, w);
        }
        ClientServer::Server { port, storage } => {
            let ip = Ipv4Addr::UNSPECIFIED;
            let server_addr = SocketAddrV4::new(ip, port);
            varwork_server::run(server_addr, storage, ks, vs, dur);
        }
    }
}
