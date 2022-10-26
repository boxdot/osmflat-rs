#!/bin/zsh
rm -rf /Users/n/geodata/flatdata/santacruz/*
cd ../osmflatc
cargo run --release -- /Users/n/geodata/extracts/santacruz.pbf /Users/n/geodata/flatdata/santacruz
cd ../osmflat
cargo run --example sort_hilbert -- /Users/n/geodata/flatdata/santacruz
