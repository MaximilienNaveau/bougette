//! What a device does with its server, as operations: join it, sync, link
//! another device of the member, invite someone to a budget, accept an
//! invitation, set up recovery, recover. The command-line client and the
//! app both use them, and word the results their own way.
//!
//! The device keeps its server's address and its username there in its
//! settings (`server`, `username`).

use std::sync::Mutex;

use bougette_store::crypto::{self, Cost};
use bougette_store::{Invitation, Role, Store, StoreError};
use iroh::{Endpoint, EndpointAddr};

use crate::net::{self, Request};
use crate::{Summary, SyncError, lock};

/// A device's connection to the network, for one or more operations.
// `'a`: the Client borrows the store; it can't outlive it.
pub struct Client<'a> {
    store: &'a Mutex<Store>,
    endpoint: Endpoint,
}

impl<'a> Client<'a> {
    /// Opens the device's endpoint (any free port).
    pub async fn new(store: &'a Mutex<Store>) -> Result<Client<'a>, String> {
        let key = net::secret_key(&mut lock(store)).map_err(failed)?;
        let endpoint = net::endpoint(key, 0).await.map_err(failed)?;
        Ok(Client { store, endpoint })
    }

    /// Closes the endpoint politely, so peers don't wait for a timeout.
    pub async fn close(self) {
        self.endpoint.close().await;
    }

    /// The server's address and this device's username there, if it
    /// joined one.
    pub fn server(&self) -> Result<Option<(String, String)>, String> {
        let store = lock(self.store);
        let address = store.setting("server").map_err(stored)?;
        let username = store.setting("username").map_err(stored)?;
        Ok(address.map(|address| (address, username.unwrap_or_default())))
    }

    /// Joins the server at `address` with the operator's invitation
    /// `token` (hexadecimal), as `username`, then syncs.
    pub async fn join(
        &self,
        address: &str,
        token: &str,
        username: &str,
    ) -> Result<Summary, String> {
        let peer = net::parse_address(address)?;
        let token = crypto::from_hex(token.trim()).ok_or("not an invitation token")?;
        let request = Request::Redeem {
            token,
            username: username.to_string(),
            member: Some(self.proof()?),
        };
        let joined = self.call(&peer, &request).await?;
        self.remember(address, &String::from_utf8_lossy(&joined))?;
        self.sync().await
    }

    /// Syncs with the server, then refreshes the backup there, if any.
    pub async fn sync(&self) -> Result<Summary, String> {
        let peer = self.peer()?;
        let summary = net::sync_with(&self.endpoint, self.store, peer.clone())
            .await
            .map_err(failed)?;
        // In its own statement: a lock taken in an `if let` condition would
        // be held through the whole block, across the `.await`.
        let backup = lock(self.store).backup().map_err(stored)?;
        if let Some(backup) = backup {
            self.call(&peer, &Request::PutBackup(backup)).await?;
        }
        Ok(summary)
    }

    /// Syncs with another device of the member, serving at `address`.
    pub async fn sync_device(&self, address: &str) -> Result<Summary, String> {
        let peer = net::parse_address(address)?;
        net::sync_with(&self.endpoint, self.store, peer)
            .await
            .map_err(failed)
    }

    /// The code that makes a new device one of the member's: their secret
    /// seed, and the server's address if any. It opens all their budgets.
    pub fn device_code(&self) -> Result<String, String> {
        let mut store = lock(self.store);
        let seed = store.identity_seed().map_err(stored)?;
        let server = store.setting("server").map_err(stored)?.unwrap_or_default();
        Ok(format!("{}#{server}", crypto::hex(&seed)))
    }

    /// Makes this new device one of the member's with a `device_code`, and
    /// joins their server and syncs if the code names one.
    pub async fn link(&self, code: &str) -> Result<Option<Summary>, String> {
        let code = code.trim();
        let (seed, address) = code.split_once('#').unwrap_or((code, ""));
        let seed = crypto::from_hex_array::<32>(seed).ok_or("not a device code")?;
        lock(self.store).adopt_identity(seed).map_err(stored)?;
        if address.is_empty() {
            return Ok(None);
        }
        let peer = net::parse_address(address)?;
        let (member, proof) = self.proof()?;
        let username = self.call(&peer, &Request::Link { member, proof }).await?;
        self.remember(address, &String::from_utf8_lossy(&username))?;
        self.sync().await.map(Some)
    }

    /// An invitation to `budget` with `role`, valid `days` days and once,
    /// as the text to give: `SERVER#BUDGET#SECRET`. Only an Admin may.
    pub async fn invite(&self, budget: &str, role: Role, days: u64) -> Result<String, String> {
        let (address, _) = self.server()?.ok_or(NO_SERVER)?;
        let peer = net::parse_address(&address)?;
        // The server checks that we are an Admin: it must have the
        // budget's latest access changes.
        self.sync().await?;
        let id = budget
            .parse()
            .map(bougette_core::id::BudgetId)
            .map_err(|_| format!("not a budget: {budget}"))?;
        let expires = now().saturating_add(days.saturating_mul(86_400_000));
        let invitation = lock(self.store).invite(id, role, expires).map_err(stored)?;
        let request = Request::Invite {
            budget: invitation.budget.clone(),
            token_hash: crypto::sha256(&[&invitation.token()]).to_vec(),
            expires,
        };
        self.call(&peer, &request).await?;
        Ok(format!(
            "{address}#{}#{}",
            invitation.budget,
            crypto::hex(&invitation.secret)
        ))
    }

    /// Asks to join a budget with an invitation's text. A device new to
    /// the invitation's server joins it as `username`. An Admin's device
    /// lets the member in at its next sync; a later sync brings the budget.
    pub async fn accept(&self, text: &str, username: Option<&str>) -> Result<(), String> {
        let mut parts = text.trim().split('#');
        let (Some(address), Some(budget), Some(secret), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err("not an invitation".into());
        };
        let invitation = Invitation {
            budget: budget.to_string(),
            secret: crypto::from_hex_array(secret).ok_or("not an invitation")?,
        };
        let known = self.server()?.map(|(address, _)| address);
        if known.as_deref() != Some(address) {
            let username = username.ok_or("new on this server: choose a username")?;
            let peer = net::parse_address(address)?;
            let request = Request::Redeem {
                token: invitation.token().to_vec(),
                username: username.to_string(),
                member: Some(self.proof()?),
            };
            self.call(&peer, &request).await?;
            self.remember(address, username)?;
        }
        lock(self.store).request_join(&invitation).map_err(stored)?;
        self.sync().await.map(|_| ())
    }

    /// Locks a backup on the server with `phrase`, or with a phrase drawn
    /// now, which is returned: the member must write it down. A chosen
    /// phrase must be strong enough.
    pub async fn set_up_recovery(
        &self,
        phrase: Option<&str>,
        cost: Cost,
    ) -> Result<Option<String>, String> {
        let peer = self.peer()?;
        let (phrase, generated) = match phrase {
            Some(phrase) if crypto::strong_enough(phrase) => (phrase.to_string(), false),
            Some(_) => return Err("too weak: at least 4 words and 20 letters".into()),
            None => (crypto::new_phrase(), true),
        };
        let backup = lock(self.store)
            .set_recovery(&phrase, cost)
            .map_err(stored)?;
        self.call(&peer, &Request::PutBackup(backup)).await?;
        Ok(generated.then_some(phrase))
    }

    /// Gets everything back on this new device, after losing all the
    /// member's devices: the operator's new invitation `token` lets it
    /// into the server as `username`, and `phrase` opens the backup.
    pub async fn recover(
        &self,
        address: &str,
        token: &str,
        username: &str,
        phrase: &str,
    ) -> Result<Summary, String> {
        let peer = net::parse_address(address)?;
        let token = crypto::from_hex(token.trim()).ok_or("not an invitation token")?;
        let request = Request::Redeem {
            token,
            username: username.to_string(),
            member: None,
        };
        // A second try (e.g. after mistyping the phrase) finds the
        // invitation used, but this device already let in.
        let redeemed = self.call(&peer, &request).await;
        let backup = match self.call(&peer, &Request::GetBackup).await {
            Ok(backup) => backup,
            Err(error) => return Err(redeemed.err().unwrap_or(error)),
        };
        lock(self.store).restore(&backup, phrase).map_err(stored)?;
        self.remember(address, username)?;
        self.sync().await
    }

    /// The server's address, parsed.
    fn peer(&self) -> Result<EndpointAddr, String> {
        let (address, _) = self.server()?.ok_or(NO_SERVER)?;
        net::parse_address(&address)
    }

    /// This member's proof that this device is theirs.
    fn proof(&self) -> Result<(String, Vec<u8>), String> {
        lock(self.store)
            .prove_device(&self.endpoint.id().to_string())
            .map_err(stored)
    }

    fn remember(&self, address: &str, username: &str) -> Result<(), String> {
        let mut store = lock(self.store);
        store.set_setting("server", address).map_err(stored)?;
        store.set_setting("username", username).map_err(stored)
    }

    async fn call(&self, peer: &EndpointAddr, request: &Request) -> Result<Vec<u8>, String> {
        net::call(&self.endpoint, peer.clone(), request)
            .await
            .map_err(failed)
    }
}

const NO_SERVER: &str = "no server yet: join one first";

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}

/// A sync error, in words.
pub fn failed(error: SyncError) -> String {
    match error {
        SyncError::Refused(reason) => format!("refused: {reason}"),
        SyncError::Store(error) => stored(error),
        other => format!("sync failed: {other:?}"),
    }
}

/// A storage error, in words.
pub fn stored(error: StoreError) -> String {
    match error {
        StoreError::NotAllowed(reason) => reason,
        StoreError::NoKey => "this device has no key of this budget yet: sync first".to_string(),
        other => format!("storage: {other:?}"),
    }
}
