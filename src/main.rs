use csvcut::{quote_field, CsvReader};
use std::fs::File;
use std::io::{self, Read, Write};
use std::process::ExitCode;

/// How columns are chosen. Numeric selectors resolve immediately; names
/// have to wait for the header row to know which index they mean.
enum Selector {
    All,
    Indices(Vec<usize>),
    Names(Vec<String>),
}

struct Args {
    fields: Selector,
    path: Option<String>,
}

fn usage() -> String {
    "usage: csvcut [--fields N,N,...] [FILE]\n\
     \n\
     Selects columns from a CSV file, or from stdin if FILE is omitted or is \"-\".\n\
     Fields are 1-indexed. Without --fields, the input is passed through unchanged.\n\
     \n\
     A field may also be a column name taken from the header row, e.g.\n\
     --fields name,email. A field list is treated as names as soon as one\n\
     entry fails to parse as a number, so names cannot look like plain\n\
     integers.\n\
     \n\
     examples:\n  \
       csvcut --fields 1,3 data.csv\n  \
       csvcut --fields name,email data.csv\n  \
       cat data.csv | csvcut --fields 2"
        .to_string()
}

fn parse_fields(value: &str) -> Result<Selector, String> {
    let parts: Vec<&str> = value.split(',').map(str::trim).collect();

    let as_indices: Option<Vec<usize>> = parts
        .iter()
        .map(|part| part.parse::<usize>().ok())
        .collect();

    match as_indices {
        Some(numbers) => {
            let mut indices = Vec::with_capacity(numbers.len());
            for n in numbers {
                if n == 0 {
                    return Err("field numbers are 1-indexed, 0 is not valid".to_string());
                }
                indices.push(n - 1);
            }
            Ok(Selector::Indices(indices))
        }
        None => Ok(Selector::Names(parts.into_iter().map(String::from).collect())),
    }
}

fn parse_args(mut argv: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut fields = Selector::All;
    let mut path = None;

    while let Some(arg) = argv.next() {
        match arg.as_str() {
            "--fields" | "-f" => {
                let value = argv.next().ok_or("--fields needs a value")?;
                fields = parse_fields(&value)?;
            }
            "--help" | "-h" => return Err(usage()),
            _ if path.is_none() => path = Some(arg),
            _ => return Err(format!("unexpected argument: {arg}")),
        }
    }

    Ok(Args { fields, path })
}

fn select<'a>(record: &'a [String], indices: &Option<Vec<usize>>) -> Vec<&'a str> {
    match indices {
        None => record.iter().map(String::as_str).collect(),
        Some(indices) => indices
            .iter()
            .map(|&i| record.get(i).map(String::as_str).unwrap_or(""))
            .collect(),
    }
}

fn write_row<W: Write>(out: &mut W, row: &[&str]) -> io::Result<()> {
    let line = row.iter().map(|f| quote_field(f)).collect::<Vec<_>>().join(",");
    writeln!(out, "{line}")
}

/// Turns column names into indices by looking them up in the header row.
/// Also writes the header row (in the requested order) to `out`, since the
/// caller has already consumed it off the record iterator.
fn resolve_names<W: Write>(
    header: &[String],
    names: &[String],
    out: &mut W,
) -> io::Result<Vec<usize>> {
    let mut indices = Vec::with_capacity(names.len());
    for name in names {
        let i = header
            .iter()
            .position(|h| h == name)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("unknown column: {name}")))?;
        indices.push(i);
    }
    write_row(out, &select(header, &Some(indices.clone())))?;
    Ok(indices)
}

fn run<R: Read, W: Write>(input: R, out: &mut W, fields: &Selector) -> io::Result<()> {
    let mut records = CsvReader::new(input);

    let indices = match fields {
        Selector::All => None,
        Selector::Indices(idx) => Some(idx.clone()),
        Selector::Names(names) => {
            let header = match records.next() {
                Some(record) => record?,
                None => return Ok(()),
            };
            Some(resolve_names(&header, names, out)?)
        }
    };

    for record in records {
        let record = record?;
        write_row(out, &select(&record, &indices))?;
    }
    Ok(())
}

fn main() -> ExitCode {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::FAILURE;
        }
    };

    let stdout = io::stdout();
    let mut out = stdout.lock();

    let result = match args.path.as_deref() {
        None | Some("-") => run(io::stdin().lock(), &mut out, &args.fields),
        Some(path) => match File::open(path) {
            Ok(file) => run(file, &mut out, &args.fields),
            Err(e) => {
                eprintln!("csvcut: {path}: {e}");
                return ExitCode::FAILURE;
            }
        },
    };

    if let Err(e) = result {
        eprintln!("csvcut: {e}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_to_string(input: &str, fields: &Selector) -> String {
        let mut out = Vec::new();
        run(input.as_bytes(), &mut out, fields).unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn parse_fields_all_numeric_gives_indices() {
        match parse_fields("1,3").unwrap() {
            Selector::Indices(v) => assert_eq!(v, vec![0, 2]),
            _ => panic!("expected Indices"),
        }
    }

    #[test]
    fn parse_fields_zero_is_rejected() {
        assert!(parse_fields("0").is_err());
    }

    #[test]
    fn parse_fields_non_numeric_gives_names() {
        match parse_fields("name,email").unwrap() {
            Selector::Names(v) => assert_eq!(v, vec!["name".to_string(), "email".to_string()]),
            _ => panic!("expected Names"),
        }
    }

    #[test]
    fn selects_columns_by_name() {
        let out = run_to_string(
            "id,name,email\n1,Alice,alice@example.com\n2,Bob,bob@example.com\n",
            &Selector::Names(vec!["email".to_string(), "name".to_string()]),
        );
        assert_eq!(
            out,
            "email,name\nalice@example.com,Alice\nbob@example.com,Bob\n"
        );
    }

    #[test]
    fn unknown_column_name_is_an_error() {
        let mut out = Vec::new();
        let err = run(
            "id,name\n1,Alice\n".as_bytes(),
            &mut out,
            &Selector::Names(vec!["nope".to_string()]),
        )
        .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn no_input_with_name_selector_produces_nothing() {
        let out = run_to_string("", &Selector::Names(vec!["id".to_string()]));
        assert_eq!(out, "");
    }
}
