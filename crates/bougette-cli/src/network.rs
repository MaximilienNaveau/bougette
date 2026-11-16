//! The commands that talk to other devices: `id`, `serve` and `sync`.
//! Unlike the others, they run on an async runtime and share the store
//! between tasks, so they take it by value.

use std::sync::{Arc, Mutex};

use bougette_store::Store;
use bougette_sync::SyncError;
use bougette_sync::net;

use crate::commands::Options;

pub fn run(words: &[&str], options: &Options, mut store: Store) -> Result<String, String> {
    let key = net::secret_key(&mut store).map_err(failed)?;
    match words {
        ["id"] => Ok(format!("{}\n", key.public())),
        ["serve"] => {
            let allowed = options
                .get("allow")
                .ok_or("serve needs --allow ID,... (run `bougette id` on the other device)")?;
            let allowed = allowed
                .split(',')
                .map(net::parse_id)
                .collect::<Result<Vec<_>, _>>()?;
            let port = match options.get("port") {
                Some(port) => port.parse().map_err(|_| format!("not a port: {port}"))?,
                None => 0,
            };
            // `block_on` runs a future to completion on this thread: the
            // bridge between ordinary code and async code. An `async` block
            // is an inline future, like an async lambda called at once.
            runtime()?.block_on(async {
                let endpoint = net::endpoint(key, port).await.map_err(failed)?;
                println!(
                    "Serving. On an allowed device, run:\n  bougette sync {}",
                    net::address(&endpoint)
                );
                net::serve(endpoint, Arc::new(Mutex::new(store)), allowed, report).await;
                Ok(String::new())
            })
        }
        ["sync", address] => {
            let peer = net::parse_address(address)?;
            runtime()?.block_on(async {
                let endpoint = net::endpoint(key, 0).await.map_err(failed)?;
                let summary = net::sync_with(&endpoint, &Mutex::new(store), peer)
                    .await
                    .map_err(failed)?;
                // Say goodbye properly, so the peer doesn't wait for a
                // timeout.
                endpoint.close().await;
                Ok(format!(
                    "Sent {} changes, received {} new ones.\n",
                    summary.sent, summary.received
                ))
            })
        }
        _ => Err("not a network command".to_string()),
    }
}

/// A runtime on this thread only: the CLI does one thing at a time, and
/// `serve` handles its connections as tasks on the same thread.
fn runtime() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("cannot start the async runtime: {error}"))
}

/// Prints how an exchange answered by `serve` went.
fn report(answered: net::Answered) {
    match answered {
        Ok((peer, summary)) => println!(
            "{peer}: sent {} changes, received {} new ones",
            summary.sent, summary.received
        ),
        Err(error) => eprintln!("{}", failed(error)),
    }
}

fn failed(error: SyncError) -> String {
    format!("sync failed: {error:?}")
}
