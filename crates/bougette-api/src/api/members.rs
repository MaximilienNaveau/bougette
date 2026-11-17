//! Members, the server and recovery, for the app: the same operations as
//! the command-line client (`bougette_sync::client`), worded by the app.

use bougette_core::id::BudgetId;
use bougette_store::Role;
use bougette_store::crypto::RECOVERY_COST;
use bougette_sync::Summary;
use bougette_sync::client::Client;

use super::{App, stored};

/// The server this device joined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerInfo {
    pub address: String,
    pub username: String,
}

/// What a sync did: changes sent, and new changes received.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncSummary {
    pub sent: u32,
    pub received: u32,
}

/// What a member may do in a budget (GLOSSARY.md: Role).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberRole {
    Admin,
    Editor,
    Reader,
}

/// A member of a budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberRow {
    pub id: String,
    /// Their display name, once they've written it.
    pub name: Option<String>,
    pub role: MemberRole,
    /// Whether it's the member using this device.
    pub me: bool,
}

impl App {
    /// The server this device joined, if any.
    pub fn server(&self) -> Result<Option<ServerInfo>, String> {
        let store = self.lock();
        let address = store.setting("server").map_err(stored)?;
        let username = store.setting("username").map_err(stored)?;
        Ok(address.map(|address| ServerInfo {
            address,
            username: username.unwrap_or_default(),
        }))
    }

    /// How this member appears to the others, if set.
    pub fn name(&self) -> Result<Option<String>, String> {
        self.lock().setting("name").map_err(stored)
    }

    /// Sets how this member appears to the others, in every budget.
    pub fn set_name(&self, name: String) -> Result<(), String> {
        self.lock().set_name(&name).map_err(stored)
    }

    /// Joins the server at `address` with the operator's invitation, then
    /// syncs.
    pub fn join_server(
        &self,
        address: String,
        token: String,
        username: String,
    ) -> Result<SyncSummary, String> {
        self.online(async |client| client.join(&address, &token, &username).await)
            .map(summary)
    }

    /// Syncs with the server. (Not `sync`: a keyword in Dart.)
    pub fn sync_now(&self) -> Result<SyncSummary, String> {
        self.online(async |client| client.sync().await).map(summary)
    }

    /// The secret code that makes a new device one of this member's.
    pub fn device_code(&self) -> Result<String, String> {
        self.online(async |client| client.device_code())
    }

    /// Makes this new device one of the member's, with a device code.
    pub fn link_device(&self, code: String) -> Result<(), String> {
        self.online(async |client| client.link(&code).await.map(|_| ()))
    }

    /// An invitation to `budget` with `role`, valid `days` days: the text
    /// to give the newcomer.
    pub fn invite(&self, budget: String, role: MemberRole, days: u32) -> Result<String, String> {
        self.online(async |client| client.invite(&budget, store_role(role), days.into()).await)
    }

    /// Asks to join a budget with an invitation, as `username` on a server
    /// this device hasn't joined yet.
    pub fn accept(&self, invitation: String, username: Option<String>) -> Result<(), String> {
        self.online(async |client| client.accept(&invitation, username.as_deref()).await)
    }

    /// Whether a recovery phrase locks a backup on the server.
    pub fn has_recovery(&self) -> Result<bool, String> {
        Ok(self
            .lock()
            .setting("recovery-key")
            .map_err(stored)?
            .is_some())
    }

    /// Locks a backup on the server with `phrase`, or with one drawn now
    /// and returned, to write down.
    pub fn set_up_recovery(&self, phrase: Option<String>) -> Result<Option<String>, String> {
        self.online(async |client| {
            client
                .set_up_recovery(phrase.as_deref(), RECOVERY_COST)
                .await
        })
    }

    /// Gets everything back on this new device with a new invitation from
    /// the operator and the recovery phrase.
    pub fn recover(
        &self,
        address: String,
        token: String,
        username: String,
        phrase: String,
    ) -> Result<SyncSummary, String> {
        self.online(async |client| client.recover(&address, &token, &username, &phrase).await)
            .map(summary)
    }

    /// The members of `budget`, Admins first.
    pub fn members(&self, budget: String) -> Result<Vec<MemberRow>, String> {
        let id = self.load(&budget)?.id;
        let mut store = self.lock();
        let me = store.member().map_err(stored)?;
        Ok(store
            .members(id)
            .map_err(stored)?
            .into_iter()
            .map(|member| MemberRow {
                me: member.id == me,
                id: member.id,
                name: member.name,
                role: member_role(member.role),
            })
            .collect())
    }

    /// This member's role in `budget`.
    pub fn my_role(&self, budget: String) -> Result<Option<MemberRole>, String> {
        let id = budget_id(&budget)?;
        Ok(self.lock().role(id).map_err(stored)?.map(member_role))
    }

    /// Gives `member` the role `role` in `budget` (an Admin only).
    pub fn set_role(&self, budget: String, member: String, role: MemberRole) -> Result<(), String> {
        let id = budget_id(&budget)?;
        self.lock()
            .set_role(id, &member, store_role(role))
            .map_err(stored)
    }

    /// Removes `member` from `budget` (an Admin only): they keep what they
    /// had, and get nothing new.
    pub fn remove_member(&self, budget: String, member: String) -> Result<(), String> {
        let id = budget_id(&budget)?;
        self.lock().remove(id, &member).map_err(stored)
    }

    /// This member leaves `budget`.
    pub fn leave(&self, budget: String) -> Result<(), String> {
        let id = budget_id(&budget)?;
        let mut store = self.lock();
        let me = store.member().map_err(stored)?;
        store.remove(id, &me).map_err(stored)
    }

    /// Runs a network operation: opens the device's connection, runs it,
    /// closes it, waiting for all that on this thread.
    // `AsyncFnOnce`: an async closure, called once. Its argument borrows
    // the client for the time of the call only (`for<'c>` is implied).
    fn online<T>(
        &self,
        operation: impl AsyncFnOnce(&Client<'_>) -> Result<T, String>,
    ) -> Result<T, String> {
        self.runtime.block_on(async {
            let client = Client::new(&self.store).await?;
            let result = operation(&client).await;
            client.close().await;
            result
        })
    }
}

fn summary(summary: Summary) -> SyncSummary {
    // More than 4 billion changes in one sync would be a bug.
    SyncSummary {
        sent: u32::try_from(summary.sent).unwrap_or(u32::MAX),
        received: u32::try_from(summary.received).unwrap_or(u32::MAX),
    }
}

fn budget_id(text: &str) -> Result<BudgetId, String> {
    text.parse()
        .map(BudgetId)
        .map_err(|_| format!("not a budget: {text}"))
}

fn member_role(role: Role) -> MemberRole {
    match role {
        Role::Admin => MemberRole::Admin,
        Role::Editor => MemberRole::Editor,
        Role::Reader => MemberRole::Reader,
    }
}

fn store_role(role: MemberRole) -> Role {
    match role {
        MemberRole::Admin => Role::Admin,
        MemberRole::Editor => Role::Editor,
        MemberRole::Reader => Role::Reader,
    }
}
