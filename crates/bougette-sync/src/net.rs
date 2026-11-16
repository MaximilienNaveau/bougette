//! The iroh transport (ADR 0001): QUIC connections between devices
//! addressed by their public key, carrying requests.
//!
//! No relay and no address lookup: a device connects to another given its
//! address, so the app contacts no third party. Each device has a key
//! pair; its public key is its identifier, and the TLS handshake proves
//! both ends hold their keys.
//!
//! A connection carries one request (`Request`) and its reply: a status
//! byte (0 accepted, 1 refused), then the reply's bytes or the reason. A
//! sync goes on with the protocol of this crate once accepted. What is
//! accepted is up to the side answering (`Responder`): another device of
//! the same member, or the server (bougette-server).

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bougette_store::{Corrupt, Store, StoreError, device_belongs};
use iroh::endpoint::{Incoming, presets};
use iroh::{Endpoint, EndpointAddr, EndpointId, SecretKey};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::wire::{read_bytes, read_text, write_bytes, write_text};
use crate::{Scope, Summary, SyncError, initiate, lock, respond};

/// The protocol's name and version, agreed during the QUIC handshake
/// (ALPN): a peer speaking another version is refused before any byte.
pub const ALPN: &[u8] = b"bougette/2";

/// Where the device's network secret key is kept, among the settings.
const SECRET_KEY: &str = "iroh-secret-key";

/// What a device asks another device or a server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// To sync, as a device of `member`: `proof` is the member's signature
    /// of the device's identifier (`Store::prove_device`).
    Sync { member: String, proof: Vec<u8> },
    /// To be let into a server with an invitation's token, as `username`;
    /// with the member (and proof) the device belongs to, or none when
    /// recovering (the member's identity comes back with the backup).
    Redeem {
        token: Vec<u8>,
        username: String,
        member: Option<(String, Vec<u8>)>,
    },
    /// To be let into a server as another device of a member already there.
    Link { member: String, proof: Vec<u8> },
    /// To register an invitation to `budget` on the server: whoever shows
    /// a token hashing to `token_hash` before `expires` (milliseconds since
    /// 1970) may come in. Only from an Admin of the budget.
    Invite {
        budget: String,
        token_hash: Vec<u8>,
        expires: u64,
    },
    /// To keep the member's encrypted backup on the server.
    PutBackup(Vec<u8>),
    /// To get it back.
    GetBackup,
}

/// What answers requests: another device of the member, or the server.
// A trait: like an abstract base class with pure virtual functions.
// `Send + Sync + 'static`: an implementation can be shared between tasks
// and lives as long as needed (owns its data).
pub trait Responder: Send + Sync + 'static {
    fn store(&self) -> &Mutex<Store>;
    /// The budgets a device, proven to belong to `member`, may receive;
    /// the reason as an error to refuse it.
    fn scope(&self, device: &EndpointId, member: &str) -> Result<Scope, String>;
    /// Any request but a sync: the reply, or the reason to refuse.
    fn answer(&self, device: &EndpointId, request: Request) -> Result<Vec<u8>, String>;
}

/// Answers the other devices of the member of `store`: they may sync,
/// nothing else.
pub struct OwnDevices {
    pub store: Mutex<Store>,
    pub member: String,
}

impl Responder for OwnDevices {
    fn store(&self) -> &Mutex<Store> {
        &self.store
    }

    fn scope(&self, _device: &EndpointId, member: &str) -> Result<Scope, String> {
        if member == self.member {
            Ok(Scope::All)
        } else {
            Err("not a device of this member".to_string())
        }
    }

    fn answer(&self, _device: &EndpointId, _request: Request) -> Result<Vec<u8>, String> {
        Err("a device only syncs".to_string())
    }
}

