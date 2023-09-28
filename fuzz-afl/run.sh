#!/bin/sh
for i in $(seq 1 $(nproc)); do
    echo "Launch $i"
    AFL_NO_UI=1 afl-fuzz -S "fuzzer-$i" -i ./fuzz/inputs/ -o ./fuzz-outputs/ -- ./target/release/fuzz &
done