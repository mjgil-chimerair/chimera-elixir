#![no_main]

use libfuzzer_sys::fuzz_target;
use chimera_lexer::{Lexer, TokenKind};
use chimera_source::{SourceFileId};

fuzz_target!(|data: &[u8]| {
    // Create a lexer from the fuzz input
    if let Ok(source) = std::str::from_utf8(data) {
        let mut lexer = Lexer::new(source, SourceFileId::new(0));
        
        // Tokenize until EOF, catching any panics
        let mut token_count = 0;
        loop {
            match lexer.next_token() {
                Ok(token) => {
                    token_count += 1;
                    if token.kind == TokenKind::Eof {
                        break;
                    }
                    // Prevent infinite loops in case of bugs
                    if token_count > 10000 {
                        break;
                    }
                }
                Err(_) => {
                    // Lexing errors are expected with random input
                    break;
                }
            }
        }
    }
});