/// This device's network secret key: read from the store's settings, or
/// drawn and saved the first time.
pub fn secret_key(store: &mut Store) -> Result<SecretKey, SyncError> {
    if let Some(hex) = store.setting(SECRET_KEY)? {
        let bytes = bougette_store::crypto::from_hex_array::<32>(&hex)
            .ok_or_else(|| StoreError::Corrupt(Corrupt("invalid secret key".to_string())))?;
        return Ok(SecretKey::from_bytes(&bytes));
    }
    let key = SecretKey::generate();
    store.set_setting(SECRET_KEY, &bougette_store::crypto::hex(&key.to_bytes()))?;
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

/// Syncs with the device or server at `peer`.
pub async fn sync_with(
    endpoint: &Endpoint,
    store: &Mutex<Store>,
    peer: EndpointAddr,
) -> Result<Summary, SyncError> {
    let (member, proof) = lock(store).prove_device(&endpoint.id().to_string())?;
    let connection = endpoint.connect(peer, ALPN).await.map_err(network)?;
    let (mut send, mut recv) = connection.open_bi().await.map_err(network)?;
    write_request(&mut send, &Request::Sync { member, proof }).await?;
    send.flush().await?;
    if let Err(error) = read_reply(&mut recv).await {
        // Refused: we read last, so we close.
        connection.close(0u32.into(), b"refused");
        return Err(error);
    }
    // `&mut recv` and `&mut send` are streams too: tokio implements its
    // traits for mutable references to streams, so we keep ownership.
    let summary = initiate(store, &mut recv, &mut send).await?;
    // The peer closes the connection once it has read our messages.
    connection.closed().await;
    Ok(summary)
}

/// Sends `request` (not a sync) to `peer` and returns the reply.
pub async fn call(
    endpoint: &Endpoint,
    peer: EndpointAddr,
    request: &Request,
) -> Result<Vec<u8>, SyncError> {
    let connection = endpoint.connect(peer, ALPN).await.map_err(network)?;
    let (mut send, mut recv) = connection.open_bi().await.map_err(network)?;
    write_request(&mut send, request).await?;
    send.flush().await?;
    let reply = read_reply(&mut recv).await;
    // We read last: closing tells the other side it may forget us.
    connection.close(0u32.into(), b"thanks");
    reply
}

/// Answers every connection with `responder`, forever. Calls `report`
/// with a line about each one.
// `fn(...)` is a plain function pointer, like `void (*)(...)`: copyable
// and safe to share between tasks. `R: Responder`: any type implementing
// the trait, like a template parameter constrained by a concept.
pub async fn serve<R: Responder>(
    endpoint: Endpoint,
    responder: Arc<R>,
    report: fn(Result<String, String>),
) {
    while let Some(incoming) = endpoint.accept().await {
        // `Arc` is a thread-safe reference count, like `std::shared_ptr`:
        // each task gets its own pointer to the same responder.
        let responder = Arc::clone(&responder);
        // A new task per connection, so a slow device doesn't hold up the
        // others. `async move` gives the task its own `responder`; a task
        // must own what it uses, as it may outlive this loop iteration
        // (like a detached std::thread taking copies).
        tokio::spawn(async move {
            report(answer_connection(incoming, &*responder).await);
        });
    }
}

/// One connection: read the request, answer it, close.
async fn answer_connection<R: Responder>(
    incoming: Incoming,
    responder: &R,
) -> Result<String, String> {
    let connection = incoming.await.map_err(|error| error.to_string())?;
    let peer = connection.remote_id();
    let short = peer.to_string()[..8].to_string();
    // An `async` block run at once: a `?` inside stops the block, not the
    // function, so the connection is closed below either way.
    let outcome = async {
        let (mut send, mut recv) = connection.accept_bi().await.map_err(network)?;
        let request = read_request(&mut recv).await?;
        // Who reads last closes the connection: closing drops what the
        // other side hasn't received yet. After a sync, that's us; after a
        // reply, the other side, so we finish our stream and wait.
        let mut they_close = true;
        let line = match request {
            Request::Sync { member, proof } => {
                let scope = if device_belongs(&member, &peer.to_string(), &proof) {
                    responder.scope(&peer, &member)
                } else {
                    Err("the proof of membership is wrong".to_string())
                };
                match scope {
                    Ok(scope) => {
                        write_reply(&mut send, Ok(Vec::new())).await?;
                        let summary =
                            respond(responder.store(), &mut recv, &mut send, &scope).await?;
                        they_close = false;
                        format!(
                            "{short}: synced, sent {}, received {} new",
                            summary.sent, summary.received
                        )
                    }
                    Err(reason) => {
                        write_reply(&mut send, Err(reason.clone())).await?;
                        format!("{short}: refused to sync: {reason}")
                    }
                }
            }
            other => {
                let name = request_name(&other);
                let reply = responder.answer(&peer, other);
                let line = match &reply {
                    Ok(_) => format!("{short}: {name}"),
                    Err(reason) => format!("{short}: refused to {name}: {reason}"),
                };
                write_reply(&mut send, reply).await?;
                line
            }
        };
        send.flush().await?;
        if they_close {
            send.finish().map_err(network)?;
        }
        Ok::<_, SyncError>((line, they_close))
    }
    .await;
    match outcome {
        Ok((line, true)) => {
            // At most 10 seconds, in case the other side never closes.
            let _ = tokio::time::timeout(Duration::from_secs(10), connection.closed()).await;
            Ok(line)
        }
        Ok((line, false)) => {
            connection.close(0u32.into(), b"done");
            Ok(line)
        }
        Err(error) => {
            connection.close(1u32.into(), b"error");
            Err(format!("{short}: {error:?}"))
        }
    }
}

fn request_name(request: &Request) -> &'static str {
    match request {
        Request::Sync { .. } => "sync",
        Request::Redeem { .. } => "join the server",
        Request::Link { .. } => "link a device",
        Request::Invite { .. } => "register an invitation",
        Request::PutBackup(_) => "keep a backup",
        Request::GetBackup => "fetch a backup",
    }
}

