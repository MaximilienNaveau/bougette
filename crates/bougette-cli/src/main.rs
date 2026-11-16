//! `bougette`: the command-line client of M0, over `bougette-core` and
//! `bougette-store`. It reads words, calls the domain and prints the
//! result; every computation is in the core (CLAUDE.md, §4).

// `mod commands;` compiles src/commands.rs as a module of this crate, like
// adding a .cpp file to the target, but with its own namespace.
mod commands;

use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;

use bougette_store::Store;

use crate::commands::{Context, USAGE};

// `ExitCode` is what `main` hands back to the shell, like the `int` of
// `int main()`.
fn main() -> ExitCode {
    // `std::env::args()` iterates over argv; `skip(1)` drops the program
    // name. `collect` gathers the rest into a Vec<String>.
    match try_main(std::env::args().skip(1).collect()) {
        Ok(output) => {
            print!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            // `eprintln!` writes to stderr.
            eprintln!("bougette: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Everything `main` does, with errors as text so `?` can stop anywhere.
fn try_main(arguments: Vec<String>) -> Result<String, String> {
    let (words, options) = commands::parse_arguments(arguments)?;
    if words.is_empty() || words == ["help"] {
        return Ok(USAGE.to_string());
    }
    let path = match options.get("db") {
        Some(path) => PathBuf::from(path),
        None => default_database()?,
    };
    // Create the folder holding the database if needed (`mkdir -p`).
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)
            .map_err(|error| format!("{}: {error}", folder.display()))?;
    }
    let mut store = Store::open(&path).map_err(|error| format!("{}: {error:?}", path.display()))?;
    let today = match options.get("today") {
        Some(day) => commands::parse_date(day)?,
        // Now, in the system's time zone, reduced to a calendar date.
        None => jiff::Zoned::now().date(),
    };
    // Colours only on a terminal, and never if NO_COLOR is set
    // (no-color.org).
    let colour = std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    // The words as `&str` views of the Strings, which the commands match
    // with slice patterns. `String::as_str` is a function used as a
    // closure.
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    let mut context = Context {
        store: &mut store,
        today,
        colour,
    };
    commands::run(&words, &options, &mut context)
}

/// `$XDG_DATA_HOME/bougette/bougette.sqlite`, with `XDG_DATA_HOME`
/// defaulting to `~/.local/share` (the XDG base directory specification).
fn default_database() -> Result<PathBuf, String> {
    // `var_os` reads an environment variable as an OsString, which, unlike
    // String, may hold bytes that aren't UTF-8, as file names can.
    let data = match std::env::var_os("XDG_DATA_HOME") {
        // A match guard: an empty variable counts as unset.
        Some(folder) if !folder.is_empty() => PathBuf::from(folder),
        _ => PathBuf::from(std::env::var_os("HOME").ok_or("HOME is not set: use --db FILE")?)
            .join(".local/share"),
    };
    Ok(data.join("bougette").join("bougette.sqlite"))
}
