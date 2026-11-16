//! Who may use the server (ADR 0003, first layer): its users, their
//! devices, the pending invitations and each user's encrypted backup. It
//! sits in the same SQLite file as the messages, in tables of its own.
//!
//! A user is a username and, once known, the member identifier their
//! devices prove. The operator can always let a user in again with a new
//! invitation (a lost phone), which gives access to the server only: the
//! content stays locked behind the member's keys and recovery phrase.

use std::path::Path;

use bougette_store::crypto;
use rusqlite::{Connection, OptionalExtension, params};

const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS users (
        username TEXT PRIMARY KEY,
        member TEXT
    );
    CREATE TABLE IF NOT EXISTS devices (
        device TEXT PRIMARY KEY,
        username TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS server_invitations (
        token_hash TEXT PRIMARY KEY,
        username TEXT,
        budget TEXT,
        expires INTEGER NOT NULL,
        used INTEGER NOT NULL DEFAULT 0
    );
    CREATE TABLE IF NOT EXISTS backups (
        username TEXT PRIMARY KEY,
        backup BLOB NOT NULL
    );
";

/// The server's list of users and devices.
pub struct Registry {
    connection: Connection,
}

/// Why a request was refused, as shown to the device.
type Refusal = String;

impl Registry {
    /// Opens the registry in the server's database file (after the message
    /// store, which checks the file's version).
    pub fn open(path: &Path) -> rusqlite::Result<Registry> {
        let connection = Connection::open(path)?;
        connection.execute_batch(SCHEMA)?;
        Ok(Registry { connection })
    }

    /// The operator invites `username`, for `days` days; returns the token
    /// to give them. Inviting an existing user lets a new device of theirs
    /// in (e.g. after losing all their devices).
    pub fn invite_user(&self, username: &str, days: u64) -> rusqlite::Result<String> {
        let token: [u8; 32] = crypto::random();
        self.connection.execute(
            "INSERT INTO server_invitations (token_hash, username, expires) VALUES (?1, ?2, ?3)",
            params![hash(&token), username, expiry(days)],
        )?;
        Ok(crypto::hex(&token))
    }

    /// An Admin's invitation to a budget: whoever shows the token may
    /// become a user, to ask to join the budget.
    pub fn invite_to_budget(
        &self,
        budget: &str,
        token_hash: &[u8],
        expires: u64,
    ) -> rusqlite::Result<()> {
        self.connection.execute(
            "INSERT OR IGNORE INTO server_invitations (token_hash, budget, expires)
             VALUES (?1, ?2, ?3)",
            params![
                crypto::hex(token_hash),
                budget,
                i64::try_from(expires).unwrap_or(i64::MAX)
            ],
        )?;
        Ok(())
    }

    /// Lets `device` in with an invitation token, as `username`, a device
    /// of `member` (if known: a recovering device learns its member back
    /// from its backup). Returns the username.
    pub fn redeem(
        &self,
        token: &[u8],
        username: &str,
        member: Option<&str>,
        device: &str,
    ) -> Result<String, Refusal> {
        // The invited username: chosen by the operator, or none for an
        // invitation to a budget.
        let row: Option<(Option<String>, i64)> = self
            .connection
            .query_row(
                "SELECT username, expires FROM server_invitations
                 WHERE token_hash = ?1 AND used = 0",
                params![hash(token)],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(failed)?;
        let Some((invited, expires)) = row else {
            return Err("unknown or already used invitation".into());
        };
        if expires < now() {
            return Err("this invitation has expired".into());
        }
        let username = match invited {
            Some(user) => user,
            // A newcomer to a budget picks a username, which must be free.
            None => {
                if self.user_exists(username)? {
                    return Err(format!("the username {username} is taken"));
                }
                if member.is_none() {
                    return Err("joining a budget needs a member".into());
                }
                username.to_string()
            }
        };
        self.connection
            .execute(
                "INSERT INTO users (username, member) VALUES (?1, ?2)
                 ON CONFLICT (username) DO UPDATE SET member = COALESCE(?2, member)",
                params![username, member],
            )
            .map_err(failed)?;
        self.add_device(device, &username)?;
        self.connection
            .execute(
                "UPDATE server_invitations SET used = 1 WHERE token_hash = ?1",
                params![hash(token)],
            )
            .map_err(failed)?;
        Ok(username)
    }

    /// Lets `device` in as another device of `member`, whose proof the
    /// caller checked.
    pub fn link(&self, member: &str, device: &str) -> Result<String, Refusal> {
        let username: Option<String> = self
            .connection
            .query_row(
                "SELECT username FROM users WHERE member = ?1",
                params![member],
                |row| row.get(0),
            )
            .optional()
            .map_err(failed)?;
        let username = username.ok_or("this member has no account on this server")?;
        self.add_device(device, &username)?;
        Ok(username)
    }

    /// The user of `device`, with their member identifier if known.
    pub fn device_user(&self, device: &str) -> Result<(String, Option<String>), Refusal> {
        self.connection
            .query_row(
                "SELECT users.username, users.member FROM devices
                 JOIN users ON users.username = devices.username
                 WHERE devices.device = ?1",
                params![device],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(failed)?
            .ok_or_else(|| "this device isn't allowed on this server".to_string())
    }

    pub fn put_backup(&self, username: &str, backup: &[u8]) -> Result<(), Refusal> {
        self.connection
            .execute(
                "INSERT OR REPLACE INTO backups (username, backup) VALUES (?1, ?2)",
                params![username, backup],
            )
            .map_err(failed)?;
        Ok(())
    }

    pub fn backup(&self, username: &str) -> Result<Vec<u8>, Refusal> {
        self.connection
            .query_row(
                "SELECT backup FROM backups WHERE username = ?1",
                params![username],
                |row| row.get(0),
            )
            .optional()
            .map_err(failed)?
            .ok_or_else(|| "no backup on this server".to_string())
    }

    /// The operator revokes `username`: their devices can't connect any
    /// more. Returns how many devices were revoked.
    pub fn revoke(&self, username: &str) -> rusqlite::Result<usize> {
        self.connection
            .execute("DELETE FROM devices WHERE username = ?1", params![username])
    }

    /// Each user with their number of devices, for the operator.
    pub fn list(&self) -> rusqlite::Result<Vec<(String, usize)>> {
        let mut statement = self.connection.prepare(
            "SELECT users.username, COUNT(devices.device) FROM users
             LEFT JOIN devices ON devices.username = users.username
             GROUP BY users.username ORDER BY users.username",
        )?;
        // SQLite counts are `i64`; a count is never negative.
        let rows = statement.query_map([], |row| {
            Ok((row.get(0)?, row.get::<_, i64>(1)?.unsigned_abs() as usize))
        })?;
        rows.collect()
    }

    fn add_device(&self, device: &str, username: &str) -> Result<(), Refusal> {
        self.connection
            .execute(
                "INSERT OR REPLACE INTO devices (device, username) VALUES (?1, ?2)",
                params![device, username],
            )
            .map_err(failed)?;
        Ok(())
    }

    fn user_exists(&self, username: &str) -> Result<bool, Refusal> {
        self.connection
            .query_row(
                "SELECT COUNT(*) FROM users WHERE username = ?1",
                params![username],
                |row| row.get::<_, i64>(0),
            )
            .map(|count| count > 0)
            .map_err(failed)
    }
}

/// What the server keeps of a token: its hash, so its database alone
/// doesn't let anyone in.
fn hash(token: &[u8]) -> String {
    crypto::hex(&crypto::sha256(&[token]))
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0)
}

fn expiry(days: u64) -> i64 {
    now().saturating_add(i64::try_from(days.saturating_mul(86_400_000)).unwrap_or(i64::MAX))
}

fn failed(error: rusqlite::Error) -> Refusal {
    format!("server error: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> Registry {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch(SCHEMA).unwrap();
        Registry { connection }
    }

    #[test]
    fn invitations_let_in_once() {
        let registry = registry();
        let token = crypto::from_hex(&registry.invite_user("max", 7).unwrap()).unwrap();
        assert_eq!(
            registry.redeem(&token, "ignored", Some("m1"), "d1"),
            Ok("max".into())
        );
        assert!(registry.redeem(&token, "max", Some("m1"), "d2").is_err());
        assert_eq!(
            registry.device_user("d1"),
            Ok(("max".into(), Some("m1".into())))
        );
        assert!(registry.device_user("d2").is_err());
        // Another device of the same member.
        assert_eq!(registry.link("m1", "d2"), Ok("max".into()));
        assert!(registry.link("m2", "d3").is_err());

        // Lost everything: the operator lets max in again; the device
        // doesn't know its member yet, which is kept.
        let again = crypto::from_hex(&registry.invite_user("max", 7).unwrap()).unwrap();
        registry.redeem(&again, "", None, "d4").unwrap();
        assert_eq!(
            registry.device_user("d4"),
            Ok(("max".into(), Some("m1".into())))
        );

        assert_eq!(registry.revoke("max").unwrap(), 3);
        assert!(registry.device_user("d1").is_err());
        assert_eq!(registry.list().unwrap(), [("max".to_string(), 0)]);
    }

    #[test]
    fn budget_invitations_need_a_free_username_in_time() {
        let registry = registry();
        let token = [7u8; 32];
        registry
            .invite_to_budget("b", &crypto::sha256(&[&token]), u64::MAX)
            .unwrap();
        let taken = crypto::from_hex(&registry.invite_user("max", 7).unwrap()).unwrap();
        registry.redeem(&taken, "", Some("m1"), "d1").unwrap();
        assert!(registry.redeem(&token, "max", Some("m2"), "d2").is_err());
        assert_eq!(
            registry.redeem(&token, "bob", Some("m2"), "d2"),
            Ok("bob".into())
        );

        let late = [8u8; 32];
        registry
            .invite_to_budget("b", &crypto::sha256(&[&late]), 0)
            .unwrap();
        assert!(registry.redeem(&late, "eve", Some("m3"), "d3").is_err());
    }
}
