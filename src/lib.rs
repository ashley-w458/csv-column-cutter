//! A small, dependency-free CSV parser.
//!
//! `str::split(',')` is enough until a field contains a comma, a quote, or a
//! newline. RFC 4180 handles that by quoting such fields with `"` and
//! doubling any literal `"` inside them. This module implements just enough
//! of that spec to read real-world CSV correctly, one record at a time, from
//! any `Read` source (a file, a pipe, stdin).

use std::io::{self, Bytes, Read};
use std::iter::Peekable;

/// Reads CSV records from an underlying byte stream.
///
/// Each call to `next()` returns one record as a `Vec<String>`, so a large
/// input can be processed without holding the whole thing in memory.
pub struct CsvReader<R: Read> {
    bytes: Peekable<Bytes<R>>,
    done: bool,
}

impl<R: Read> CsvReader<R> {
    pub fn new(inner: R) -> Self {
        CsvReader {
            bytes: inner.bytes().peekable(),
            done: false,
        }
    }
}

fn finish_field(buf: Vec<u8>) -> io::Result<String> {
    String::from_utf8(buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

impl<R: Read> Iterator for CsvReader<R> {
    type Item = io::Result<Vec<String>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let mut fields = Vec::new();
        let mut field = Vec::new();
        let mut in_quotes = false;
        let mut read_any = false;

        loop {
            let byte = match self.bytes.next() {
                Some(Ok(b)) => b,
                Some(Err(e)) => return Some(Err(e)),
                None => {
                    self.done = true;
                    if !read_any {
                        return None;
                    }
                    return Some(finish_field(field).map(|f| {
                        fields.push(f);
                        fields
                    }));
                }
            };
            read_any = true;

            if in_quotes {
                if byte == b'"' {
                    if let Some(Ok(b'"')) = self.bytes.peek() {
                        field.push(b'"');
                        self.bytes.next();
                    } else {
                        in_quotes = false;
                    }
                } else {
                    field.push(byte);
                }
                continue;
            }

            match byte {
                b'"' if field.is_empty() => in_quotes = true,
                b',' => {
                    let s = match finish_field(std::mem::take(&mut field)) {
                        Ok(s) => s,
                        Err(e) => return Some(Err(e)),
                    };
                    fields.push(s);
                }
                b'\n' => {
                    let s = match finish_field(field) {
                        Ok(s) => s,
                        Err(e) => return Some(Err(e)),
                    };
                    fields.push(s);
                    return Some(Ok(fields));
                }
                b'\r' => {
                    if let Some(Ok(b'\n')) = self.bytes.peek() {
                        self.bytes.next();
                    }
                    let s = match finish_field(field) {
                        Ok(s) => s,
                        Err(e) => return Some(Err(e)),
                    };
                    fields.push(s);
                    return Some(Ok(fields));
                }
                _ => field.push(byte),
            }
        }
    }
}

/// Quotes a field for CSV output, per RFC 4180.
///
/// A field is wrapped in `"..."` only if it contains a comma, a quote, or a
/// newline, since those are the characters that would otherwise change how
/// the field is split back apart. Any literal `"` inside is doubled.
pub fn quote_field(field: &str) -> String {
    if !field.contains(['"', ',', '\n', '\r']) {
        return field.to_string();
    }

    let mut out = String::with_capacity(field.len() + 2);
    out.push('"');
    for c in field.chars() {
        if c == '"' {
            out.push('"');
        }
        out.push(c);
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn records(input: &str) -> Vec<Vec<String>> {
        CsvReader::new(input.as_bytes())
            .collect::<io::Result<Vec<_>>>()
            .unwrap()
    }

    #[test]
    fn splits_plain_fields() {
        assert_eq!(records("a,b,c\n"), vec![vec!["a", "b", "c"]]);
    }

    #[test]
    fn keeps_commas_inside_quotes() {
        assert_eq!(
            records("\"a,b\",c\n"),
            vec![vec!["a,b".to_string(), "c".to_string()]]
        );
    }

    #[test]
    fn unescapes_doubled_quotes() {
        assert_eq!(
            records("\"say \"\"hi\"\"\"\n"),
            vec![vec!["say \"hi\"".to_string()]]
        );
    }

    #[test]
    fn handles_newline_inside_quoted_field() {
        assert_eq!(
            records("\"line1\nline2\",b\n"),
            vec![vec!["line1\nline2".to_string(), "b".to_string()]]
        );
    }

    #[test]
    fn last_record_without_trailing_newline() {
        assert_eq!(records("a,b"), vec![vec!["a", "b"]]);
    }

    #[test]
    fn empty_input_has_no_records() {
        assert_eq!(records(""), Vec::<Vec<String>>::new());
    }

    #[test]
    fn quote_field_leaves_plain_text_alone() {
        assert_eq!(quote_field("plain"), "plain");
        assert_eq!(quote_field(""), "");
    }

    #[test]
    fn quote_field_wraps_commas() {
        assert_eq!(quote_field("a,b"), "\"a,b\"");
    }

    #[test]
    fn quote_field_doubles_embedded_quotes() {
        assert_eq!(quote_field("say \"hi\""), "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn quote_field_wraps_embedded_newlines() {
        assert_eq!(quote_field("line1\nline2"), "\"line1\nline2\"");
        assert_eq!(quote_field("a\rb"), "\"a\rb\"");
    }
}
