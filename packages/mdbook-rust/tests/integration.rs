use indoc::indoc;
use mdbook_rust::write_module;

fn check(source: &str, expected: &str) {
    assert_eq!(write_module(source).unwrap(), Some(expected.to_string()));
}

#[test]
fn basic() {
    check(
        indoc! {"
            fn body() {
                // # Title
                //
                // Body text
                let x = 1;
            }
        "},
        indoc! {"
            # Title

            Body text

            ```rust
            let x = 1;
            ```
        "},
    )
}
