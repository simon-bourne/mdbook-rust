use std::{cmp::min, collections::VecDeque, fmt::Display};

use anyhow::{bail, Result};
use itertools::Itertools;
use ra_ap_syntax::{
    ast::{self, HasModuleItem, HasName, Item},
    AstNode, AstToken, NodeOrToken, SourceFile, SyntaxKind, SyntaxNode, SyntaxToken,
};

pub fn write_module(source_text: &str) -> Result<Option<String>> {
    let source = parse_module(source_text)?;

    for item in source.items() {
        if let Item::Fn(function) = item {
            if is_named(&function, "body") {
                if let Some(new_content) = write_function(function)? {
                    return Ok(Some(new_content));
                }
            }
        }
    }

    Ok(None)
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
        write_node_or_token(
            &mut output,
            &mut in_code_block,
            &mut whitespace,
            node,
            longest_prefix,
        );
    }

    if in_code_block {
        output.push_str("\n```");
    }

    output.push('\n');

    output
}

fn write_node_or_token(
    output: &mut String,
    in_code_block: &mut bool,
    whitespace: &mut String,
    node: NodeOrToken<SyntaxNode, SyntaxToken>,
    longest_prefix: &str,
) {
    match &node {
        NodeOrToken::Node(node) => {
            let mut children = node.children_with_tokens();

            // `Fn` nodes will have comments associated with them, rather than the parent.
            // We want to include these comments as markdown.
            for child in children.by_ref() {
                if child.kind() == SyntaxKind::COMMENT || child.kind() == SyntaxKind::WHITESPACE {
                    write_node_or_token(output, in_code_block, whitespace, child, longest_prefix);
                } else {
                    output.push_str(ensure_in_code_block(in_code_block, whitespace));
                    output.push_str(&write_lines(child, longest_prefix));
                    break;
                }
            }

            for child in children {
                output.push_str(&write_lines(child, longest_prefix));
            }

            whitespace.clear();
        }
        NodeOrToken::Token(token) => {
            write_token(output, in_code_block, whitespace, token, longest_prefix);
        }
    }
}

fn write_token(
    output: &mut String,
    in_code_block: &mut bool,
    whitespace: &mut String,
    token: &SyntaxToken,
    longest_prefix: &str,
) {
    if let Some(comment) = ast::Comment::cast(token.clone()) {
        if comment.is_doc() {
            output.push_str(ensure_in_code_block(in_code_block, &*whitespace));
            output.push_str(&write_lines(comment, longest_prefix));
        } else {
            output.push_str(ensure_in_markdown(in_code_block, &*whitespace));
            output.push_str(&write_comment(comment, longest_prefix));
        }

        whitespace.clear();
    } else if ast::Whitespace::can_cast(token.kind()) {
        *whitespace = "\n".repeat(token.to_string().chars().filter(|c| *c == '\n').count())
    } else {
        output.push_str(&*whitespace);
        output.push_str(&write_lines(token, longest_prefix));
        whitespace.clear();
    }
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
        "\n\n```rust,ignore\n"
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
