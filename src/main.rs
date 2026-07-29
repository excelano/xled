//! The xled CLI/REPL.
//!
//! Three modes, sed-shaped:
//!   xled '<script>' file.csv   one-shot: run the script, print the result to stdout
//!   … | xled '<script>'        one-shot over stdin (data piped in)
//!   xled file.csv              open the REPL on a file (when stdin is a terminal)

use clap::Parser as ClapParser;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::process::exit;
use xled::{exec, io as xio, model::Buffer, parser, session::Session};

#[derive(ClapParser)]
#[command(name = "xled", version, about = "sed and awk for tabular data")]
struct Cli {
    /// command script — omit (give only a file, with a terminal stdin) to open the REPL
    script: Option<String>,
    /// input file (CSV/TSV); omit to read stdin
    file: Option<String>,
    /// field delimiter, `\t` for tab (defaults to ',', or tab for a .tsv file)
    #[arg(short, long, value_name = "CHAR", value_parser = xio::parse_delim)]
    delim: Option<u8>,
    /// read the command script from a file instead of the inline argument (like `sed -f`).
    /// The lone positional is then the input file: `xled -f batch.xled data.csv`
    #[arg(short = 'f', long = "file", value_name = "SCRIPTFILE")]
    script_file: Option<String>,
    /// edit the file in place instead of writing to stdout (like `sed -i`). Attach an
    /// optional backup suffix to keep the original: `-i.bak` / `--in-place=.bak`
    #[arg(
        short = 'i',
        long = "in-place",
        value_name = "SUFFIX",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = ""
    )]
    in_place: Option<String>,
    /// value-only output: print just the addressed cell values, one per line, with no header
    /// row and no CSV quoting — a single-cell read is then just the value, handy in shell
    /// pipelines and command substitutions. Applies to inspect scripts (a bare address / `show`)
    #[arg(long)]
    raw: bool,
    /// prefix each output row with its logical 1-based row number, so a pipe keeps xled's own
    /// row addressing even across cells with embedded newlines (which line-based tools miscount).
    /// Applies to inspect scripts (a bare address / `show`)
    #[arg(long)]
    number: bool,
    /// treat the first row as data, not a header. Use this when the real header is buried
    /// under a title block: row numbers then match the file, so you can `crop` to the table
    /// and promote the right row with `header` (otherwise row 1 is silently taken as the
    /// header and every address shifts up by one)
    #[arg(long)]
    no_header: bool,
}

fn main() {
    let cli = Cli::parse_from(normalize_in_place(std::env::args()));
    if let Err(e) = real_main(cli) {
        eprintln!("{e}");
        exit(1);
    }
}

/// sed attaches the in-place backup suffix to the flag (`-i.bak`); clap models an optional
/// value with `=`, so rewrite the short attached form `-i<suffix>` to `-i=<suffix>` before
/// parsing. Bare `-i` (no backup) and the long `--in-place[=suffix]` form pass through as-is.
fn normalize_in_place<I: IntoIterator<Item = String>>(args: I) -> Vec<String> {
    args.into_iter()
        .map(|a| {
            if a.len() > 2 && a.starts_with("-i") && !a.starts_with("-i=") {
                format!("-i={}", &a[2..])
            } else {
                a
            }
        })
        .collect()
}

