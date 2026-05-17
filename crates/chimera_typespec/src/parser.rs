//! Typespec parser module.
//!
//! Parses typespec types and attributes from strings.

use super::*;
use chimera_lexer::{Lexer, Token, TokenKind};

/// Parse a typespec type from a string representation.
pub fn parse_type(source: &str, file_id: SourceFileId) -> Result<TypespecType, String> {
    let mut lexer = Lexer::new(source, file_id);
    parse_type_from_tokens(&mut lexer)
}

fn parse_type_from_tokens(lexer: &mut Lexer) -> Result<TypespecType, String> {
    // First consume any opening paren before calling parse_type_token
    let token = lexer
        .next_token()
        .map_err(|e| format!("lexer error: {:?}", e))?;

    // Check if it's an OpenParen - we might need to handle this specially
    match token.kind {
        TokenKind::OpenParen => {
            // Parenthesized type: (type)
            let inner = parse_type_from_tokens(lexer)?;
            consume_token(lexer, TokenKind::CloseParen)?;
            Ok(TypespecType::Parens(Box::new(inner)))
        }
        _ => parse_type_token(token, lexer),
    }
}

fn parse_type_token(token: Token, lexer: &mut Lexer) -> Result<TypespecType, String> {
    match token.kind {
        TokenKind::Identifier | TokenKind::AliasIdentifier => {
            let type_name = match token.value {
                chimera_lexer::TokenValue::Identifier(ref name) => name.clone(),
                _ => return Err("expected identifier".to_string()),
            };
            // Handle remote types from aliases like "String.t"
            if let Some(dot_pos) = type_name.find('.') {
                let module = type_name[..dot_pos].to_string();
                let type_name_part = type_name[dot_pos + 1..].to_string();
                // Check if this remote type has parens
                let next_tok = lexer.next_token();
                eprintln!("DEBUG: after alias split, next_tok = {:?}", next_tok);
                let args = match next_tok {
                    Ok(Token {
                        kind: TokenKind::OpenParen,
                        ..
                    }) => {
                        let _ = consume_token(lexer, TokenKind::CloseParen);
                        Vec::new()
                    }
                    Ok(Token {
                        kind: TokenKind::LessThan,
                        ..
                    }) => {
                        let mut args = Vec::new();
                        loop {
                            match lexer.next_token() {
                                Ok(Token {
                                    kind: TokenKind::GreaterThan,
                                    ..
                                }) => break,
                                Ok(Token {
                                    kind: TokenKind::Comma,
                                    ..
                                }) => continue,
                                Ok(token) => {
                                    let ty = parse_type_token(token, lexer)?;
                                    args.push(ty);
                                }
                                Err(e) => return Err(format!("{:?}", e)),
                            }
                        }
                        args
                    }
                    _ => Vec::new(),
                };
                let mut table = chimera_term::AtomTable::new();
                return Ok(TypespecType::Remote {
                    module: table.intern(&module),
                    name: table.intern(&type_name_part),
                    args,
                });
            }
            parse_type_name(&type_name, lexer)
        }
        TokenKind::Atom => Ok(TypespecType::LitAtom(Atom::new(0))),
        TokenKind::Integer => {
            let int_val = match token.value {
                chimera_lexer::TokenValue::Integer(n) => n as i64,
                _ => return Err("expected integer".to_string()),
            };
            Ok(TypespecType::LitInteger(int_val))
        }
        _ => Err(format!("unexpected token: {:?}", token.kind)),
    }
}

