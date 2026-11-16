//! The iroh transport (ADR 0001): QUIC connections between devices
//! addressed by their public key, carrying the protocol of this crate.
//!
//! No relay and no address lookup: a device connects to another given its
//! address, so the app contacts no third party. Each device has a key
//! pair; its public key is its identifier, and the TLS handshake proves
//! both ends hold their keys. Until members and invitations (M2), a
//! device serving accepts only the identifiers it was told to allow.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use bougette_store::{Corrupt, Store, StoreError};
use iroh::endpoint::{Incoming, presets};
use iroh::{Endpoint, EndpointAddr, EndpointId, SecretKey};

use crate::{Summary, SyncError, initiate, respond};

/// The protocol's name and version, agreed during the QUIC handshake
/// (ALPN): a peer speaking another version is refused before any byte.
pub const ALPN: &[u8] = b"bougette/sync/1";

/// Where the device's secret key is kept, among the store's settings.
const SECRET_KEY: &str = "iroh-secret-key";

/// What one exchange answered by `serve` gave: who, and how it went.
pub type Answered = Result<(EndpointId, Summary), SyncError>;

/// This device's secret key: read from the store's settings, or drawn and
/// saved the first time.
pub fn secret_key(store: &mut Store) -> Result<SecretKey, SyncError> {
    if let Some(hex) = store.setting(SECRET_KEY)? {
        // `try_into` turns the Vec into a `[u8; 32]`, failing if the
        // length differs.
        let bytes: [u8; 32] = from_hex(&hex)
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or_else(|| StoreError::Corrupt(Corrupt("invalid secret key".to_string())))?;
        return Ok(SecretKey::from_bytes(&bytes));
    }
    let key = SecretKey::generate();
    store.set_setting(SECRET_KEY, &to_hex(&key.to_bytes()))?;
    Ok(key)
}

/// An endpoint for `key`, on UDP port `port` of every interface, or on
/// any free port if `port` is 0.
pub async fn endpoint(key: SecretKey, port: u16) -> Result<Endpoint, SyncError> {
    let mut builder = Endpoint::builder(presets::Minimal)
        .secret_key(key)
        .alpns(vec![ALPN.to_vec()]);
    if port != 0 {
        builder = builder
            .clear_ip_transports()
            .bind_addr(SocketAddr::from(([0, 0, 0, 0], port)))
            .map_err(network)?;
    }
    builder.bind().await.map_err(network)
}

/// The text to give another device so it can reach this endpoint:
/// `ID@IP:PORT,IP:PORT...`.
pub fn address(endpoint: &Endpoint) -> String {
    let addr = endpoint.addr();
    let ips: Vec<String> = addr.ip_addrs().map(|ip| ip.to_string()).collect();
    format!("{}@{}", addr.id, ips.join(","))
}

/// Reads an address written by `address`.
pub fn parse_address(text: &str) -> Result<EndpointAddr, String> {
    let (id, ips) = text
        .split_once('@')
        .ok_or_else(|| format!("not an address (ID@IP:PORT): {text}"))?;
    let mut addr = EndpointAddr::new(parse_id(id)?);
    for ip in ips.split(',') {
        let ip: SocketAddr = ip.parse().map_err(|_| format!("not IP:PORT: {ip}"))?;
        addr = addr.with_ip_addr(ip);
    }
    Ok(addr)
}

/// Reads a device identifier: its public key, in hexadecimal.
pub fn parse_id(text: &str) -> Result<EndpointId, String> {
    text.parse()
        .map_err(|_| format!("not a device identifier: {text}"))
}

/// Syncs with the device at `peer`, which must be serving.
pub async fn sync_with(
    endpoint: &Endpoint,
    store: &Mutex<Store>,
    peer: EndpointAddr,
) -> Result<Summary, SyncError> {
    let connection = endpoint.connect(peer, ALPN).await.map_err(network)?;
    let (mut send, mut recv) = connection.open_bi().await.map_err(network)?;
    // `&mut recv` and `&mut send` are streams too: tokio implements its
    // traits for mutable references to streams, so we keep ownership.
    let summary = initiate(store, &mut recv, &mut send).await?;
    // The peer closes the connection once it has read our messages.
    connection.closed().await;
    Ok(summary)
}

