//! How version vectors, messages and requests travel as bytes. Plain and
//! explicit, with no serialization crate:
//! - a number: 4 bytes, big-endian (`u32`), or 8 for a time (`u64`);
//! - a byte string or text: its length (a number), then its bytes;
//! - an optional number: one byte, 0 for none, or 1 followed by it;
//! - a list: its length (a number), then its items.
//!
//! A version vector is a list of (budget, device, timestamp); a message is
//! its timestamp, budget, author and previous hash (texts), key number
//! (optional), body and signature (byte strings).

use bougette_store::{Message, Versions};
// The `...Ext` traits add helper methods (`read_u32`, `write_all`...) to
// every type implementing AsyncRead/AsyncWrite, like free functions found
// by argument-dependent lookup in C++, but opted into by `use`.
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::SyncError;

/// The longest byte string accepted, so a broken peer can't make us
/// allocate gigabytes. Fields are short; a backup is a few kilobytes.
const MAX_BYTES: u32 = 1 << 20;

pub async fn write_versions<W: AsyncWrite + Unpin>(
    writer: &mut W,
    versions: &Versions,
) -> Result<(), SyncError> {
    write_length(writer, versions.len()).await?;
    for ((budget, node), timestamp) in versions {
        write_text(writer, budget).await?;
        write_text(writer, node).await?;
        write_text(writer, timestamp).await?;
    }
    Ok(())
}

pub async fn read_versions<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Versions, SyncError> {
    let count = reader.read_u32().await?;
    let mut versions = Versions::new();
    for _ in 0..count {
        let budget = read_text(reader).await?;
        let node = read_text(reader).await?;
        let timestamp = read_text(reader).await?;
        versions.insert((budget, node), timestamp);
    }
    Ok(versions)
}

pub async fn write_messages<W: AsyncWrite + Unpin>(
    writer: &mut W,
    messages: &[Message],
) -> Result<(), SyncError> {
    write_length(writer, messages.len()).await?;
    for message in messages {
        write_text(writer, &message.timestamp).await?;
        write_text(writer, &message.budget).await?;
        write_text(writer, &message.author).await?;
        write_text(writer, &message.previous).await?;
        let (key, body) = message.stored_body();
        match key {
            None => writer.write_u8(0).await?,
            Some(key) => {
                writer.write_u8(1).await?;
                writer.write_u32(key).await?;
            }
        }
        write_bytes(writer, &body).await?;
        write_bytes(writer, &message.signature).await?;
    }
    Ok(())
}

pub async fn read_messages<R: AsyncRead + Unpin>(
    reader: &mut R,
) -> Result<Vec<Message>, SyncError> {
    let count = reader.read_u32().await?;
    // No `Vec::with_capacity(count)`: `count` comes from the peer, and
    // memory should only grow with what actually arrives.
    let mut messages = Vec::new();
    for _ in 0..count {
        let timestamp = read_text(reader).await?;
        let budget = read_text(reader).await?;
        let author = read_text(reader).await?;
        let previous = read_text(reader).await?;
        let key = match reader.read_u8().await? {
            0 => None,
            1 => Some(reader.read_u32().await?),
            other => return Err(SyncError::Protocol(format!("bad option tag {other}"))),
        };
        let body = Message::body_from(key, read_bytes(reader).await?)
            .ok_or_else(|| SyncError::Protocol("bad message body".to_string()))?;
        messages.push(Message {
            timestamp,
            budget,
            author,
            previous,
            body,
            signature: read_bytes(reader).await?,
        });
    }
    Ok(messages)
}

/// A length as a 4-byte number; more than 4 billion items is a bug.
pub async fn write_length<W: AsyncWrite + Unpin>(
    writer: &mut W,
    length: usize,
) -> Result<(), SyncError> {
    let length = u32::try_from(length).expect("fewer than 2^32 items");
    writer.write_u32(length).await?;
    Ok(())
}

pub async fn write_bytes<W: AsyncWrite + Unpin>(
    writer: &mut W,
    bytes: &[u8],
) -> Result<(), SyncError> {
    write_length(writer, bytes.len()).await?;
    writer.write_all(bytes).await?;
    Ok(())
}

pub async fn read_bytes<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Vec<u8>, SyncError> {
    let length = reader.read_u32().await?;
    if length > MAX_BYTES {
        return Err(SyncError::Protocol(format!("{length} bytes at once")));
    }
    // `vec![0; n]`: n zero bytes, filled by `read_exact`.
    let mut bytes = vec![0; length as usize];
    reader.read_exact(&mut bytes).await?;
    Ok(bytes)
}

pub async fn write_text<W: AsyncWrite + Unpin>(
    writer: &mut W,
    text: &str,
) -> Result<(), SyncError> {
    write_bytes(writer, text.as_bytes()).await
}

pub async fn read_text<R: AsyncRead + Unpin>(reader: &mut R) -> Result<String, SyncError> {
    String::from_utf8(read_bytes(reader).await?)
        .map_err(|_| SyncError::Protocol("text is not UTF-8".to_string()))
}
