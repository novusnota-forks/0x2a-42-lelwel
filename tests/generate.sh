#!/bin/bash

output=frontend.rs

cat << EOF > $output
// This file was generated by the generate.sh script.
// DO NOT EDIT THIS FILE MANUALLY!

use lelwel::frontend::ast::*;
use lelwel::frontend::diag::*;
use lelwel::frontend::lexer::*;
use lelwel::frontend::sema::*;

macro_rules! check_next {
    (\$diags: ident, \$message: expr) => {
        assert_eq!(\$diags.next().unwrap().to_string(), \$message);
    };
}
macro_rules! check_empty {
    (\$diags: ident) => {
        if !\$diags.next().is_none() {
            panic!("too many {}", stringify!(\$diags))
        }
    };
}

fn gen_diag(input: &str) -> std::io::Result<Diag> {
    let path = std::path::Path::new(input);
    let contents = std::fs::read_to_string(path)?;
    let mut diag = Diag::new(&input, 100);
    let mut lexer = Lexer::new(contents, false);
    let ast = Ast::new(&mut lexer, &mut diag);
    if let Some(root) = ast.root() {
        SemanticPass::run(root, &mut diag);
    }
    for (range, msg) in lexer.error_iter() {
        diag.error(Code::ParserError(msg), *range);
    }
    Ok(diag)
}
EOF

for path in frontend/*.llw; do
  file=${path##*/}
  cat << EOF >> $output

#[test]
#[rustfmt::skip]
fn ${file%.llw}() {
    let diag = gen_diag("tests/$path").unwrap();
    let mut errors = diag.error_iter();
    let mut warnings = diag.warning_iter();
EOF
  diag=$(llw -c "$path" 2>&1 > /dev/null)
  echo >> $output
  echo "$diag" | grep 'error: ' | while read -r line ; do
    echo "$line" | sed 's/^[^:]*: \(.*\)/    check_next!(errors, "tests\/\1");/' >> $output
  done
  echo '    check_empty!(errors);' >> $output
  echo >> $output
  echo "$diag" | grep 'warning: ' | while read -r line ; do
    echo "$line" | sed 's/^[^:]*: \(.*\)/    check_next!(warnings, "tests\/\1");/' >> $output
  done
  echo '    check_empty!(warnings);' >> $output
  echo '}' >> $output
done