fn real_main(cli: Cli) -> xled::Result<()> {
    let has_header = !cli.no_header;
    let delim = cli.delim;
    let stdin_tty = io::stdin().is_terminal();
    let in_place = cli.in_place.as_deref();
    let opts = exec::RenderOpts { raw: cli.raw, number: cli.number };

    // -f/--file reads the script from a file; the lone positional (if any) is then the input
    // file, so the script-vs-file polymorphism of the bare single-positional form disappears.
    if let Some(path) = &cli.script_file {
        if cli.file.is_some() {
            eprintln!("-f reads the script from a file — pass only the input file, not an inline script");
            exit(2);
        }
        let script = read_script_file(path)?;
        return match cli.script {
            Some(file) => {
                let buf = xio::read_file(&file, delim, has_header)?;
                emit(render(buf, &script, opts)?, in_place, Some(&file))
            }
            None => {
                if stdin_tty {
                    eprintln!("-f needs data: give an input file or pipe data in");
                    exit(2);
                }
                emit(render(read_stdin(delim, has_header)?, &script, opts)?, in_place, None)
            }
        };
    }

    match (cli.script, cli.file) {
        // explicit script + file → one-shot on the file
        (Some(script), Some(file)) => {
            let buf = xio::read_file(&file, delim, has_header)?;
            emit(render(buf, &script, opts)?, in_place, Some(&file))
        }
        // single positional: a file to open (terminal) or a script over piped stdin
        (Some(arg), None) => {
            if stdin_tty {
                if in_place.is_some() {
                    eprintln!("-i edits a one-shot result in place — it has no effect on the REPL (use `write`)");
                    exit(2);
                }
                let buf = xio::read_file(&arg, delim, has_header)?;
                repl(buf, Some(arg))
            } else {
                let buf = read_stdin(delim, has_header)?;
                emit(render(buf, &arg, opts)?, in_place, None)
            }
        }
        (None, _) => {
            eprintln!("usage: xled '<command>' <file>   |   <data> | xled '<command>'   |   xled <file>");
            exit(2);
        }
    }
}

/// Read the command script from a file (for `-f`/`--file`).
fn read_script_file(path: &str) -> xled::Result<String> {
    fs::read_to_string(path).map_err(|e| xled::XledError::Io(format!("{path}: {e}")))
}

fn read_stdin(delim: Option<u8>, has_header: bool) -> xled::Result<Buffer> {
    let mut data = String::new();
    io::stdin().read_to_string(&mut data)?;
    xio::read_str(&data, delim.unwrap_or(b','), has_header)
}

/// The result of a one-shot run, ready to hand to a destination.
struct Rendered {
    text: String,
    /// true when the script produced `show`/inspect output rather than mutating the table —
    /// such a result must not be written back over the source file by `-i`.
    is_query: bool,
}

/// Run the script once. If the program only mutated the buffer, the rendered text is the
/// serialized table (sed-without-`-i` behaviour); if it produced `show` output, that is the
/// text and `is_query` is set. Notices always go to stderr so the data stream stays clean.
fn render(mut buf: Buffer, script: &str, opts: exec::RenderOpts) -> xled::Result<Rendered> {
    let program = parser::parse_program(script)?;
    let out = exec::run_with(&mut buf, &program, opts)?;
    for n in &out.notices {
        eprintln!("{n}");
    }
    if out.output.is_empty() {
        if opts.raw || opts.number {
            eprintln!(
                "note: --raw/--number format inspect output (a bare address or `show`); this \
                 script writes the whole table, so they had no effect"
            );
        }
        Ok(Rendered { text: xio::serialize(&buf)?, is_query: false })
    } else {
        Ok(Rendered { text: format!("{}\n", out.output.join("\n")), is_query: true })
    }
}

/// Send a one-shot result to its destination: stdout by default, or — when `-i`/`--in-place`
/// is set and a source file exists — back to that file, first copying it to `<file><suffix>`
/// when a backup suffix was given. In-place refuses an inspect-only result (it would overwrite
/// the file with query output) and refuses piped stdin (there is no file to edit).
fn emit(r: Rendered, in_place: Option<&str>, file: Option<&str>) -> xled::Result<()> {
    match (in_place, file) {
        (Some(suffix), Some(path)) => {
            if r.is_query {
                eprintln!(
                    "-i edits the table in place, but this script only inspects it — drop -i to \
                     print the result, or use a command that changes cells"
                );
                exit(2);
            }
            if !suffix.is_empty() {
                fs::copy(path, format!("{path}{suffix}"))
                    .map_err(|e| xled::XledError::Io(e.to_string()))?;
            }
            fs::write(path, &r.text).map_err(|e| xled::XledError::Io(e.to_string()))
        }
        (Some(_), None) => {
            eprintln!("-i edits a file in place — it needs a file argument, not piped stdin");
            exit(2);
        }
        (None, _) => write_stdout(&r.text),
    }
}

