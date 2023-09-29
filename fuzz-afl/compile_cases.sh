#!/bin/sh

rm -rf ./inputs-compiled/
mkdir ./inputs-compiled
mkdir ./outputs
ls ./inputs | grep '\.case$' | while read x; do
    echo "Compiling $x"
    ./target/debug/compile-case < "./inputs/$x" > "./inputs-compiled/$x.bin"
done