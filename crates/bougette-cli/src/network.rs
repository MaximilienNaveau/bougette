//! The commands that talk to the server or to another device. They run
//! on an async runtime and share the store between tasks, so they take it
//! by value.
//!
//! The device remembers its server's address and its username in its
//! settings (`server`, `username`).

use std::sync::{Arc, Mutex};

use bougette_store::crypto::{self, RECOVERY_COST};
use bougette_store::{Invitation, Role, Store};
use bougette_sync::net::{self, OwnDevices, Request};
use bougette_sync::{SyncError, lock};
use iroh::{Endpoint, EndpointAddr, SecretKey};

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

pub fn run(words: &[&str], options: &Options, mut store: Store) -> Result<String, String> {
    let key = net::secret_key(&mut store).map_err(failed)?;
    let store = Mutex::new(store);
    // `block_on` runs a future to completion on this thread: the bridge
    // between ordinary code and async code. An `async` block is an inline
    // future, like an async lambda called at once.
    runtime()?.block_on(async {
        match words {
            ["id"] => Ok(format!(
                "device {}\nmember {}\n",
                key.public(),
                stored(lock(&store).member())?
            )),
            ["serve"] => serve(key, store, options).await,
            ["sync"] => {
                let endpoint = endpoint(key).await?;
                let output = sync_server(&endpoint, &store).await;
                endpoint.close().await;
                output
            }
            ["sync", address] => {
                let peer = net::parse_address(address)?;
                let endpoint = endpoint(key).await?;
                let summary = net::sync_with(&endpoint, &store, peer)
                    .await
                    .map_err(failed)?;
                endpoint.close().await;
                Ok(format!(
                    "Sent {} changes, received {} new ones.\n",
                    summary.sent, summary.received
                ))
            }
            ["server", "join", address, token, username] => {
                let peer = net::parse_address(address)?;
                let token = crypto::from_hex(token).ok_or("not an invitation token")?;
                let endpoint = endpoint(key).await?;
                let proof = stored(lock(&store).prove_device(&endpoint.id().to_string()))?;
                let request = Request::Redeem {
                    token,
                    username: username.to_string(),
                    member: Some(proof),
                };
                let joined = call(&endpoint, &peer, &request).await?;
                remember_server(&store, address, &String::from_utf8_lossy(&joined))?;
                let synced = sync_server(&endpoint, &store).await?;
                endpoint.close().await;
                Ok(format!("Joined the server.\n{synced}"))
            }
            ["device", "code"] => {
                let seed = stored(lock(&store).identity_seed())?;
                let server = stored(lock(&store).setting("server"))?.unwrap_or_default();
                Ok(format!(
                    "On your new device, run `bougette device link CODE` with this\n\
                     code. Keep it secret: it opens all your budgets.\n  {}#{server}\n",
                    crypto::hex(&seed)
                ))
            }
            ["device", "link", code] => {
                let (seed, address) = code.split_once('#').unwrap_or((code, ""));
                let seed = crypto::from_hex_array::<32>(seed).ok_or("not a device code")?;
                stored(lock(&store).adopt_identity(seed))?;
                if address.is_empty() {
                    return Ok(
                        "Linked. Sync with your other device: `bougette sync ADDRESS`.\n".into(),
                    );
                }
                let peer = net::parse_address(address)?;
                let endpoint = endpoint(key).await?;
                let (member, proof) =
                    stored(lock(&store).prove_device(&endpoint.id().to_string()))?;
                let username = call(&endpoint, &peer, &Request::Link { member, proof }).await?;
                remember_server(&store, address, &String::from_utf8_lossy(&username))?;
                let synced = sync_server(&endpoint, &store).await?;
                endpoint.close().await;
                Ok(format!("Linked.\n{synced}"))
            }
            ["invite", budget] => {
                let (address, peer) = server(&store)?;
                let endpoint = endpoint(key).await?;
                // The server checks that we are an Admin: it must have the
                // budget's latest access changes.
                sync_server(&endpoint, &store).await?;
                let id = find_budget(&lock(&store), budget)?.id;
                let role = match options.get("role") {
                    Some(role) => Role::parse(role).ok_or_else(|| format!("not a role: {role}"))?,
                    None => Role::Editor,
                };
                let days: u64 = match options.get("days") {
                    Some(days) => days.parse().map_err(|_| format!("not a number: {days}"))?,
                    None => 7,
                };
                let expires = now().saturating_add(days.saturating_mul(86_400_000));
                let invitation = stored(lock(&store).invite(id, role, expires))?;
                let request = Request::Invite {
                    budget: invitation.budget.clone(),
                    token_hash: crypto::sha256(&[&invitation.token()]).to_vec(),
                    expires,
                };
                call(&endpoint, &peer, &request).await?;
                endpoint.close().await;
                Ok(format!(
                    "Give this invitation ({}, valid {days} days, once):\n  {address}#{}#{}\n\
                     Then sync now and then: your device lets them in.\n",
                    role.as_str(),
                    invitation.budget,
                    crypto::hex(&invitation.secret)
                ))
            }
            ["accept", text] => {
                let mut parts = text.split('#');
                let (Some(address), Some(budget), Some(secret), None) =
                    (parts.next(), parts.next(), parts.next(), parts.next())
                else {
                    return Err("not an invitation".into());
                };
                let invitation = Invitation {
                    budget: budget.to_string(),
                    secret: crypto::from_hex_array(secret).ok_or("not an invitation")?,
                };
                let peer = net::parse_address(address)?;
                let endpoint = endpoint(key).await?;
                let known = stored(lock(&store).setting("server"))?;
                if known.as_deref() != Some(address) {
                    let username = options
                        .get("username")
                        .ok_or("new on this server: choose a --username")?;
                    let proof = stored(lock(&store).prove_device(&endpoint.id().to_string()))?;
                    let request = Request::Redeem {
                        token: invitation.token().to_vec(),
                        username: username.clone(),
                        member: Some(proof),
                    };
                    call(&endpoint, &peer, &request).await?;
                    remember_server(&store, address, username)?;
                }
                stored(lock(&store).request_join(&invitation))?;
                sync_server(&endpoint, &store).await?;
                endpoint.close().await;
                Ok(
                    "Asked to join. An Admin's device lets you in at its next sync;\n\
                    then `bougette sync` brings you the budget.\n"
                        .to_string(),
                )
            }
            ["recovery", "setup"] => {
                let (_, peer) = server(&store)?;
                let (phrase, generated) = match options.get("phrase") {
                    Some(phrase) if crypto::strong_enough(phrase) => (phrase.to_string(), false),
                    Some(_) => {
                        return Err("too weak: at least 4 words and 20 letters".into());
                    }
                    None => (crypto::new_phrase(), true),
                };
                let backup = stored(lock(&store).set_recovery(&phrase, RECOVERY_COST))?;
                let endpoint = endpoint(key).await?;
                call(&endpoint, &peer, &Request::PutBackup(backup)).await?;
                endpoint.close().await;
                let mut out = "Your backup is on the server, locked by your phrase.\n".to_string();
                if generated {
                    out += &format!(
                        "Write this recovery phrase down and keep it safe; nobody,\n\
                         the server's operator included, can give it back:\n  {phrase}\n"
                    );
                }
                Ok(out)
            }
            ["recover", address, token, username, phrase @ ..] => {
                let peer = net::parse_address(address)?;
                let token = crypto::from_hex(token).ok_or("not an invitation token")?;
                let endpoint = endpoint(key).await?;
                let request = Request::Redeem {
                    token,
                    username: username.to_string(),
                    member: None,
                };
                // A second try (e.g. after mistyping the phrase) finds the
                // invitation used, but this device already let in.
                let redeemed = call(&endpoint, &peer, &request).await;
                let backup = match call(&endpoint, &peer, &Request::GetBackup).await {
                    Ok(backup) => backup,
                    Err(error) => return Err(redeemed.err().unwrap_or(error)),
                };
                stored(lock(&store).restore(&backup, &phrase.join(" ")))?;
                remember_server(&store, address, username)?;
                let synced = sync_server(&endpoint, &store).await?;
                endpoint.close().await;
                Ok(format!("Recovered.\n{synced}"))
            }
            _ => Err("not a network command".to_string()),
        }
    })
}

