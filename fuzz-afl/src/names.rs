const NAMES: &str = include_str!("./names.txt");

pub fn build() -> Vec<&'static str> {
    let mut out = Vec::with_capacity(256);
    for name in NAMES.lines() {
        out.push(name);
    }
    out
}