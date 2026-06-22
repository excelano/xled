//! The xled CLI/REPL.
//!
//! Three modes, sed-shaped:
//!   xled '<script>' file.csv   one-shot: run the script, print the result to stdout
//!   … | xled '<script>'        one-shot over stdin (data piped in)
//!   xled file.csv              open the REPL on a file (when stdin is a terminal)

use clap::Parser as ClapParser;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::io::{self, IsTerminal, Read};
use std::process::exit;
use xled::{exec, io as xio, model::Buffer, parser, session::Session};

#[derive(ClapParser)]
#[command(name = "xled", version, about = "sed and awk for tabular data")]
struct Cli {
    /// command script — omit (give only a file, with a terminal stdin) to open the REPL
    script: Option<String>,
    /// input file (CSV/TSV); omit to read stdin
    file: Option<String>,
    /// field delimiter (defaults to ',', or tab for a .tsv file)
    #[arg(short, long)]
    delim: Option<char>,
    /// treat the first row as data, not a header. Use this when the real header is buried
    /// under a title block: row numbers then match the file, so you can `crop` to the table
    /// and promote the right row with `header` (otherwise row 1 is silently taken as the
    /// header and every address shifts up by one)
    #[arg(long)]
    no_header: bool,
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = real_main(cli) {
        eprintln!("{e}");
        exit(1);
    }
}

fn real_main(cli: Cli) -> xled::Result<()> {
    let has_header = !cli.no_header;
    let delim = cli.delim.map(|c| c as u8);
    let stdin_tty = io::stdin().is_terminal();

    match (cli.script, cli.file) {
        // explicit script + file → one-shot on the file
        (Some(script), Some(file)) => {
            let buf = xio::read_file(&file, delim, has_header)?;
            one_shot(buf, &script)
        }
        // single positional: a file to open (terminal) or a script over piped stdin
        (Some(arg), None) => {
            if stdin_tty {
                let buf = xio::read_file(&arg, delim, has_header)?;
                repl(buf, Some(arg))
            } else {
                let buf = read_stdin(delim, has_header)?;
                one_shot(buf, &arg)
            }
        }
        (None, _) => {
            eprintln!("usage: xled '<command>' <file>   |   <data> | xled '<command>'   |   xled <file>");
            exit(2);
        }
    }
}

fn read_stdin(delim: Option<u8>, has_header: bool) -> xled::Result<Buffer> {
    let mut data = String::new();
    io::stdin().read_to_string(&mut data)?;
    xio::read_str(&data, delim.unwrap_or(b','), has_header)
}

/// Run the script once. Print any `show` output; if the program only mutated the buffer,
/// stream the resulting table to stdout (sed-without-`-i` behaviour).
fn one_shot(mut buf: Buffer, script: &str) -> xled::Result<()> {
    let program = parser::parse_program(script)?;
    let out = exec::run(&mut buf, &program)?;
    if out.output.is_empty() {
        print!("{}", xio::serialize(&buf)?);
    } else {
        println!("{}", out.output.join("\n"));
    }
    // Notices to stderr: keep stdout a clean data stream for piping.
    for n in &out.notices {
        eprintln!("{n}");
    }
    Ok(())
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
