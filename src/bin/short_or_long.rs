use perf_examples::short_long::*;
use rand::Rng;

fn main() {
    let start = std::time::Instant::now();
    let mut rng = rand::rng();

    loop {
        if rng.random_bool(0.5) {
            do_long_work();
        } else {
            do_short_work();
        }
        if start.elapsed() > std::time::Duration::from_secs(10) {
            break;
        }
    }
}