/// Write the data stream to stdout, exiting quietly when the downstream reader has
/// closed the pipe early (e.g. `xled … | head`). The `print!`/`println!` macros unwrap
/// the write and panic on `EPIPE`; matching `cat`/`grep`, a broken pipe is a clean stop.
fn write_stdout(s: &str) -> xled::Result<()> {
    let mut out = io::stdout().lock();
    match out.write_all(s.as_bytes()).and_then(|()| out.flush()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => exit(0),
        Err(e) => Err(xled::XledError::Io(e.to_string())),
    }
}

/// The REPL: a live buffer edited in place, saved deliberately. Word commands —
/// `preview <cmd>`, `undo`, `write [path]`, `help`, `quit` — sit alongside ordinary
/// `address command` lines. Nothing is written to disk until `write`.
fn repl(buf: Buffer, source: Option<String>) -> xled::Result<()> {
    let mut sess = Session::new(buf, source);
    let mut rl = DefaultEditor::new().map_err(|e| xled::XledError::Io(e.to_string()))?;

    loop {
        match rl.readline("xled> ") {
            Ok(line) => {
                let t = line.trim();
                if t.is_empty() {
                    continue;
                }
                let _ = rl.add_history_entry(t);
                let (word, rest) = split_word(t);
                match word {
                    "quit" | "q" => {
                        if sess.dirty {
                            eprintln!("unsaved changes — `write` to save, or `quit!` to discard");
                        } else {
                            break;
                        }
                    }
                    "quit!" | "q!" => break,
                    "help" => print_help(),
                    "undo" => {
                        if sess.undo() {
                            println!("reverted last change");
                        } else {
                            println!("nothing to undo");
                        }
                    }
                    "write" => {
                        let path = if rest.is_empty() { None } else { Some(rest) };
                        match sess.save(path) {
                            Ok(p) => println!("wrote {} rows to {p}", sess.buf.nrows()),
                            Err(e) => eprintln!("{e}"),
                        }
                    }
                    "preview" => match parser::parse_program(rest).and_then(|p| sess.preview(&p)) {
                        Ok(out) => println!("{out}"),
                        Err(e) => eprintln!("{e}"),
                    },
                    _ => match parser::parse_program(t).and_then(|p| sess.run(&p)) {
                        Ok(out) => {
                            for o in out.output {
                                println!("{o}");
                            }
                            for n in out.notices {
                                eprintln!("{n}");
                            }
                        }
                        Err(e) => eprintln!("{e}"),
                    },
                }
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
                if sess.dirty {
                    eprintln!("unsaved changes — `write` to save, or `quit!` to discard");
                }
                break;
            }
            Err(e) => return Err(xled::XledError::Io(e.to_string())),
        }
    }
    Ok(())
}

/// Split a line into its first word and the remainder (trimmed).
fn split_word(line: &str) -> (&str, &str) {
    match line.split_once(char::is_whitespace) {
        Some((w, rest)) => (w, rest.trim()),
        None => (line, ""),
    }
}

fn print_help() {
    println!(
        "xled — sed and awk for tabular data\n\
         \n\
         address command   edit:    [price] s/\\$//g · /active/i [status] = \"done\" · 3 del\n\
         address           inspect: [price] · 2:4 · B2:C3 · /tools/\n\
         \n\
         preview <cmd>      show what a command would do, without committing\n\
         undo               revert the last change\n\
         write [path]       save the buffer (to the source file, or a given path)\n\
         help               this text\n\
         quit / quit!       exit (quit! discards unsaved changes)"
    );
}