/// Answers the devices in `allowed`, forever; refuses the others. Calls
/// `report` after each exchange.
// `fn(Answered)` is a plain function pointer, like `void (*)(Answered)`:
// copyable and safe to share between tasks.
pub async fn serve(
    endpoint: Endpoint,
    store: Arc<Mutex<Store>>,
    allowed: Vec<EndpointId>,
    report: fn(Answered),
) {
    // `Arc` is a thread-safe reference count, like `std::shared_ptr`: each
    // task gets its own pointer to the same allow-list and store.
    let allowed = Arc::new(allowed);
    while let Some(incoming) = endpoint.accept().await {
        let store = Arc::clone(&store);
        let allowed = Arc::clone(&allowed);
        // A new task per connection, so a slow device doesn't hold up the
        // others. `async move` gives the task its own `store` and
        // `allowed`; a task must own what it uses, as it may outlive this
        // loop iteration (like a detached std::thread taking copies).
        tokio::spawn(async move {
            report(answer(incoming, &store, &allowed).await);
        });
    }
}

/// One exchange with a device that connected.
async fn answer(incoming: Incoming, store: &Mutex<Store>, allowed: &[EndpointId]) -> Answered {
    let connection = incoming.await.map_err(network)?;
    let peer = connection.remote_id();
    if !allowed.contains(&peer) {
        // QUIC application error code 1, our "not allowed".
        connection.close(1u32.into(), b"not allowed");
        return Err(SyncError::NotAllowed(peer.to_string()));
    }
    let (mut send, mut recv) = connection.accept_bi().await.map_err(network)?;
    let summary = respond(store, &mut recv, &mut send).await?;
    // Everything was read: closing tells the peer we are done.
    connection.close(0u32.into(), b"done");
    Ok((peer, summary))
}

/// Any network error, as text: the caller only reports it.
// `impl Display`: iroh has many error types; anything printable will do.
fn network(error: impl std::fmt::Display) -> SyncError {
    SyncError::Network(error.to_string())
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn from_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || !text.is_ascii() {
        return None;
    }
    // `step_by(2)`: every other index, one per byte.
    (0..text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&text[index..index + 2], 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_secret_key_is_kept() {
        let mut store = Store::open_in_memory().unwrap();
        let first = secret_key(&mut store).unwrap();
        let again = secret_key(&mut store).unwrap();
        assert_eq!(first.to_bytes(), again.to_bytes());
    }

    #[test]
    fn addresses_read_back() {
        let id = SecretKey::generate().public();
        let text = format!("{id}@127.0.0.1:4433,[::1]:4433");
        let addr = parse_address(&text).unwrap();
        assert_eq!(addr.id, id);
        assert_eq!(addr.ip_addrs().count(), 2);
        assert!(parse_address("nothing").is_err());
        assert!(parse_address(&format!("{id}@localhost")).is_err());
    }

    // A real exchange over QUIC on this machine, through the loopback.
    #[tokio::test]
    async fn two_devices_sync_over_iroh() {
        let laptop = Mutex::new(Store::open_in_memory().unwrap());
        let mut phone = Store::open_in_memory().unwrap();
        let budget =
            bougette_core::budget::Budget::new("Courses", bougette_core::money::Currency::EUR);
        phone.save(&budget).unwrap();
        let phone_key = secret_key(&mut phone).unwrap();
        let phone_id = phone_key.public();
        let laptop_key = SecretKey::generate();
        let laptop_id = laptop_key.public();

        let server = endpoint(phone_key, 0).await.unwrap();
        let port = server.bound_sockets()[0].port();
        let phone_address =
            EndpointAddr::new(phone_id).with_ip_addr(SocketAddr::from(([127, 0, 0, 1], port)));
        let phone = Arc::new(Mutex::new(phone));
        // The server runs in the background for the rest of the test.
        tokio::spawn(serve(server, Arc::clone(&phone), vec![laptop_id], |_| {}));

        let client = endpoint(laptop_key, 0).await.unwrap();
        let summary = sync_with(&client, &laptop, phone_address.clone())
            .await
            .unwrap();
        assert_eq!(
            summary,
            Summary {
                sent: 0,
                received: 4
            }
        );
        assert_eq!(
            laptop.lock().unwrap().load(budget.id).unwrap(),
            Some(budget)
        );

        // A device that isn't allowed gets nothing.
        let stranger = endpoint(SecretKey::generate(), 0).await.unwrap();
        let empty = Mutex::new(Store::open_in_memory().unwrap());
        assert!(sync_with(&stranger, &empty, phone_address).await.is_err());
        assert!(empty.lock().unwrap().versions().unwrap().is_empty());
    }
}
