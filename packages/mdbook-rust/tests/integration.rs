use indoc::indoc;
use mdbook_rust::write_module;

fn check(source: &str, expected: &str) {
    assert_eq!(write_module(source).unwrap(), Some(expected.to_string()));
}

#[test]
fn empty() {
    assert!(write_module("").unwrap().is_none());
}

#[test]
fn ignored() {
    assert!(write_module(indoc! {"
        fn ingnore_me() {}
    "})
    .unwrap()
    .is_none());
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

            ```rust,ignore
            let x = 1;
            ```
        "},
    )
}

#[test]
fn empty_body() {
    check("fn body() {}", "\n")
}

#[test]
fn line_comment_indent() {
    check(
        indoc! {"
            fn body() {
                //# No space after comment marker
                //
                // - Item 1
                //- Item 2 with no space after comment marker
                //   - Sub-item
            }
        "},
        indoc! {"
            # No space after comment marker
            
            - Item 1
            - Item 2 with no space after comment marker
              - Sub-item
        "},
    )
}

#[test]
fn block_comment_indent() {
    check(
        indoc! {"
            fn body() {
                /*
                # Heading
                
                - Item 1
                - Item 2
                  - Sub-item
                */
            }
        "},
        indoc! {"

            # Heading
            
            - Item 1
            - Item 2
              - Sub-item

        "},
    )
}

#[test]
fn code_only() {
    check(
        indoc! {"
            fn body() {
                let x = 1;
                let y = 1;
            }
        "},
        indoc! {"


            ```rust,ignore
            let x = 1;
            let y = 1;
            ```
        "},
    )
}

#[test]
fn code_spacing() {
    check(
        indoc! {"
            fn body() {
                let x = 1;

                let y = 1;
            }
        "},
        indoc! {"


            ```rust,ignore
            let x = 1;

            let y = 1;
            ```
        "},
    )
}

#[test]
fn local_function() {
    check(
        indoc! {"
            fn body() {
                // Lorem ipsum
                fn local() {}
            }
        "},
        indoc! {"
            Lorem ipsum

            ```rust,ignore
            fn local() {}
            ```
        "},
    )
}
