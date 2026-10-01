import logging
import subprocess as sh
import os

cfg_keys = {
    "net": ["ip", "port"],
    "exp": ["duration", "value_size", "key_space_size", "server_storage_impl", "work_type"],
    "client": ["taskset", "num_clients"],
    "server": ["taskset", "record_dcache"],
}

record_dcache_modes = ["off", "record", "event"]

def build(record_dcache):
    features = "perf" if record_dcache == "event" else "varwork"
    sh.run(f"cargo build --release --features=\"{features}\" --bin=\"varwork\"", shell=True)

class Exp:
    duration = None
    value_size = None
    key_space_size = None
    server_storage_impl = None
    work_type = None

    def __init__(self, cfg) -> None:
        self.duration = cfg["duration"]
        self.value_size = cfg["value_size"]
        self.key_space_size = cfg["key_space_size"]
        self.server_storage_impl = cfg["server_storage_impl"]
        self.work_type = cfg["work_type"]

    def __str__(self):
        return f"Exp<duration: {self.duration}, value_size: {self.value_size}, keyspace: {self.key_space_size}, storage: {self.server_storage_impl}, workload: {self.work_type}>"

def check_file(fn, search_str):
    with open(fn, 'r') as f:
        found = False
        for line in f:
            if search_str in line:
                found = True
                break
        assert found, f"{fn} did not contain {search_str}"

def start_server(port, taskset, record_dcache, exp, outdir):
    prefix = ""
    if record_dcache != "event":
        perf_args = f"-c \"record -o {outdir}/perf.data --call-graph dwarf,64000"
        if record_dcache == "record":
            perf_args += " -e L1-dcache-load-misses"
        perf_args += "\""
        prefix = f"\
        flamegraph \
          -o {outdir}/flamegraph.{exp.server_storage_impl}.{exp.work_type}.svg \
          {perf_args} \
          --"
    sh.Popen(f"\
        {prefix} \
        taskset \
          -c {taskset} \
        ./target/release/varwork \
	      --duration {exp.duration} \
	      --keyspace {exp.key_space_size} \
	      --value-size {exp.value_size} \
        server \
          --port {port} \
          --storage {exp.server_storage_impl} \
      > {outdir}/server.out \
      2> {outdir}/server.err",
      shell=True)
    import time
    start = time.time()
    while True:
        time.sleep(1)
        try:
            check_file(f"{outdir}/server.out", "Server listening")
            break
        except Exception as e:
            if time.time() - start > float(exp.duration) * 1.5:
                raise e

def run_client(ip, port, taskset, n_clients, exp, outdir):
	sh.run(f"\
      taskset \
        -c {taskset} \
      ./target/release/varwork \
	    --duration {exp.duration} \
	    --keyspace {exp.key_space_size} \
	    --value-size {exp.value_size} \
	  client \
	    --ip {ip} \
	    --port {port} \
	    --n-clients {n_clients} \
	    --workload {exp.work_type} \
      > {outdir}/client.out \
      2> {outdir}/client.err",
      shell=True)

def run(ip, port, client, server, exp, outdir, setup_only=False):
    logging.info(f"running {exp}")
    logging.debug("building varwork app")
    build(server["record_dcache"])
    logging.debug("done building")
    if setup_only:
        return

    logging.debug("starting server")
    start_server(
      port,
      server["taskset"],
      server["record_dcache"],
      exp,
      outdir)

    logging.debug("running client")
    run_client(
      ip,
      port,
      client["taskset"],
      client["num_clients"],
      exp,
      outdir)
    logging.info("done")

    sh.run("sudo pkill -INT varwork", shell=True)

if __name__ == '__main__':
    import argparse
    import toml
    parser = argparse.ArgumentParser()
    parser.add_argument('--config', type=str, required=True)
    parser.add_argument('--loglevel', type=str, default='INFO')
    parser.add_argument('--overwrite', action='store_true')
    parser.add_argument('--setup_only', action='store_true',  required=False)
    args = parser.parse_args()
    logging.getLogger().setLevel(args.loglevel)

    cfg = toml.load(args.config)
    logging.info(f"read config from {args.config}: {cfg}")
    args.outdir = os.path.basename(args.config).split('.')[0]
    if os.path.exists(args.outdir) and args.overwrite:
        sh.run("rm -rf {args.outdir}", shell=True)

    if not os.path.exists(args.outdir):
        os.mkdir(args.outdir)

    for val in cfg_keys:
        cfgval = cfg[val]
        for key in cfg_keys[val]:
            assert key in cfgval

    assert cfg["server"]["record_dcache"] in record_dcache_modes, \
        f"record_dcache must be one of {record_dcache_modes}"

    sh.run(f"cp {args.config} {args.outdir}/exp.toml", shell=True)

    ip = cfg["net"]["ip"]
    port = cfg["net"]["port"]
    run(
      ip,
      port,
      cfg["client"],
      cfg["server"],
      Exp(cfg["exp"]),
      args.outdir,
      setup_only=args.setup_only)
