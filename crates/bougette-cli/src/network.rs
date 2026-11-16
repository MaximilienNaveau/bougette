//! The commands that talk to the server or to another device. The
//! operations are in `bougette_sync::client`, shared with the app; this
//! module reads the words and words the results for the terminal.

use std::sync::{Arc, Mutex};

use bougette_store::crypto::RECOVERY_COST;
use bougette_store::{Role, Store};
use bougette_sync::client::{Client, failed};
use bougette_sync::net::{self, OwnDevices};
use bougette_sync::{Summary, lock};

use crate::commands::{Options, find_budget, stored};

/// Whether `words` is one of the commands of this module.
pub fn handles(words: &[&str]) -> bool {
    matches!(
        words,
        ["id"]
            | ["serve"]
            | ["sync"]
            | ["sync", _]
            | ["server", "join", _, _, _]
            | ["device", "code"]
            | ["device", "link", _]
            | ["invite", _]
            | ["accept", _]
            | ["recovery", "setup"]
            | ["recover", _, _, _, _, ..]
    )
}

pub fn run(words: &[&str], options: &Options, store: Store) -> Result<String, String> {
    let store = Mutex::new(store);
    // `block_on` runs a future to completion on this thread: the bridge
    // between ordinary code and async code. An `async` block is an inline
    // future, like an async lambda called at once.
    runtime()?.block_on(async {
        if words == ["serve"] {
            return serve(store, options).await;
        }
        let client = Client::new(&store).await?;
        let output = command(&client, &store, words, options).await;
        client.close().await;
        output
    })
}

/// One command, with the device's connection open.
async fn command(
    client: &Client<'_>,
    store: &Mutex<Store>,
    words: &[&str],
    options: &Options,
) -> Result<String, String> {
    match words {
        ["id"] => {
            let key = net::secret_key(&mut lock(store)).map_err(failed)?;
            let member = stored(lock(store).member())?;
            Ok(format!("device {}\nmember {member}\n", key.public()))
        }
        ["sync"] => Ok(synced(client.sync().await?)),
        ["sync", address] => Ok(synced(client.sync_device(address).await?)),
        ["server", "join", address, token, username] => {
            let summary = client.join(address, token, username).await?;
            Ok(format!("Joined the server.\n{}", synced(summary)))
        }
        ["device", "code"] => Ok(format!(
            "On your new device, run `bougette device link CODE` with this\n\
             code. Keep it secret: it opens all your budgets.\n  {}\n",
            client.device_code()?
        )),
        ["device", "link", code] => Ok(match client.link(code).await? {
            Some(summary) => format!("Linked.\n{}", synced(summary)),
            None => "Linked. Sync with your other device: `bougette sync ADDRESS`.\n".into(),
        }),
        ["invite", budget] => {
            let id = find_budget(&lock(store), budget)?.id;
            let role = match options.get("role") {
                Some(role) => Role::parse(role).ok_or_else(|| format!("not a role: {role}"))?,
                None => Role::Editor,
            };
            let days: u64 = match options.get("days") {
                Some(days) => days.parse().map_err(|_| format!("not a number: {days}"))?,
                None => 7,
            };
            let invitation = client.invite(&id.to_string(), role, days).await?;
            Ok(format!(
                "Give this invitation ({}, valid {days} days, once):\n  {invitation}\n\
                 Then sync now and then: your device lets them in.\n",
                role.as_str()
            ))
        }
        ["accept", invitation] => {
            let username = options.get("username").map(String::as_str);
            client.accept(invitation, username).await?;
            Ok(
                "Asked to join. An Admin's device lets you in at its next sync;\n\
                then `bougette sync` brings you the budget.\n"
                    .to_string(),
            )
        }
        ["recovery", "setup"] => {
            let phrase = options.get("phrase").map(String::as_str);
            let generated = client.set_up_recovery(phrase, RECOVERY_COST).await?;
            let mut out = "Your backup is on the server, locked by your phrase.\n".to_string();
            if let Some(phrase) = generated {
                out += &format!(
                    "Write this recovery phrase down and keep it safe; nobody,\n\
                     the server's operator included, can give it back:\n  {phrase}\n"
                );
            }
            Ok(out)
        }
        ["recover", address, token, username, phrase @ ..] => {
            let summary = client
                .recover(address, token, username, &phrase.join(" "))
                .await?;
            Ok(format!("Recovered.\n{}", synced(summary)))
        }
        _ => Err("not a network command".to_string()),
    }
}

/// Lets this member's other devices sync with this one, forever.
async fn serve(store: Mutex<Store>, options: &Options) -> Result<String, String> {
    let port = match options.get("port") {
        Some(port) => port.parse().map_err(|_| format!("not a port: {port}"))?,
        None => 0,
    };
    let key = net::secret_key(&mut lock(&store)).map_err(failed)?;
    let member = stored(lock(&store).member())?;
    let endpoint = net::endpoint(key, port).await.map_err(failed)?;
    println!(
        "Serving your devices. On another one, run:\n  bougette sync {}",
        net::address(&endpoint)
    );
    net::serve(endpoint, Arc::new(OwnDevices { store, member }), report).await;
    Ok(String::new())
}

fn synced(summary: Summary) -> String {
    format!(
        "Sent {} changes, received {} new ones.\n",
        summary.sent, summary.received
    )
}

/// A runtime on this thread only: the CLI does one thing at a time, and
/// `serve` handles its connections as tasks on the same thread.
fn runtime() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("cannot start the async runtime: {error}"))
}

/// Prints a line about each exchange answered by `serve`.
fn report(line: Result<String, String>) {
    match line {
        Ok(line) => println!("{line}"),
        Err(line) => eprintln!("{line}"),
    }
}
