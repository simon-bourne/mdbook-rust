use std::{cmp::min, collections::VecDeque, env, fmt::Display, io, process};

use anyhow::{bail, Result};
use indoc::eprintdoc;
use itertools::Itertools;
use mdbook::{book::Chapter, preprocess::CmdPreprocessor, BookItem};
use ra_ap_syntax::{
    ast::{self, HasModuleItem, HasName, Item},
    AstNode, AstToken, NodeOrToken, SourceFile, SyntaxKind, SyntaxNode, SyntaxToken,
};
use semver::{Version, VersionReq};

fn main() {
    let args = Vec::from_iter(env::args());
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [_exe, "supports", _] => process::exit(0),
        [_exe] => (),
        [exe, args @ ..] => usage(exe, args),
        args => usage("mdbook-rust", args),
    }

    if let Err(e) = preprocess() {
        eprintln!("{e}");
        process::exit(1);
    }
}

fn usage(exe: &str, args: &[&str]) {
    let args = args.join(" ");

    eprintdoc!(
        "
        Invalid arguments: {args}

        Usage:
            {exe}
            {exe} supports [OUTPUT_FORMAT]
        "
    );
    process::exit(1);
}

fn preprocess() -> Result<()> {
    let (ctx, mut book) = CmdPreprocessor::parse_input(io::stdin())?;

    let book_version = Version::parse(&ctx.mdbook_version)?;
    let version_req = VersionReq::parse(mdbook::MDBOOK_VERSION)?;

    if !version_req.matches(&book_version) {
        eprintln!(
            "Warning: MDBook version ({}) doesn't match plugin version ({})",
            ctx.mdbook_version,
            mdbook::MDBOOK_VERSION,
        );
    }

    let mut errors = Vec::new();

    book.for_each_mut(|item| match item {
        BookItem::Chapter(chapter) => {
            if let Err(e) = write_chapter(chapter) {
                errors.push(e);
            }
        }
        BookItem::Separator => (),
        BookItem::PartTitle(_) => (),
    });

    errors.into_iter().try_for_each(Err)?;
    serde_json::to_writer(io::stdout(), &book)?;

    Ok(())
}

fn write_chapter(chapter: &mut Chapter) -> Result<()> {
    if let Some(path) = &chapter.path {
        if path.extension() == Some("rs".as_ref()) {
            let source = parse_module(&chapter.content)?;

            for item in source.items() {
                if let Item::Fn(function) = item {
                    if is_named(&function, "body") {
                        if let Some(new_content) = write_function(function)? {
                            chapter.content = new_content;
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

fn write_function(function: ast::Fn) -> Result<Option<String>> {
    if let Some(stmts) = function.body().and_then(|body| body.stmt_list()) {
        let mut stmts: VecDeque<_> = stmts.syntax().children_with_tokens().collect();

        expect_kind(SyntaxKind::L_CURLY, stmts.pop_front())?;
        expect_kind(SyntaxKind::R_CURLY, stmts.pop_back())?;

        let body_text = stmts.iter().map(|s| s.to_string()).collect::<String>();
        let ws_prefixes = body_text.lines().filter_map(whitespace_prefix);
        let longest_prefix = longest_prefix(ws_prefixes);

        if stmts
            .front()
            .and_then(|node| node.as_token())
            .is_some_and(|token| ast::Whitespace::can_cast(token.kind()))
        {
            stmts.pop_front();
        }

        Ok(Some(write_body(stmts, longest_prefix)))
    } else {
        Ok(None)
    }
}

fn write_body(
    stmts: impl IntoIterator<Item = NodeOrToken<SyntaxNode, SyntaxToken>>,
    longest_prefix: &str,
) -> String {
    let mut whitespace = String::new();
    let mut in_code_block = false;
    let mut output = String::new();

    for node in stmts {
        match &node {
            NodeOrToken::Node(node) => {
                output.push_str(ensure_in_code_block(&mut in_code_block, &whitespace));
                output.push_str(&write_lines(node, longest_prefix));
                whitespace.clear();
            }
            NodeOrToken::Token(token) => {
                if let Some(comment) = ast::Comment::cast(token.clone()) {
                    if comment.is_doc() {
                        output.push_str(ensure_in_code_block(&mut in_code_block, &whitespace));
                        output.push_str(&write_lines(comment, longest_prefix));
                    } else {
                        output.push_str(ensure_in_markdown(&mut in_code_block, &whitespace));
                        output.push_str(&write_comment(comment, longest_prefix));
                    }

                    whitespace.clear();
                } else if ast::Whitespace::can_cast(token.kind()) {
                    whitespace =
                        "\n".repeat(token.to_string().chars().filter(|c| *c == '\n').count())
                } else {
                    output.push_str(&whitespace);
                    output.push_str(&write_lines(token, longest_prefix));
                    whitespace.clear();
                }
            }
        }
    }

    if in_code_block {
        output.push_str("\n```");
    }

    output.push('\n');

    output
}

fn write_lines(text: impl Display, prefix: &str) -> String {
    text.to_string()
        .split('\n')
        .map(|line| line.strip_prefix(prefix).unwrap_or(line))
        .join("\n")
}

fn write_comment(comment: ast::Comment, prefix: &str) -> String {
    let comment_suffix = &comment.text()[comment.prefix().len()..];
    let comment_text = match comment.kind().shape {
        ast::CommentShape::Line => comment_suffix,
        ast::CommentShape::Block => comment_suffix.strip_suffix("*/").unwrap_or(comment_suffix),
    };

    let mut lines = comment_text.split('\n');
    let mut output = String::new();

    if let Some(first_line) = lines.next() {
        output.push_str(first_line.strip_prefix(' ').unwrap_or(first_line));
    }

    for line in lines {
        output.push('\n');
        output.push_str(line.strip_prefix(prefix).unwrap_or(line))
    }

    output
}

fn parse_module(source_text: &str) -> Result<SourceFile> {
    let parsed = SourceFile::parse(source_text);
    let errors = parsed.errors();

    if !errors.is_empty() {
        bail!(errors.iter().join("\n"))
    }

    Ok(parsed.tree())
}

fn is_named(item: &impl HasName, name: &str) -> bool {
    item.name().is_some_and(|n| n.text().as_ref() == name)
}

fn longest_prefix<'a>(mut prefixes: impl Iterator<Item = &'a str>) -> &'a str {
    if let Some(mut longest_prefix) = prefixes.next() {
        for prefix in prefixes {
            // We can use `split_at` with `find_position` as our strings
            // only contain single byte chars (' ' or '\t').
            longest_prefix = longest_prefix
                .split_at(
                    longest_prefix
                        .chars()
                        .zip(prefix.chars())
                        .find_position(|(x, y)| x != y)
                        .map(|(position, _ch)| position)
                        .unwrap_or_else(|| min(longest_prefix.len(), prefix.len())),
                )
                .0;
        }

        longest_prefix
    } else {
        ""
    }
}

fn ensure_in_markdown<'a>(in_code_block: &mut bool, whitespace: &'a str) -> &'a str {
    let text = if *in_code_block {
        "\n```\n\n"
    } else {
        whitespace
    };

    *in_code_block = false;
    text
}

fn ensure_in_code_block<'a>(in_code_block: &mut bool, whitespace: &'a str) -> &'a str {
    let text = if *in_code_block {
        whitespace
    } else {
        "\n\n```rust\n"
    };

    *in_code_block = true;
    text
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
        bail!("Unexpected token")
    }
}

// TODO: Tests
