//! The encrypted backup (ADR 0003): the member's identity, display name
//! and budget keys, locked by their recovery phrase, kept on the server.
//! With the phrase, a new device gets everything back after all the
//! member's devices are lost; without it, the server can't read it.
//!
//! The key stretched from the phrase is kept on the device, so the backup
//! can be refreshed (new budget keys) without asking for the phrase again.

use crate::bytes::{Reader, Writer};
use crate::crypto::{self, Cost};
use crate::{Corrupt, Store, StoreError};

/// The first bytes of a backup: what it is and its format's version.
const MAGIC: &str = "bougette backup 1";

impl Store {
    /// Locks the backup with `phrase` from now on, and returns it. `cost`
    /// is `crypto::RECOVERY_COST` except in tests.
    pub fn set_recovery(&mut self, phrase: &str, cost: Cost) -> Result<Vec<u8>, StoreError> {
        let salt: [u8; 16] = crypto::random();
        let key = crypto::stretch(phrase, &salt, cost);
        self.keep_recovery(&key, &salt, cost)?;
        Ok(self.backup()?.expect("the recovery key was just set"))
    }

    /// The current backup, if a recovery phrase was set.
    pub fn backup(&mut self) -> Result<Option<Vec<u8>>, StoreError> {
        let (Some(key), Some(salt), Some(cost)) = (
            self.setting("recovery-key")?,
            self.setting("recovery-salt")?,
            self.setting("recovery-cost")?,
        ) else {
            return Ok(None);
        };
        let corrupt = || Corrupt("invalid recovery settings".into());
        let key = crypto::from_hex_array::<32>(&key).ok_or_else(corrupt)?;
        let salt = crypto::from_hex(&salt).ok_or_else(corrupt)?;
        let (memory, passes) = cost.split_once(' ').ok_or_else(corrupt)?;

        // The content: identity seed, name, then every budget key.
        let mut content = Writer::default();
        content
            .bytes(&self.identity_seed()?)
            .optional(self.setting("name")?.as_deref());
        let mut statement = self
            .connection
            .prepare("SELECT budget, number, key FROM keys ORDER BY budget, number")?;
        let keys = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u32>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        content.u32(u32::try_from(keys.len()).expect("fewer than 4 billion keys"));
        for (budget, number, key) in &keys {
            content.text(budget).u32(*number).bytes(key);
        }

        let mut backup = Writer::default();
        backup
            .text(MAGIC)
            .u32(memory.parse().map_err(|_| corrupt())?)
            .u32(passes.parse().map_err(|_| corrupt())?)
            .bytes(&salt)
            .bytes(&crypto::encrypt(&key, MAGIC.as_bytes(), &content.0));
        Ok(Some(backup.0))
    }

    /// Restores a backup on this new device with the member's phrase:
    /// their identity, name and budget keys.
    pub fn restore(&mut self, backup: &[u8], phrase: &str) -> Result<(), StoreError> {
        let corrupt = || StoreError::Corrupt(Corrupt("not a Bougette backup".into()));
        let mut reader = Reader(backup);
        if reader.text().as_deref() != Some(MAGIC) {
            return Err(corrupt());
        }
        let cost = Cost {
            memory: reader.u32().ok_or_else(corrupt)?,
            passes: reader.u32().ok_or_else(corrupt)?,
        };
        let salt = reader.bytes().ok_or_else(corrupt)?.to_vec();
        let sealed = reader.bytes().ok_or_else(corrupt)?;
        let key = crypto::stretch(phrase, &salt, cost);
        let content = crypto::decrypt(&key, MAGIC.as_bytes(), sealed)
            .ok_or_else(|| StoreError::NotAllowed("wrong recovery phrase".into()))?;

        let mut reader = Reader(&content);
        let seed: [u8; 32] = reader
            .bytes()
            .and_then(|seed| seed.try_into().ok())
            .ok_or_else(corrupt)?;
        let name = reader.optional().ok_or_else(corrupt)?;
        self.adopt_identity(seed)?;
        if let Some(name) = name {
            self.set_setting("name", &name)?;
        }
        for _ in 0..reader.u32().ok_or_else(corrupt)? {
            let budget = reader.text().ok_or_else(corrupt)?;
            let number = reader.u32().ok_or_else(corrupt)?;
            let key: [u8; 32] = reader
                .bytes()
                .and_then(|key| key.try_into().ok())
                .ok_or_else(corrupt)?;
            self.add_key(&budget, number, &key)?;
        }
        self.keep_recovery(&key, &salt, cost)?;
        // The budgets already received become readable.
        for budget in self.budget_ids()? {
            self.rebuild(&budget)?;
        }
        Ok(())
    }

    fn keep_recovery(&mut self, key: &[u8; 32], salt: &[u8], cost: Cost) -> Result<(), StoreError> {
        self.set_setting("recovery-key", &crypto::hex(key))?;
        self.set_setting("recovery-salt", &crypto::hex(salt))?;
        self.set_setting("recovery-cost", &format!("{} {}", cost.memory, cost.passes))
    }
}

#[cfg(test)]
mod tests {
    use crate::crypto::Cost;
    use crate::{Store, StoreError, Versions};
    use bougette_core::budget::Budget;
    use bougette_core::money::Currency;

    const CHEAP: Cost = Cost {
        memory: 64,
        passes: 1,
    };

    #[test]
    fn a_new_device_gets_everything_back_with_the_phrase() {
        let mut lost = Store::open_in_memory().unwrap();
        lost.set_name("Alice").unwrap();
        let budget = lost.create(Budget::new("Home", Currency::EUR)).unwrap();
        assert!(lost.backup().unwrap().is_none());
        let backup = lost
            .set_recovery("correct horse battery staple", CHEAP)
            .unwrap();

        // The server passes the messages and the backup to a new device.
        let mut new = Store::open_in_memory().unwrap();
        let messages = lost.missing_for(&Versions::new(), |_| true).unwrap();
        new.receive(&messages).unwrap();
        assert!(new.load(budget.id).unwrap().is_none());
        assert!(matches!(
            new.restore(&backup, "wrong horse battery staple"),
            Err(StoreError::NotAllowed(_))
        ));
        new.restore(&backup, "correct  horse battery staple")
            .unwrap();
        assert_eq!(new.member().unwrap(), lost.member().unwrap());
        assert_eq!(new.setting("name").unwrap().as_deref(), Some("Alice"));
        assert_eq!(new.load(budget.id).unwrap(), Some(budget));
        // The restored device keeps the backup up to date by itself.
        assert!(new.backup().unwrap().is_some());
    }
}
