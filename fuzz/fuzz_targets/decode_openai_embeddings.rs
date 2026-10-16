#![no_main]

use kaeso_kpp::DialectId;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| kpp_fuzz::decoders(DialectId::OpenAiEmbeddings, data));