/// Lets this member's other devices sync with this one, forever.
async fn serve(key: SecretKey, store: Mutex<Store>, options: &Options) -> Result<String, String> {
    let port = match options.get("port") {
        Some(port) => port.parse().map_err(|_| format!("not a port: {port}"))?,
        None => 0,
    };
    let member = stored(lock(&store).member())?;
    let endpoint = net::endpoint(key, port).await.map_err(failed)?;
    println!(
        "Serving your devices. On another one, run:\n  bougette sync {}",
        net::address(&endpoint)
    );
    net::serve(endpoint, Arc::new(OwnDevices { store, member }), report).await;
    Ok(String::new())
}

/// Syncs with the server, then refreshes the backup there if there is one.
async fn sync_server(endpoint: &Endpoint, store: &Mutex<Store>) -> Result<String, String> {
    let (_, peer) = server(store)?;
    let summary = net::sync_with(endpoint, store, peer.clone())
        .await
        .map_err(failed)?;
    // In its own statement: a lock taken in an `if let` condition would
    // be held through the whole block, across the `.await`.
    let backup = stored(lock(store).backup())?;
    if let Some(backup) = backup {
        call(endpoint, &peer, &Request::PutBackup(backup)).await?;
    }
    Ok(format!(
        "Sent {} changes, received {} new ones.\n",
        summary.sent, summary.received
    ))
}

/// The server's address, as text and parsed.
fn server(store: &Mutex<Store>) -> Result<(String, EndpointAddr), String> {
    let address = stored(lock(store).setting("server"))?
        .ok_or("no server yet: `bougette server join ADDRESS TOKEN USERNAME`")?;
    let peer = net::parse_address(&address)?;
    Ok((address, peer))
}

fn remember_server(store: &Mutex<Store>, address: &str, username: &str) -> Result<(), String> {
    let mut store = lock(store);
    stored(store.set_setting("server", address))?;
    stored(store.set_setting("username", username))
}

async fn call(
    endpoint: &Endpoint,
    peer: &EndpointAddr,
    request: &Request,
) -> Result<Vec<u8>, String> {
    net::call(endpoint, peer.clone(), request)
        .await
        .map_err(failed)
}

async fn endpoint(key: SecretKey) -> Result<Endpoint, String> {
    net::endpoint(key, 0).await.map_err(failed)
}

/// A runtime on this thread only: the CLI does one thing at a time, and
/// `serve` handles its connections as tasks on the same thread.
fn runtime() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("cannot start the async runtime: {error}"))
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}

/// Prints a line about each exchange answered by `serve`.
fn report(line: Result<String, String>) {
    match line {
        Ok(line) => println!("{line}"),
        Err(line) => eprintln!("{line}"),
    }
}

fn failed(error: SyncError) -> String {
    match error {
        SyncError::Refused(reason) => format!("refused: {reason}"),
        other => format!("sync failed: {other:?}"),
    }
}