async fn write_request<W: AsyncWrite + Unpin>(
    writer: &mut W,
    request: &Request,
) -> Result<(), SyncError> {
    match request {
        Request::Sync { member, proof } => {
            writer.write_u8(1).await?;
            write_text(writer, member).await?;
            write_bytes(writer, proof).await?;
        }
        Request::Redeem {
            token,
            username,
            member,
        } => {
            writer.write_u8(2).await?;
            write_bytes(writer, token).await?;
            write_text(writer, username).await?;
            match member {
                None => writer.write_u8(0).await?,
                Some((member, proof)) => {
                    writer.write_u8(1).await?;
                    write_text(writer, member).await?;
                    write_bytes(writer, proof).await?;
                }
            }
        }
        Request::Link { member, proof } => {
            writer.write_u8(3).await?;
            write_text(writer, member).await?;
            write_bytes(writer, proof).await?;
        }
        Request::Invite {
            budget,
            token_hash,
            expires,
        } => {
            writer.write_u8(4).await?;
            write_text(writer, budget).await?;
            write_bytes(writer, token_hash).await?;
            writer.write_u64(*expires).await?;
        }
        Request::PutBackup(backup) => {
            writer.write_u8(5).await?;
            write_bytes(writer, backup).await?;
        }
        Request::GetBackup => writer.write_u8(6).await?,
    }
    Ok(())
}

async fn read_request<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Request, SyncError> {
    Ok(match reader.read_u8().await? {
        1 => Request::Sync {
            member: read_text(reader).await?,
            proof: read_bytes(reader).await?,
        },
        2 => Request::Redeem {
            token: read_bytes(reader).await?,
            username: read_text(reader).await?,
            member: match reader.read_u8().await? {
                0 => None,
                _ => Some((read_text(reader).await?, read_bytes(reader).await?)),
            },
        },
        3 => Request::Link {
            member: read_text(reader).await?,
            proof: read_bytes(reader).await?,
        },
        4 => Request::Invite {
            budget: read_text(reader).await?,
            token_hash: read_bytes(reader).await?,
            expires: reader.read_u64().await?,
        },
        5 => Request::PutBackup(read_bytes(reader).await?),
        6 => Request::GetBackup,
        other => return Err(SyncError::Protocol(format!("unknown request {other}"))),
    })
}

