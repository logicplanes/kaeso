# Fuzzing

One cargo-fuzz target per KPP decoder plus the SSE parser, in a Cargo workspace of its own so `libfuzzer-sys` never reaches the library crates; the seed corpus is `crates/kaeso-codecs/fixtures`. How to run the targets and handle a crash is in the documentation: [Testing › Fuzzing](https://docs.kaeso.app/development/testing#fuzzing).
