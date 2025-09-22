pub fn do_long_work() {
    let k = 2350845.545;
    for i in 0..9001 {
        std::hint::black_box(f64::sqrt(k * i as f64));
    }
}

pub fn do_short_work() {
    let k = 2350845.545;
    for i in 0..901 {
        std::hint::black_box(f64::sqrt(k * i as f64));
    }
}
