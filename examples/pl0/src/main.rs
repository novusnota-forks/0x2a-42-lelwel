use codespan_reporting::diagnostic::Severity;
use codespan_reporting::files::SimpleFile;
use codespan_reporting::term::termcolor::{ColorChoice, StandardStream};
use codespan_reporting::term::{self, Config};
use logos::Logos;
use parser::*;

mod parser;

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        std::process::exit(1);
    }
    let contents = std::fs::read_to_string(&args[1])?;

    let mut tokens = TokenStream::new(Token::lexer(&contents));
    let mut diags = vec![];
    Parser::parse(&mut tokens, &mut diags);

    let writer = StandardStream::stderr(ColorChoice::Auto);
    let config = Config::default();
    let file = SimpleFile::new(&args[1], &contents);
    for diag in diags.iter() {
        term::emit(&mut writer.lock(), &config, &file, &diag).unwrap();
    }
    if diags.iter().any(|d| d.severity == Severity::Error) {
        std::process::exit(1);
    }
    Ok(())
}