fn parse_type_name(name: &str, lexer: &mut Lexer) -> Result<TypespecType, String> {
    // Check for remote types
    let next_token = lexer.next_token();
    match next_token {
        Ok(Token {
            kind: TokenKind::Dot,
            ..
        }) => {
            let type_name_token = lexer.next_token().map_err(|e| format!("{:?}", e))?;
            let type_name = match type_name_token.value {
                chimera_lexer::TokenValue::Identifier(n) => n,
                _ => return Err("expected type name after dot".to_string()),
            };

            // Check for generic args or parens
            let next_tok = lexer.next_token();
            let args = match next_tok {
                Ok(Token {
                    kind: TokenKind::LessThan,
                    ..
                }) => {
                    let mut args = Vec::new();
                    loop {
                        match lexer.next_token() {
                            Ok(Token {
                                kind: TokenKind::GreaterThan,
                                ..
                            }) => break,
                            Ok(Token {
                                kind: TokenKind::Comma,
                                ..
                            }) => continue,
                            Ok(token) => {
                                let ty = parse_type_token(token, lexer)?;
                                args.push(ty);
                            }
                            Err(e) => return Err(format!("{:?}", e)),
                        }
                    }
                    args
                }
                Ok(Token {
                    kind: TokenKind::OpenParen,
                    ..
                }) => {
                    let _ = consume_token(lexer, TokenKind::CloseParen);
                    Vec::new()
                }
                _ => Vec::new(),
            };

            let mut table = chimera_term::AtomTable::new();
            Ok(TypespecType::Remote {
                module: table.intern(name),
                name: table.intern(&type_name),
                args,
            })
        }
        Ok(Token {
            kind: TokenKind::OpenParen,
            ..
        }) => {
            if TypespecType::is_builtin_type(name) {
                parse_builtin_with_parens(name, lexer)
            } else {
                Err(format!("'{}' is not a valid type", name))
            }
        }
        Ok(_) => {
            if TypespecType::is_builtin_type(name) {
                parse_builtin_type(name, lexer)
            } else {
                let mut table = chimera_term::AtomTable::new();
                Ok(TypespecType::Variable(table.intern(name)))
            }
        }
        Err(e) => Err(format!("{:?}", e)),
    }
}

fn parse_builtin_with_parens(name: &str, lexer: &mut Lexer) -> Result<TypespecType, String> {
    match name {
        "integer" => {
            let _ = consume_token(lexer, TokenKind::CloseParen);
            Ok(TypespecType::Integer)
        }
        "float" => {
            let _ = consume_token(lexer, TokenKind::CloseParen);
            Ok(TypespecType::Float)
        }
        "number" => {
            let _ = consume_token(lexer, TokenKind::CloseParen);
            Ok(TypespecType::Number)
        }
        "binary" => {
            let _ = consume_token(lexer, TokenKind::CloseParen);
            Ok(TypespecType::Binary)
        }
        "bitstring" => {
            let _ = consume_token(lexer, TokenKind::CloseParen);
            Ok(TypespecType::Bitstring(None))
        }
        "string" => {
            let _ = consume_token(lexer, TokenKind::CloseParen);
            Ok(TypespecType::String)
        }
        "charlist" => {
            let _ = consume_token(lexer, TokenKind::CloseParen);
            Ok(TypespecType::Charlist)
        }
        "boolean" => {
            let _ = consume_token(lexer, TokenKind::CloseParen);
            Ok(TypespecType::Boolean)
        }
        "list" => {
            // list(integer()) - note parentheses for args
            let inner = parse_type_from_tokens(lexer)?;
            // Don't consume close paren here - parse_type_from_tokens will handle it
            Ok(TypespecType::List(Box::new(inner)))
        }
        "tuple" => {
            // tuple(integer(), string()) - tuple with type args
            let args = parse_type_args_paren(lexer)?;
            Ok(TypespecType::Tuple(args))
        }
        "map" => {
            let _ = consume_token(lexer, TokenKind::CloseParen);
            Ok(TypespecType::Map(Vec::new()))
        }
        "atom" => {
            let _ = consume_token(lexer, TokenKind::CloseParen);
            Ok(TypespecType::DynamicAtom)
        }
        "pid" => {
            let _ = consume_token(lexer, TokenKind::CloseParen);
            Ok(TypespecType::Pid)
        }
        "reference" => {
            let _ = consume_token(lexer, TokenKind::CloseParen);
            Ok(TypespecType::Reference)
        }
        "port" => {
            let _ = consume_token(lexer, TokenKind::CloseParen);
            Ok(TypespecType::Port)
        }
        "any" => {
            let _ = consume_token(lexer, TokenKind::CloseParen);
            Ok(TypespecType::Any)
        }
        _ => {
            let _ = consume_token(lexer, TokenKind::CloseParen);
            Ok(TypespecType::Variable(Atom::new(0)))
        }
    }
}

