# Fuzzing with AFL

```bash
cargo install cargo-afl
cargo afl build
./compile_cases.sh
afl-fuzz -i ./inputs-compiled/ -o ./outputs -- ./target/debug/fuzz-afl
```

## Debugging a crash

```bash
./target/debug/fuzz-afl --manual < ./outputs/crash_file
```