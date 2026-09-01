use csvcut::CsvReader;
use std::fs::File;
use std::io::{self, Read, Write};
use std::process::ExitCode;

struct Args {
    fields: Option<Vec<usize>>,
    path: Option<String>,
}

fn usage() -> String {
    "usage: csvcut [--fields N,N,...] [FILE]\n\
     \n\
     Selects columns from a CSV file, or from stdin if FILE is omitted or is \"-\".\n\
     Fields are 1-indexed. Without --fields, the input is passed through unchanged.\n\
     \n\
     examples:\n  \
       csvcut --fields 1,3 data.csv\n  \
       cat data.csv | csvcut --fields 2"
        .to_string()
}

fn parse_args(mut argv: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut fields = None;
    let mut path = None;

    while let Some(arg) = argv.next() {
        match arg.as_str() {
            "--fields" | "-f" => {
                let value = argv.next().ok_or("--fields needs a value")?;
                let mut parsed = Vec::new();
                for part in value.split(',') {
                    let n: usize = part
                        .trim()
                        .parse()
                        .map_err(|_| format!("not a valid field number: {part}"))?;
                    if n == 0 {
                        return Err("field numbers are 1-indexed, 0 is not valid".to_string());
                    }
                    parsed.push(n - 1);
                }
                fields = Some(parsed);
            }
            "--help" | "-h" => return Err(usage()),
            _ if path.is_none() => path = Some(arg),
            _ => return Err(format!("unexpected argument: {arg}")),
        }
    }

    Ok(Args { fields, path })
}

fn select<'a>(record: &'a [String], fields: &Option<Vec<usize>>) -> Vec<&'a str> {
    match fields {
        None => record.iter().map(String::as_str).collect(),
        Some(indices) => indices
            .iter()
            .map(|&i| record.get(i).map(String::as_str).unwrap_or(""))
            .collect(),
    }
}

fn run<R: Read, W: Write>(input: R, out: &mut W, fields: &Option<Vec<usize>>) -> io::Result<()> {
    for record in CsvReader::new(input) {
        let record = record?;
        let row = select(&record, fields);
        writeln!(out, "{}", row.join(","))?;
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