fn parse_type_args_angled(lexer: &mut Lexer) -> Result<Vec<TypespecType>, String> {
    // Check if next token is '<'
    let next_token = lexer.next_token();
    match next_token {
        Ok(Token {
            kind: TokenKind::LessThan,
            ..
        }) => {
            // Parse args until '>'
            let mut args = Vec::new();
            loop {
                match lexer.next_token() {
                    Ok(Token {
                        kind: TokenKind::GreaterThan,
                        ..
                    }) => break,
                    Ok(Token {
                        kind: TokenKind::Comma,
                        ..
                    }) => continue,
                    Ok(token) => {
                        let ty = parse_type_token(token, lexer)?;
                        args.push(ty);
                    }
                    Err(e) => return Err(format!("{:?}", e)),
                }
            }
            Ok(args)
        }
        Ok(Token {
            kind: TokenKind::OpenParen,
            ..
        }) => {
            // This is actually the parens for the type, not angle brackets
            // e.g., String.t() - we already consumed the '(' here
            // We should consume ')' and return empty since there are no type args
            let _ = consume_token(lexer, TokenKind::CloseParen);
            Ok(Vec::new())
        }
        Ok(_) => {
            // Not '<' or '(' - return empty
            Ok(Vec::new())
        }
        Err(_) => Ok(Vec::new()),
    }
}

fn parse_type_args_paren(lexer: &mut Lexer) -> Result<Vec<TypespecType>, String> {
    let mut args = Vec::new();
    loop {
        match lexer.next_token() {
            Ok(Token {
                kind: TokenKind::CloseParen,
                ..
            }) => break,
            Ok(Token {
                kind: TokenKind::Comma,
                ..
            }) => continue,
            Ok(token) => {
                let ty = parse_type_token(token, lexer)?;
                args.push(ty);
            }
            Err(e) => return Err(format!("{:?}", e)),
        }
    }
    Ok(args)
}

fn parse_builtin_type(name: &str, lexer: &mut Lexer) -> Result<TypespecType, String> {
    match name {
        "integer" => Ok(TypespecType::Integer),
        "float" => Ok(TypespecType::Float),
        "number" => Ok(TypespecType::Number),
        "binary" => Ok(TypespecType::Binary),
        "bitstring" => {
            let args = parse_type_args_angled(lexer)?;
            if !args.is_empty() {
                Ok(TypespecType::Bitstring(Some(Box::new(
                    args.into_iter().next().unwrap(),
                ))))
            } else {
                Ok(TypespecType::Bitstring(None))
            }
        }
        "string" => Ok(TypespecType::String),
        "charlist" => Ok(TypespecType::Charlist),
        "boolean" => Ok(TypespecType::Boolean),
        "list" => {
            // list() uses parentheses, not angle brackets
            let next_token = lexer.next_token();
            match next_token {
                Ok(Token {
                    kind: TokenKind::OpenParen,
                    ..
                }) => {
                    let inner = parse_type_from_tokens(lexer)?;
                    consume_token(lexer, TokenKind::CloseParen)?;
                    Ok(TypespecType::List(Box::new(inner)))
                }
                _ => Ok(TypespecType::List(Box::new(TypespecType::Any))),
            }
        }
        "tuple" => {
            let args = parse_type_args_angled(lexer)?;
            Ok(TypespecType::Tuple(args))
        }
        "map" => Ok(TypespecType::Map(Vec::new())),
        "atom" => Ok(TypespecType::DynamicAtom),
        "pid" => Ok(TypespecType::Pid),
        "reference" => Ok(TypespecType::Reference),
        "port" => Ok(TypespecType::Port),
        "any" => Ok(TypespecType::Any),
        _ => Ok(TypespecType::Variable(Atom::new(0))),
    }
}

