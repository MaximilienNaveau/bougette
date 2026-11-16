//! `bougette-server`: a device that is always online (ADR 0001). It keeps
//! the messages the devices send it and passes them on to the others, so
//! two devices sync without ever being online together. It never reads
//! the budgets.
//!
//! Until members and invitations (M2), it serves only the devices given
//! with `--allow`.

use std::collections::HashMap;
use std::path::Path;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};

use bougette_store::Store;
use bougette_sync::net;

const USAGE: &str = "\
Usage: bougette-server --db FILE --port PORT --allow ID[,ID...]

Keeps the changes of the allowed devices and passes them on. Each device
gives its identifier with `bougette id`, then syncs with the address this
prints, using `bougette sync ADDRESS`.
";

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        // `run` only returns on an error: serving goes on forever.
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("bougette-server: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: Vec<String>) -> Result<(), String> {
    let options = parse(arguments)?;
    // A closure reading a required option.
    let required = |name: &str| {
        options
            .get(name)
            .ok_or_else(|| format!("missing --{name}\n\n{USAGE}"))
    };
    let mut store = Store::open(Path::new(required("db")?))
        .map_err(|error| format!("cannot open the database: {error:?}"))?;
    let port: u16 = required("port")?
        .parse()
        .map_err(|_| "--port takes a number from 1 to 65535".to_string())?;
    let allowed = required("allow")?
        .split(',')
        .map(net::parse_id)
        .collect::<Result<Vec<_>, _>>()?;
    let key = net::secret_key(&mut store).map_err(|error| format!("{error:?}"))?;

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("cannot start the async runtime: {error}"))?;
    runtime.block_on(async {
        let endpoint = net::endpoint(key, port)
            .await
            .map_err(|error| format!("cannot listen: {error:?}"))?;
        println!("Serving {} device(s). Devices sync with:", allowed.len());
        println!("  bougette sync {}", net::address(&endpoint));
        net::serve(endpoint, Arc::new(Mutex::new(store)), allowed, report).await;
        Ok(())
    })
}

/// Reads `--name value` pairs; anything else is an error.
fn parse(arguments: Vec<String>) -> Result<HashMap<String, String>, String> {
    let mut options = HashMap::new();
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        let name = match argument.strip_prefix("--") {
            Some(name @ ("db" | "port" | "allow")) => name.to_string(),
            _ => return Err(format!("unexpected {argument}\n\n{USAGE}")),
        };
        let value = arguments
            .next()
            .ok_or_else(|| format!("--{name} needs a value"))?;
        options.insert(name, value);
    }
    Ok(options)
}

/// One line per exchange, for the server's log.
fn report(answered: net::Answered) {
    match answered {
        Ok((peer, summary)) => println!(
            "{peer}: sent {}, received {} new",
            summary.sent, summary.received
        ),
        Err(error) => eprintln!("refused or failed: {error:?}"),
    }
}
