use std::io::Read;

use afl::fuzz;

fn main() {
    let manual = std::env::args().any(|a|a == "--manual");
    let f = fuzzable::Fuzz::new(manual);
    if manual {
        let mut stdin = std::io::stdin().lock();
        let mut v = Vec::new();
        stdin.read_to_end(&mut v).unwrap();
        f.run(&v);
    } else {
        fuzz!(|data: &[u8]| {
            f.run(data);
        });
    }
}