fn consume_token(lexer: &mut Lexer, expected: TokenKind) -> Result<(), String> {
    match lexer.next_token() {
        Ok(token) if token.kind == expected => Ok(()),
        Ok(token) => Err(format!("expected {:?}, got {:?}", expected, token.kind)),
        Err(e) => Err(format!("{:?}", e)),
    }
}

/// Parse a spec attribute: @spec function_name(type1, type2) :: return_type
pub fn parse_spec(source: &str, file_id: SourceFileId) -> Result<Typespec, String> {
    let mut lexer = Lexer::new(source, file_id);

    let name_token = lexer.next_token().map_err(|e| format!("{:?}", e))?;
    let func_name = match name_token.value {
        chimera_lexer::TokenValue::Identifier(name) => name,
        _ => return Err("expected function name".to_string()),
    };

    consume_token(&mut lexer, TokenKind::OpenParen)?;
    let (args, saw_colon) = parse_type_args_with_colon(&mut lexer)?;
    if !saw_colon {
        // Need to consume the CloseParen and then DoubleColon
        consume_token(&mut lexer, TokenKind::CloseParen)?;
        consume_token(&mut lexer, TokenKind::DoubleColon)?;
    }
    let return_type = parse_type_from_tokens(&mut lexer)?;

    let arity = args.len() as u8;
    let mut table = chimera_term::AtomTable::new();
    let name_atom = table.intern(&func_name);

    Ok(Typespec::Spec {
        name: name_atom,
        arity,
        params: args
            .into_iter()
            .map(|t| TypespecArg::Anonymous(Box::new(t)))
            .collect(),
        return_type: Box::new(return_type),
        meta: SpecMeta::new(file_id),
    })
}

fn parse_type_args(lexer: &mut Lexer) -> Result<Vec<TypespecType>, String> {
    let mut args = Vec::new();
    loop {
        match lexer.next_token() {
            Ok(Token {
                kind: TokenKind::CloseParen,
                ..
            }) => break,
            Ok(Token {
                kind: TokenKind::Comma,
                ..
            }) => continue,
            Ok(token) => {
                // Use parse_type_token directly - it handles the token we already consumed
                let ty = parse_type_token(token, lexer)?;
                args.push(ty);
            }
            Err(e) => return Err(format!("{:?}", e)),
        }
    }
    Ok(args)
}

/// Parse type args and track whether we hit DoubleColon
fn parse_type_args_with_colon(lexer: &mut Lexer) -> Result<(Vec<TypespecType>, bool), String> {
    let mut args = Vec::new();
    loop {
        match lexer.next_token() {
            Ok(Token {
                kind: TokenKind::CloseParen,
                ..
            }) => break,
            Ok(Token {
                kind: TokenKind::Comma,
                ..
            }) => continue,
            Ok(Token {
                kind: TokenKind::DoubleColon,
                ..
            }) => {
                // Return early - don't consume, caller needs to know
                return Ok((args, true));
            }
            Ok(token) => {
                // Use parse_type_token directly - it handles the token we already consumed
                let ty = parse_type_token(token, lexer)?;
                args.push(ty);
            }
            Err(e) => return Err(format!("{:?}", e)),
        }
    }
    Ok((args, false))
}

/// Parse a type attribute: @type name :: type_def
pub fn parse_type_def(source: &str, file_id: SourceFileId) -> Result<Typespec, String> {
    let mut lexer = Lexer::new(source, file_id);

    let name_token = lexer.next_token().map_err(|e| format!("{:?}", e))?;
    let type_name = match name_token.value {
        chimera_lexer::TokenValue::Identifier(name) => name,
        _ => return Err("expected type name".to_string()),
    };

    let mut table = chimera_term::AtomTable::new();
    let name_atom = table.intern(&type_name);

    consume_token(&mut lexer, TokenKind::DoubleColon)?;
    let type_def = parse_type_from_tokens(&mut lexer)?;

    Ok(Typespec::Type {
        name: name_atom,
        type_def: Box::new(type_def),
        meta: TypeMeta::new(file_id),
    })
}

