pub fn body() {
    // # Chapter 1
    //
    // Any function called `body` will have it's body converted to Markdown:
    //
    // - Non-doc comments are interpreted as Markdown
    println!("Anything else is interpreted as Rust code");
    // - Any other top level items are ignored.
}

pub fn ignore_me() {
    // This will be ignored.
}
