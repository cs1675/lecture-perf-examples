import logging
import os
import re
import subprocess as sh
import time

impls = ["threads", "nonblocking"]

def bin_name(impl):
    return f"manyconns_{impl}"

def build(impl):
    sh.run(f"cargo build --release --bin=\"{bin_name(impl)}\"", shell=True, check=True)

def is_listening(port):
    # don't probe by connecting: manyconns_threads exits once its last connection closes.
    out = sh.run(f"ss -Hltn 'sport = :{port}'", shell=True, capture_output=True, text=True)
    return out.stdout.strip() != ""

def start_server(impl, port, taskset, outdir, timeout=10):
    srv = sh.Popen(f"\
        exec taskset \
          -c {taskset} \
        ./target/release/{bin_name(impl)} \
          {port} \
      > {outdir}/server.out \
      2> {outdir}/server.err",
      shell=True)
    start = time.time()
    while not is_listening(port):
        if srv.poll() is not None:
            raise Exception(f"server exited with {srv.returncode}, see {outdir}/server.err")
        if time.time() - start > timeout:
            srv.kill()
            raise Exception(f"server did not start listening on port {port}")
        time.sleep(0.1)
    return srv

def stop_server(srv, timeout=5):
    # manyconns_threads exits on its own after the client disconnects; manyconns_nonblocking never does.
    try:
        srv.wait(timeout=timeout)
    except sh.TimeoutExpired:
        srv.terminate()
        srv.wait()

def run_client(impl, ip, port, taskset, num_conns, outdir):
    sh.run(f"\
      taskset \
        -c {taskset} \
      ./target/release/{bin_name(impl)} \
        {ip}:{port} \
        {num_conns} \
      > {outdir}/client.out \
      2> {outdir}/client.err",
      shell=True, check=True)

duration_units_us = {"ns": 1e-3, "µs": 1, "us": 1, "ms": 1e3, "s": 1e6}

def parse_p95(fn):
    """parse the Debug-formatted Duration printed by sum_client into microseconds."""
    with open(fn, 'r') as f:
        for line in f:
            m = re.search(r"median per-connection p95 latency: ([0-9.]+)(ns|µs|us|ms|s)", line)
            if m:
                return float(m.group(1)) * duration_units_us[m.group(2)]
    raise Exception(f"{fn} did not contain a p95 latency")

def run(impl, ip, port, client_taskset, server_taskset, num_conns, outdir):
    logging.info(f"running {impl} with {num_conns} connections")
    os.makedirs(outdir, exist_ok=True)
    srv = start_server(impl, port, server_taskset, outdir)
    try:
        run_client(impl, ip, port, client_taskset, num_conns, outdir)
    finally:
        stop_server(srv)
    p95 = parse_p95(f"{outdir}/client.out")
    logging.info(f"done: {impl} {num_conns} connections median p95 = {p95:.3f}us")
    return p95

if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument('--impl', type=str, choices=impls + ["both"], default="both")
    parser.add_argument('--num_conns', type=str, default="1,2,4,8,16,32,64,128",
                        help="comma-separated list of connection counts")
    parser.add_argument('--ip', type=str, default="127.0.0.1")
    parser.add_argument('--port', type=int, default=4242)
    parser.add_argument('--client_taskset', type=str, default="0-1")
    parser.add_argument('--server_taskset', type=str, default="2-5")
    parser.add_argument('--outdir', type=str, default=None,
                        help="defaults to manyconns_{impl}")
    parser.add_argument('--loglevel', type=str, default='INFO')
    args = parser.parse_args()
    logging.getLogger().setLevel(args.loglevel)
    if args.outdir is None:
        args.outdir = f"manyconns_{args.impl}"

    run_impls = impls if args.impl == "both" else [args.impl]
    num_conns = [int(n) for n in args.num_conns.split(',')]

    for impl in run_impls:
        build(impl)

    results = []
    for impl in run_impls:
        for n in num_conns:
            p95 = run(
              impl,
              args.ip,
              args.port,
              args.client_taskset,
              args.server_taskset,
              n,
              f"{args.outdir}/{impl}-{n}")
            results.append((impl, n, p95))

    with open(f"{args.outdir}/results.csv", 'w') as f:
        f.write("impl,num_conns,median_p95_us\n")
        for impl, n, p95 in results:
            f.write(f"{impl},{n},{p95}\n")

    print("impl,num_conns,median_p95_us")
    for impl, n, p95 in results:
        print(f"{impl},{n},{p95:.3f}")
