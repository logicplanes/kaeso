# Fuzzing

One cargo-fuzz target per KPP decoder plus the SSE parser, MCP messages (`crates/kaeso-mcp`), the audio length probe and the image size probe, in a Cargo workspace of its own so `libfuzzer-sys` never reaches the library crates; the seed corpus is `crates/kaeso-codecs/fixtures`, and `crates/kaeso-mcp/fixtures` for `mcp`. How to run the targets and handle a crash is in the documentation: [Testing › Fuzzing](https://docs.kaeso.app/development/testing#fuzzing).
