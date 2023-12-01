# MDBook Rust

[![tests](https://github.com/simon-bourne/mdbook-rust/actions/workflows/tests.yml/badge.svg)](https://github.com/simon-bourne/mdbook-rust/actions/workflows/tests.yml)
[![crates.io](https://img.shields.io/crates/v/mdbook-rust.svg)](https://crates.io/cratemdbook-rustok)
[![Documentation](https://docs.rs/mdbook-rust/badge.svg)](https://docs.rs/mdbook-rust)
[![MIT/Apache-2 licensed](https://img.shields.io/crates/l/mdbook-rust)](./LICENSE-APACHE)

Enhanced Rust support for MDBook.

- Any chapters written as Rust source files will be translated to Markdown, allowing you to test your book as a Rust crate. It's like literate programming in reverse (illiterate programming).
- Everything else is left alone.

This Rust code:

```rust
fn body() {
    // # Heading
    //
    // Paragraph text.
    some_code();
}
```

will be converted to:

````markdown
# Heading

Paragraph text.

```rust,ignore
some_code();
```
````

See [`examples/book`](https://github.com/simon-bourne/mdbook-rust/tree/main/examples/book) for a complete example.
