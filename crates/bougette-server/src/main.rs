//! `bougette-server`: a device that is always online (ADR 0001). It keeps
//! the messages the devices send it and passes them on to the members of
//! each budget, so they sync without ever being online together. It keeps
//! each member's encrypted backup too. It reads only the access changes
//! (who is in which budget), never the budgets themselves.
//!
//! The operator lets users in (`invite`), revokes them (`revoke`) and
//! lists them (`list`); an Admin of a budget can let a newcomer in with an
//! invitation to the budget.

mod registry;

use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};

use bougette_store::{Role, Store, device_belongs};
use bougette_sync::net::{self, Request, Responder};
use bougette_sync::{Scope, lock};
use iroh::EndpointId;

use crate::registry::Registry;

const USAGE: &str = "\
Usage: bougette-server --db FILE COMMAND

  serve --port PORT          answer the devices, forever
  invite USERNAME [--days N] let a user in (7 days by default); again for
                             a user who lost their devices
  revoke USERNAME            the user's devices can't connect any more
  list                       the users and their number of devices

The server keeps encrypted changes and backups: it can't read budgets,
and letting someone in again never gives them the content.
";

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(output) => {
            print!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("bougette-server: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: Vec<String>) -> Result<String, String> {
    let (words, options) = parse(arguments)?;
    let path = options
        .get("db")
        .ok_or_else(|| format!("missing --db\n\n{USAGE}"))?;
    let path = Path::new(path);
    // The message store first: it checks the file's version.
    let mut store =
        Store::open_server(path).map_err(|error| format!("cannot open the database: {error:?}"))?;
    let registry =
        Registry::open(path).map_err(|error| format!("cannot open the registry: {error}"))?;
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["serve"] => {
            let port: u16 = options
                .get("port")
                .ok_or("serve needs --port")?
                .parse()
                .map_err(|_| "--port takes a number from 1 to 65535".to_string())?;
            let key = net::secret_key(&mut store).map_err(|error| format!("{error:?}"))?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| format!("cannot start the async runtime: {error}"))?;
            runtime.block_on(async {
                let endpoint = net::endpoint(key, port)
                    .await
                    .map_err(|error| format!("cannot listen: {error:?}"))?;
                println!("Serving at {}", net::address(&endpoint));
                let server = Arc::new(Server {
                    store: Mutex::new(store),
                    registry: Mutex::new(registry),
                });
                net::serve(endpoint, server, report).await;
                Ok(String::new())
            })
        }
        ["invite", username] => {
            let days = match options.get("days") {
                Some(days) => days
                    .parse()
                    .map_err(|_| format!("not a number of days: {days}"))?,
                None => 7,
            };
            let token = registry
                .invite_user(username, days)
                .map_err(|error| error.to_string())?;
            Ok(format!(
                "Give {username} this invitation, valid {days} days, with the server's address:\n  {token}\n"
            ))
        }
        ["revoke", username] => {
            let devices = registry
                .revoke(username)
                .map_err(|error| error.to_string())?;
            Ok(format!("Revoked {devices} device(s) of {username}.\n"))
        }
        ["list"] => {
            let mut out = String::new();
            for (username, devices) in registry.list().map_err(|error| error.to_string())? {
                out += &format!("{username}: {devices} device(s)\n");
            }
            Ok(out)
        }
        _ => Err(format!("unknown command\n\n{USAGE}")),
    }
}

/// The server's answers to devices.
struct Server {
    store: Mutex<Store>,
    // rusqlite connections can't be shared between threads at once
    // (`!Sync`): a Mutex makes the registry shareable.
    registry: Mutex<Registry>,
}

impl Server {
    /// The username and member of a device allowed on the server.
    fn user(&self, device: &EndpointId) -> Result<(String, Option<String>), String> {
        self.registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .device_user(&device.to_string())
    }
}

impl Responder for Server {
    fn store(&self) -> &Mutex<Store> {
        &self.store
    }

    /// A device of a user receives the budgets its member belongs to.
    fn scope(&self, device: &EndpointId, member: &str) -> Result<Scope, String> {
        let (_, known) = self.user(device)?;
        if known.as_deref() != Some(member) {
            return Err("this device belongs to another member".to_string());
        }
        let store = lock(&self.store);
        let mut budgets = BTreeSet::new();
        for budget in store.budget_ids().map_err(|error| format!("{error:?}"))? {
            let access = store
                .access(&budget)
                .map_err(|error| format!("{error:?}"))?;
            if access.members.contains_key(member) {
                budgets.insert(budget);
            }
        }
        Ok(Scope::Only(budgets))
    }

    fn answer(&self, device: &EndpointId, request: Request) -> Result<Vec<u8>, String> {
        let registry = || {
            self.registry
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
        };
        let device_id = device.to_string();
        match request {
            Request::Redeem {
                token,
                username,
                member,
            } => {
                if let Some((member, proof)) = &member
                    && !device_belongs(member, &device_id, proof)
                {
                    return Err("the proof of membership is wrong".into());
                }
                let member = member.as_ref().map(|(member, _)| member.as_str());
                let username = registry().redeem(&token, &username, member, &device_id)?;
                Ok(username.into_bytes())
            }
            Request::Link { member, proof } => {
                if !device_belongs(&member, &device_id, &proof) {
                    return Err("the proof of membership is wrong".into());
                }
                Ok(registry().link(&member, &device_id)?.into_bytes())
            }
            Request::Invite {
                budget,
                token_hash,
                expires,
            } => {
                let (_, member) = self.user(device)?;
                let access = lock(&self.store)
                    .access(&budget)
                    .map_err(|error| format!("{error:?}"))?;
                if member.is_none_or(|member| access.role(&member) != Some(Role::Admin)) {
                    return Err("only an Admin of the budget invites (sync it first)".into());
                }
                if token_hash.len() != 32 {
                    return Err("bad token".into());
                }
                registry()
                    .invite_to_budget(&budget, &token_hash, expires)
                    .map_err(|error| error.to_string())?;
                Ok(Vec::new())
            }
            Request::PutBackup(backup) => {
                let (username, _) = self.user(device)?;
                registry().put_backup(&username, &backup)?;
                Ok(Vec::new())
            }
            Request::GetBackup => {
                let (username, _) = self.user(device)?;
                registry().backup(&username)
            }
            Request::Sync { .. } => Err("not here".into()),
        }
    }
}

/// Reads words and `--name value` options.
fn parse(arguments: Vec<String>) -> Result<(Vec<String>, HashMap<String, String>), String> {
    let mut words = Vec::new();
    let mut options = HashMap::new();
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        match argument.strip_prefix("--") {
            Some(name @ ("db" | "port" | "days")) => {
                let value = arguments
                    .next()
                    .ok_or_else(|| format!("--{name} needs a value"))?;
                options.insert(name.to_string(), value);
            }
            Some(_) => return Err(format!("unexpected {argument}\n\n{USAGE}")),
            None => words.push(argument),
        }
    }
    Ok((words, options))
}

/// One line per connection, for the server's log.
fn report(line: Result<String, String>) {
    match line {
        Ok(line) => println!("{line}"),
        Err(line) => eprintln!("{line}"),
    }
}