async fn write_reply<W: AsyncWrite + Unpin>(
    writer: &mut W,
    reply: Result<Vec<u8>, String>,
) -> Result<(), SyncError> {
    match reply {
        Ok(bytes) => {
            writer.write_u8(0).await?;
            write_bytes(writer, &bytes).await
        }
        Err(reason) => {
            writer.write_u8(1).await?;
            write_text(writer, &reason).await
        }
    }
}

async fn read_reply<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Vec<u8>, SyncError> {
    match reader.read_u8().await? {
        0 => read_bytes(reader).await,
        _ => Err(SyncError::Refused(read_text(reader).await?)),
    }
}

/// Any network error, as text: the caller only reports it.
// `impl Display`: iroh has many error types; anything printable will do.
fn network(error: impl std::fmt::Display) -> SyncError {
    SyncError::Network(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bougette_core::budget::Budget;
    use bougette_core::money::Currency;

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

    #[tokio::test]
    async fn requests_read_back() {
        let requests = [
            Request::Sync {
                member: "m".into(),
                proof: vec![1, 2],
            },
            Request::Redeem {
                token: vec![3],
                username: "max".into(),
                member: None,
            },
            Request::Redeem {
                token: vec![3],
                username: "max".into(),
                member: Some(("m".into(), vec![4])),
            },
            Request::Link {
                member: "m".into(),
                proof: vec![],
            },
            Request::Invite {
                budget: "b".into(),
                token_hash: vec![5; 32],
                expires: 1789,
            },
            Request::PutBackup(vec![6; 10]),
            Request::GetBackup,
        ];
        for request in requests {
            let mut bytes = Vec::new();
            write_request(&mut bytes, &request).await.unwrap();
            assert_eq!(read_request(&mut &bytes[..]).await.unwrap(), request);
        }
    }

    // A real exchange over QUIC on this machine, through the loopback.
    #[tokio::test]
    async fn two_devices_of_a_member_sync_over_iroh() {
        let mut phone = Store::open_in_memory().unwrap();
        let budget = phone.create(Budget::new("Courses", Currency::EUR)).unwrap();
        let member = phone.member().unwrap();
        let mut laptop = Store::open_in_memory().unwrap();
        laptop
            .adopt_identity(phone.identity_seed().unwrap())
            .unwrap();
        let laptop = Mutex::new(laptop);

        let server = endpoint(secret_key(&mut phone).unwrap(), 0).await.unwrap();
        let port = server.bound_sockets()[0].port();
        let phone_address =
            EndpointAddr::new(server.id()).with_ip_addr(SocketAddr::from(([127, 0, 0, 1], port)));
        let responder = Arc::new(OwnDevices {
            store: Mutex::new(phone),
            member,
        });
        // The phone serves in the background for the rest of the test.
        tokio::spawn(serve(server, responder, |_| {}));

        let client = endpoint(SecretKey::generate(), 0).await.unwrap();
        let summary = sync_with(&client, &laptop, phone_address.clone())
            .await
            .unwrap();
        assert!(summary.received > 0);
        assert_eq!(lock(&laptop).load(budget.id).unwrap(), Some(budget));

        // Another member's device is refused, with the reason.
        let stranger = endpoint(SecretKey::generate(), 0).await.unwrap();
        let empty = Mutex::new(Store::open_in_memory().unwrap());
        let refused = sync_with(&stranger, &empty, phone_address.clone()).await;
        assert!(matches!(refused, Err(SyncError::Refused(_))));
        assert!(lock(&empty).versions().unwrap().is_empty());
        // And a device answers no other request.
        let other = call(&stranger, phone_address, &Request::GetBackup).await;
        assert!(matches!(other, Err(SyncError::Refused(_))));
    }
}
