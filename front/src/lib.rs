#![deny(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(
    clippy::option_map_unit_fn,
    clippy::cast_precision_loss,
    clippy::too_many_lines,
    clippy::module_name_repetitions,
    clippy::missing_errors_doc
)]

//! Compiler for the yex language
mod compiler;
mod error;
mod lexer;
mod parser;
mod tokens;

use compiler::Compiler;
pub use error::ParseError;

use error::ParseResult;
use lexer::Lexer;
use parser::Parser;
use vm::{Bytecode, Value};

/// Parses a given string into an AST
pub fn parse<T: Into<String>>(str: T) -> ParseResult<(Bytecode, Vec<Value>)> {
    let lexer = Lexer::new(str);
    let parser = Parser::new(lexer)?;
    let ast = parser.parse()?;

    let compiler = Compiler::new();
    Ok(compiler.compile_stmts(&ast))
}

/// Parses the given string in a single expression
pub fn parse_expr<T: Into<String>>(str: T) -> ParseResult<(Bytecode, Vec<Value>)> {
    let lexer = Lexer::new(str);

    let parser = Parser::new(lexer)?;
    let ast = parser.parse_expr()?;

    let compiler = Compiler::new();
    Ok(compiler.compile_expr(&ast))
}
