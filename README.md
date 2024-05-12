# rustdoc-include

[![Crates.io](https://img.shields.io/crates/v/rustdoc-include.svg)](https://crates.io/crates/rustdoc-include)
[![Actions Status](https://github.com/frozenlib/rustdoc-include/workflows/CI/badge.svg)](https://github.com/frozenlib/rustdoc-include)

A utility for importing external Markdown files into `*.rs` file as doc comments.

## Motivation

In Rust, you can use `#[doc = include_str!("...")]` to import external markdown files in your doc comments. However, when using this method to import code examples, the location of errors in the code examples will not be correctly reported.

For example, consider the following case:

```rs
// src/lib.rs
#![doc = include_str!("doc.md")]
```

````md
# src/doc.md

```rust
invalid_func();
```
````

```txt
running 1 test
test src\lib.rs - (line 4) ... FAILED

failures:

---- src\lib.rs - (line 4) stdout ----
error[E0425]: cannot find function `invalid_func` in this scope
 --> src\lib.rs:5:1
  |
3 | invalid_func();
  | ^^^^^^^^^^^^ not found in this scope
```

The error location is reported as `src/lib.rs:5` instead of `src/doc.md:3`, and IDE jump functionality will not work correctly.

`rustdoc-include` solves this problem by copying the documentation from markdown files and embedding it into `*.rs` files. Furthermore, after initially importing the documentation, you can easily synchronize the `*.rs` files with the markdown files by running `rustdoc-include` again.

## Install

You can install this tool with `cargo install`.

```sh
cargo install rustdoc-include
```

## Usage

First, add `// #[include_doc("{filepath}", start)]` and `// #[include_doc("{filepath}", end)]` to the rust source code as follows

```rust :main.rs
// #[include_doc("file.md", start)]
// #[include_doc("file.md", end)]
fn main() {}
```

Next, create a markdown file with the path specified in the comment you just added.

```md :example.md
# Title

this is main function.
```

Run `rustdoc-include` with the `--root` option to specify the path to the directory containing rust source code and markdown file.

```sh
rustdoc-include --root ./
```

This tool will update the rust source code to look like this

```rust :main.rs
// #[include_doc("file.md", start)]
/// # Title
///
/// this is main function.
// #[include_doc("file.md", end)]
fn main() {}
```

This tool replaces the area enclosed by `// #[include_doc("{filepath}", start)]` and `// #[include_doc("{filepath}", end)]` with the contents of the markdown file. So if you rerun the same command after updating the markdown file, you can synchronize the doc comment in the source code with the external markdown file.

## Import doc comments for enclosing item

You can import an external file as a doc comment for the enclosing item by writing `// #![include_doc(...)]` instead of `// #[include_doc(...)]` as follows

```rust
// #![include_doc("file.md", start)]
// #![include_doc("file.md", end)]
```

### Use relative path

The path to the imported markdown file can also be specified relative to the source file.

```rust
// #[include_doc("../dir/file.md", start)]
// #[include_doc("../dir/file.md", end)]
```

However, it is not possible to import files outside the directory specified by the `--root` option.

## Restrict the scope of import

You can restrict the range to be imported by adding arguments to `start` and `end` for `// #[include_doc("{filepath}", start)]` and `// #[include_doc("{filepath}", end)]`.

### `start({line_number})`

Specifies the starting line number of the range to be imported.

```md
- line 1
- line 2
- line 3
```

```rs
// #[include_doc("file.md", start(2))]
// #[include_doc("file.md", end)]
fn main() {}
```

```rs
// #[include_doc("file.md", start(2))]
/// - line 2
/// - line 3
// #[include_doc("file.md", end)]
fn main() {}
```

### `start("{text}")`

Set the starting line of the range to be imported by specifying the text of that line.

### `end({line_number})`

Specifies the ending line number of the range to be imported.

### `end(-{line_number})`

Specifies the ending line of the range to be imported by specifying number of lines from the end.

### `end("{text}")`

Set the ending line of the range to be imported by specifying the text of that line.

## License

This project is dual licensed under Apache-2.0/MIT. See the two LICENSE-\* files for details.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
