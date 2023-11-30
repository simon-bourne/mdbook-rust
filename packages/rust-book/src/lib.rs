use std::{
    cmp::min,
    collections::VecDeque,
    env::{self, VarError},
    error,
    fmt::{self, Display},
    fs::{self, File},
    io::{self, BufWriter, Write},
    path::PathBuf,
    result,
};

use itertools::Itertools;
use ra_ap_syntax::{
    ast::{self, HasModuleItem, HasName, HasVisibility, Item, VisibilityKind},
    AstNode, AstToken, NodeOrToken, SourceFile, SyntaxKind, SyntaxNode, SyntaxToken,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub struct Error(String);

impl Error {
    fn raise(err: impl Display) -> Result<()> {
        Err(Self(err.to_string()))
    }
}

trait ToError: error::Error {}

impl<T: ToError> From<T> for Error {
    fn from(value: T) -> Self {
        Self(value.to_string())
    }
}

impl ToError for VarError {}
impl ToError for io::Error {}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

pub type Result<T> = result::Result<T, Error>;

pub fn build() -> Result<()> {
    Book::new()?.build_modules(&[])
}

struct Book {
    cargo_manifest_dir: String,
    out_dir: String,
}

impl Book {
    fn new() -> Result<Self> {
        let cargo_manifest_dir = env::var("CARGO_MANIFEST_DIR")?;
        let out_dir = env::var("OUT_DIR")?;

        Ok(Self {
            cargo_manifest_dir,
            out_dir,
        })
    }

    fn build_modules(&self, path: &[&str]) -> Result<()> {
        // TODO: Divide this up into functions
        let path = if path.is_empty() {
            vec!["lib.rs"]
        } else {
            path.to_vec()
        };
        let filename = [&self.cargo_manifest_dir, "src"]
            .iter()
            .collect::<PathBuf>()
            .join(path.iter().collect::<PathBuf>());
        let source_text = fs::read_to_string(filename)?;
        let mut output_filename: PathBuf = [&self.out_dir, "rust-book"]
            .into_iter()
            .chain(path)
            .collect();
        output_filename.set_extension("md");
        fs::create_dir_all(output_filename.parent().unwrap())?;
        let mut output_file = BufWriter::new(File::create(output_filename)?);

        let parsed = SourceFile::parse(&source_text);

        let errors = parsed.errors();

        if !errors.is_empty() {
            Error::raise(errors.iter().join("\n"))?;
        }

        let source = parsed.tree();

        for item in source.items() {
            match item {
                Item::Fn(function) => {
                    if is_public(&function) && is_named(&function, "body") {
                        // TODO: Strip prefix
                        if let Some(stmts) = function.body().and_then(|body| body.stmt_list()) {
                            let mut stmts: VecDeque<_> =
                                stmts.syntax().children_with_tokens().collect();

                            expect_kind(SyntaxKind::L_CURLY, stmts.pop_front())?;
                            expect_kind(SyntaxKind::R_CURLY, stmts.pop_back())?;

                            // Find prefix
                            let body_text = stmts.iter().map(|s| s.to_string()).collect::<String>();
                            let mut ws_prefixes = body_text.lines().filter_map(whitespace_prefix);

                            let longest_prefix =
                                if let Some(mut longest_prefix) = ws_prefixes.next() {
                                    for prefix in ws_prefixes {
                                        // We can use `split_at` with `find_position` as our strings
                                        // only contain single byte chars (' ' or '\t').
                                        longest_prefix = longest_prefix
                                            .split_at(
                                                longest_prefix
                                                    .chars()
                                                    .zip(prefix.chars())
                                                    .find_position(|(x, y)| x != y)
                                                    .map(|(position, _ch)| position)
                                                    .unwrap_or_else(|| {
                                                        min(longest_prefix.len(), prefix.len())
                                                    }),
                                            )
                                            .0;
                                    }

                                    longest_prefix
                                } else {
                                    ""
                                };

                            if stmts
                                .front()
                                .and_then(|node| node.as_token())
                                .is_some_and(|token| ast::Whitespace::can_cast(token.kind()))
                            {
                                stmts.pop_front();
                            }

                            let mut whitespace = String::new();
                            let mut in_code_block = false;

                            for node in stmts {
                                match &node {
                                    NodeOrToken::Node(node) => {
                                        if !in_code_block {
                                            writeln!(&mut output_file, "\n\n```rust")?;
                                        } else {
                                            write!(&mut output_file, "{whitespace}")?;
                                        }

                                        write!(&mut output_file, "{node}")?;
                                        in_code_block = true;
                                        whitespace.clear();
                                    }
                                    NodeOrToken::Token(token) => {
                                        if let Some(comment) = ast::Comment::cast(token.clone()) {
                                            if comment.is_doc() {
                                                // TODO: Factor this out from here and Node writer
                                                if !in_code_block {
                                                    writeln!(&mut output_file, "\n\n```rust")?;
                                                } else {
                                                    write!(&mut output_file, "{whitespace}")?;
                                                }

                                                write!(&mut output_file, "{comment}")?;
                                                in_code_block = true;
                                            } else {
                                                let comment_suffix =
                                                    &comment.text()[comment.prefix().len()..];

                                                let comment_text = match comment.kind().shape {
                                                    ast::CommentShape::Line => comment_suffix,
                                                    ast::CommentShape::Block => comment_suffix
                                                        .strip_suffix("*/")
                                                        .unwrap_or(comment_suffix),
                                                }
                                                .trim_start();

                                                if in_code_block {
                                                    writeln!(&mut output_file, "\n```\n")?;
                                                } else {
                                                    write!(&mut output_file, "{whitespace}")?;
                                                }

                                                write!(&mut output_file, "{comment_text}")?;
                                                in_code_block = false;
                                            }

                                            whitespace.clear();
                                        } else if ast::Whitespace::can_cast(token.kind()) {
                                            let token_text = token.to_string();
                                            let (prefix, suffix) = token_text
                                                .rsplit_once(longest_prefix)
                                                .unwrap_or((&token_text, ""));
                                            whitespace = format!("{prefix}{suffix}");
                                        } else {
                                            write!(&mut output_file, "{whitespace}{token}")?;
                                            whitespace.clear();
                                        }
                                    }
                                }
                            }

                            if in_code_block {
                                writeln!(&mut output_file, "\n```")?;
                            }
                        }
                    }
                }
                Item::Module(module) => {
                    if is_public(&module) {
                        todo!()
                    }
                }
                _ => (),
            }
        }

        Ok(())
    }
}

fn whitespace_prefix(line: &str) -> Option<&str> {
    let non_ws = |c| c != ' ' && c != '\t';
    line.split_once(non_ws).map(|(prefix, _)| prefix)
}

fn expect_kind(
    expected: SyntaxKind,
    actual: Option<NodeOrToken<SyntaxNode, SyntaxToken>>,
) -> Result<()> {
    let actual_kind = actual
        .and_then(|last| last.into_token())
        .map(|token| token.kind());

    if Some(expected) == actual_kind {
        Ok(())
    } else {
        Error::raise("Unexpected token")
    }
}

fn is_public(item: &impl HasVisibility) -> bool {
    item.visibility()
        .is_some_and(|vis| matches!(vis.kind(), VisibilityKind::Pub))
}

fn is_named(item: &impl HasName, name: &str) -> bool {
    item.name().is_some_and(|n| n.text().as_ref() == name)
}
