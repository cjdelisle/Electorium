#!/bin/sh
for i in $(seq 1 $(nproc)); do
    echo "Launch $i"
    afl-fuzz -S "fuzzer-$i" -i ./fuzz/inputs/ -o ./fuzz-outputs/ -- ./target/release/fuzz
done