/// Parse any typespec attribute (@spec, @type, @callback, etc.)
pub fn parse_attribute(source: &str, file_id: SourceFileId) -> Result<Typespec, String> {
    let source = source.trim_start_matches('@').trim();

    if source.starts_with("spec ") {
        parse_spec(&source[5..], file_id)
    } else if source.starts_with("type ") {
        parse_type_def(&source[5..], file_id)
    } else if source.starts_with("callback ") {
        parse_callback(&source[9..], file_id)
    } else if source.starts_with("opaque ") {
        parse_opaque(&source[7..], file_id)
    } else if source.starts_with("typep ") {
        parse_typep(&source[6..], file_id)
    } else {
        Err("unknown typespec attribute".to_string())
    }
}

fn parse_callback(source: &str, file_id: SourceFileId) -> Result<Typespec, String> {
    let mut lexer = Lexer::new(source, file_id);

    let name_token = lexer.next_token().map_err(|e| format!("{:?}", e))?;
    let func_name = match name_token.value {
        chimera_lexer::TokenValue::Identifier(name) => name,
        _ => return Err("expected function name".to_string()),
    };

    consume_token(&mut lexer, TokenKind::OpenParen)?;
    let (args, saw_colon) = parse_type_args_with_colon(&mut lexer)?;
    if !saw_colon {
        // Need to consume the CloseParen and then DoubleColon
        consume_token(&mut lexer, TokenKind::CloseParen)?;
        consume_token(&mut lexer, TokenKind::DoubleColon)?;
    }
    let return_type = parse_type_from_tokens(&mut lexer)?;

    let arity = args.len() as u8;
    let mut table = chimera_term::AtomTable::new();
    let name_atom = table.intern(&func_name);

    Ok(Typespec::Callback {
        name: name_atom,
        arity,
        params: args
            .into_iter()
            .map(|t| TypespecArg::Anonymous(Box::new(t)))
            .collect(),
        return_type: Box::new(return_type),
        meta: SpecMeta::new(file_id),
    })
}

fn parse_opaque(source: &str, file_id: SourceFileId) -> Result<Typespec, String> {
    let mut lexer = Lexer::new(source, file_id);

    let name_token = lexer.next_token().map_err(|e| format!("{:?}", e))?;
    let type_name = match name_token.value {
        chimera_lexer::TokenValue::Identifier(name) => name,
        _ => return Err("expected type name".to_string()),
    };

    let mut table = chimera_term::AtomTable::new();
    let name_atom = table.intern(&type_name);

    consume_token(&mut lexer, TokenKind::DoubleColon)?;
    let type_def = parse_type_from_tokens(&mut lexer)?;

    Ok(Typespec::Opaque {
        name: name_atom,
        type_def: Box::new(type_def),
        meta: TypeMeta {
            opaque: true,
            ..TypeMeta::new(file_id)
        },
    })
}

fn parse_typep(source: &str, file_id: SourceFileId) -> Result<Typespec, String> {
    let mut lexer = Lexer::new(source, file_id);

    let name_token = lexer.next_token().map_err(|e| format!("{:?}", e))?;
    let type_name = match name_token.value {
        chimera_lexer::TokenValue::Identifier(name) => name,
        _ => return Err("expected type name".to_string()),
    };

    let mut table = chimera_term::AtomTable::new();
    let name_atom = table.intern(&type_name);

    consume_token(&mut lexer, TokenKind::DoubleColon)?;
    let type_def = parse_type_from_tokens(&mut lexer)?;

    Ok(Typespec::Typep {
        name: name_atom,
        type_def: Box::new(type_def),
        meta: TypeMeta::new(file_id),
    })
}
