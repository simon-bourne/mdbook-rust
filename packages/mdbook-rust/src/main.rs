use std::{env, io, process};

use anyhow::Result;
use indoc::eprintdoc;
use mdbook::{book::Chapter, preprocess::CmdPreprocessor, BookItem};
use mdbook_rust::write_module;
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
            if let Some(new_content) = write_module(&chapter.content)? {
                chapter.content = new_content;
            }
        }
    }

    Ok(())
}
