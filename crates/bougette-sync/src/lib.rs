//! The sync protocol: how two stores send each other what they lack, over
//! any pair of byte streams: an iroh connection, or an in-memory pipe in
//! the tests. It knows nothing of the network (ADR 0001), and finding what
//! is missing is the store's job (ADR 0004).
//!
//! The exchange, started by the initiator:
//! 1. initiator → responder: its version vector;
//! 2. responder → initiator: its version vector, then the messages the
//!    initiator lacks;
//! 3. initiator → responder: the messages the responder lacks.
//!
//! The two sides take turns: one writes while the other reads, so neither
//! can block because both are writing into full buffers.
//!
//! About `async`: an `async fn` returns a *future*, a value describing the
//! work, which runs only when something `.await`s it. At each `.await` the
//! task may pause (waiting for the network) and let others run on the same
//! thread. Unlike C++20 coroutines, Rust ships no scheduler: a runtime
//! (here tokio) drives the futures. There is no direct C++ equivalent.

pub mod net;
mod wire;

use std::sync::{Mutex, MutexGuard, PoisonError};

use bougette_store::{Store, StoreError};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

/// Why an exchange failed.
#[derive(Debug)]
pub enum SyncError {
    /// The connection failed or closed early.
    Io(std::io::Error),
    /// The local store failed, or refused what the peer sent.
    Store(StoreError),
    /// The peer sent something that isn't the protocol.
    Protocol(String),
    /// The network layer failed: no route to the peer, refused handshake...
    Network(String),
    /// A device that isn't allowed to sync with us tried to.
    NotAllowed(String),
}

impl From<std::io::Error> for SyncError {
    fn from(error: std::io::Error) -> Self {
        SyncError::Io(error)
    }
}

impl From<StoreError> for SyncError {
    fn from(error: StoreError) -> Self {
        SyncError::Store(error)
    }
}

/// What an exchange did: messages sent, and new messages received.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    pub sent: usize,
    pub received: usize,
}

// The store sits behind a Mutex (like `std::mutex` *wrapping* the data:
// the only way to reach the Store is through the lock), because a server
// syncs with several devices at once. Each use below locks it for one
// statement and never across an `.await`, so a slow peer can't hold the
// store.

/// Syncs with a peer, starting the exchange.
// Generic over the stream types, constrained by traits after `where`:
// like a template with concepts. `Unpin` is a technical requirement of
// the helper methods: the stream may be moved in memory between awaits.
pub async fn initiate<R, W>(
    store: &Mutex<Store>,
    mut reader: R,
    mut writer: W,
) -> Result<Summary, SyncError>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let ours = lock(store).versions()?;
    wire::write_versions(&mut writer, &ours).await?;
    writer.flush().await?;

    let theirs = wire::read_versions(&mut reader).await?;
    let incoming = wire::read_messages(&mut reader).await?;
    let received = lock(store).receive(&incoming)?;

    // What they lack, computed after receiving: what they just sent is
    // covered by their version vector, so it doesn't go back.
    let outgoing = lock(store).missing_for(&theirs)?;
    wire::write_messages(&mut writer, &outgoing).await?;
    writer.flush().await?;
    Ok(Summary {
        sent: outgoing.len(),
        received,
    })
}

/// Syncs with a peer that started the exchange.
pub async fn respond<R, W>(
    store: &Mutex<Store>,
    mut reader: R,
    mut writer: W,
) -> Result<Summary, SyncError>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let theirs = wire::read_versions(&mut reader).await?;
    // One lock for both reads, released at the end of the block (`}`).
    let (ours, outgoing) = {
        let store = lock(store);
        (store.versions()?, store.missing_for(&theirs)?)
    };
    wire::write_versions(&mut writer, &ours).await?;
    wire::write_messages(&mut writer, &outgoing).await?;
    writer.flush().await?;

    let incoming = wire::read_messages(&mut reader).await?;
    let received = lock(store).receive(&incoming)?;
    Ok(Summary {
        sent: outgoing.len(),
        received,
    })
}

/// Locks the store. If another task panicked while holding it, the lock
/// is "poisoned"; the store is still consistent (SQLite rolled back any
/// unfinished transaction), so carry on with it.
fn lock(store: &Mutex<Store>) -> MutexGuard<'_, Store> {
    store.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bougette_core::budget::Budget;
    use bougette_core::money::Currency;

    /// Syncs two stores through an in-memory pipe, `a` initiating.
    async fn sync(a: &Mutex<Store>, b: &Mutex<Store>) -> (Summary, Summary) {
        // `duplex` makes two connected ends, each readable and writable;
        // `split` cuts an end into its reading and writing halves.
        let (end_a, end_b) = tokio::io::duplex(64);
        let (read_a, write_a) = tokio::io::split(end_a);
        let (read_b, write_b) = tokio::io::split(end_b);
        // `join!` runs both futures concurrently and waits for both.
        let (a, b) = tokio::join!(initiate(a, read_a, write_a), respond(b, read_b, write_b));
        (a.unwrap(), b.unwrap())
    }

    /// A store holding one new budget named `name`.
    fn store_with(name: &str) -> (Mutex<Store>, Budget) {
        let mut store = Store::open_in_memory().unwrap();
        let budget = Budget::new(name, Currency::EUR);
        store.save(&budget).unwrap();
        (Mutex::new(store), budget)
    }

    // `#[tokio::test]` wraps the async test in a runtime that drives it.
    #[tokio::test]
    async fn both_sides_get_what_they_lack() {
        let (laptop, groceries) = store_with("Courses");
        let (phone, trip) = store_with("Voyage à Lyon");
        let (from_laptop, from_phone) = sync(&laptop, &phone).await;
        // Each budget is 4 messages: name, currency, personal, deleted.
        assert_eq!(
            from_laptop,
            Summary {
                sent: 4,
                received: 4
            }
        );
        assert_eq!(
            from_phone,
            Summary {
                sent: 4,
                received: 4
            }
        );
        for store in [&laptop, &phone] {
            let store = lock(store);
            assert_eq!(store.load(groceries.id).unwrap(), Some(groceries.clone()));
            assert_eq!(store.load(trip.id).unwrap(), Some(trip.clone()));
        }
        // Once in sync, nothing travels.
        let (again, _) = sync(&phone, &laptop).await;
        assert_eq!(
            again,
            Summary {
                sent: 0,
                received: 0
            }
        );
    }

    // The server as a mailbox: the laptop and the phone are never online
    // together, yet the phone gets what the laptop wrote.
    #[tokio::test]
    async fn the_server_passes_changes_on() {
        let (laptop, groceries) = store_with("Courses");
        let (phone, _) = store_with("Voyage");
        let server = Mutex::new(Store::open_in_memory().unwrap());
        sync(&laptop, &server).await;
        sync(&phone, &server).await;
        assert_eq!(lock(&phone).load(groceries.id).unwrap(), Some(groceries));
        // And the laptop gets the phone's budget on its next sync.
        let (back, _) = sync(&laptop, &server).await;
        assert_eq!(
            back,
            Summary {
                sent: 0,
                received: 4
            }
        );
        assert_eq!(
            lock(&laptop).versions().unwrap(),
            lock(&phone).versions().unwrap()
        );
    }

    #[tokio::test]
    async fn refuses_garbage() {
        let (store, _) = store_with("Courses");
        // A version vector announcing one entry, whose text is 4 GiB.
        let garbage: &[u8] = &[0, 0, 0, 1, 0xff, 0xff, 0xff, 0xff];
        let result = respond(&store, garbage, tokio::io::sink()).await;
        assert!(matches!(result, Err(SyncError::Protocol(_))));
    }
}
