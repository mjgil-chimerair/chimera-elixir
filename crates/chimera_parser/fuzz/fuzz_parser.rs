#![no_main]

use libfuzzer_sys::fuzz_target;
use chimera_parser::Parser;
use chimera_source::SourceFileId;

fuzz_target!(|data: &[u8]| {
    // Limit input size to prevent excessive memory usage
    if data.len() > 10000 {
        return;
    }

    // Try to parse the input as UTF-8
    if let Ok(source) = std::str::from_utf8(data) {
        // Parse with default options
        let mut parser = Parser::new(source, SourceFileId::new(0));
        let _ = parser.parse();
    }
});
