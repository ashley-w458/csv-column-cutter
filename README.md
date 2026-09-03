# csvcut

A small CSV library and a CLI for picking columns out of a CSV file.

## Why

`split(',')` works on a CSV file right up until a field has a comma in it,
at which point it's quoted per RFC 4180 and naive splitting silently
produces the wrong number of columns:

```
name,notes
"Doe, Jane","said ""hi"" on Monday
and Tuesday"
```

That's two records, two fields each. A shell one-liner built on `cut -d,`
reads it as five. `csvcut` parses quoting, escaped quotes, and quoted
newlines properly, and exposes that parser as a library so other tools can
use it without pulling in a dependency.

## Building

```
cargo build --release
```

No external crates, so this only needs a stable Rust toolchain — nothing to
fetch.

## CLI usage

Fields are 1-indexed to match how people usually talk about "column 3".

```
# from a file
csvcut --fields 1,3 data.csv

# from stdin
cat data.csv | csvcut --fields 2

# passthrough (re-serializes with RFC 4180 quoting)
csvcut data.csv
```

`-` also means stdin, so it can sit in the middle of a pipeline that also
takes flags:

```
some-producer | csvcut --fields 1 -
```

## Library usage

```rust
use csvcut::CsvReader;

let input = "id,name\n1,Alice\n2,\"Bob, Jr.\"\n";
let mut rows = CsvReader::new(input.as_bytes());

assert_eq!(rows.next().unwrap().unwrap(), vec!["id", "name"]);
assert_eq!(rows.next().unwrap().unwrap(), vec!["1", "Alice"]);
assert_eq!(rows.next().unwrap().unwrap(), vec!["2", "Bob, Jr."]);
```

`CsvReader` takes anything that implements `std::io::Read`, so it works the
same way against a `File`, a `TcpStream`, or `Stdin`.

## Known limitations (first pass)

- No header-aware mode yet (`--fields name,email` instead of numeric
  indexes).
- Reads one byte at a time internally, which is correct but not fast on
  very large files.

## License

MIT, see LICENSE.
