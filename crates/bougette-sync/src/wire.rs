//! How version vectors and messages travel as bytes. Plain and explicit,
//! with no serialization crate:
//! - a number: 4 bytes, big-endian (`u32`);
//! - a text: its length in bytes (a number), then its UTF-8 bytes;
//! - an optional text: one byte, 0 for none, or 1 followed by the text;
//! - a list: its length (a number), then its items.
//!
//! A version vector is a list of (device, timestamp) pairs; a message is
//! its timestamp, budget, entity, id and field (texts), then its value (an
//! optional text).

use bougette_store::{Message, Versions};
// The `...Ext` traits add helper methods (`read_u32`, `write_all`...) to
// every type implementing AsyncRead/AsyncWrite, like free functions found
// by argument-dependent lookup in C++, but opted into by `use`.
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::SyncError;

/// The longest text accepted, so a broken peer can't make us allocate
/// gigabytes. Fields are short: names, notes, identifiers.
const MAX_TEXT: u32 = 1 << 20;

pub async fn write_versions<W: AsyncWrite + Unpin>(
    writer: &mut W,
    versions: &Versions,
) -> Result<(), SyncError> {
    write_length(writer, versions.len()).await?;
    for (node, timestamp) in versions {
        write_text(writer, node).await?;
        write_text(writer, timestamp).await?;
    }
    Ok(())
}

pub async fn read_versions<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Versions, SyncError> {
    let count = reader.read_u32().await?;
    let mut versions = Versions::new();
    for _ in 0..count {
        let node = read_text(reader).await?;
        let timestamp = read_text(reader).await?;
        versions.insert(node, timestamp);
    }
    Ok(versions)
}

pub async fn write_messages<W: AsyncWrite + Unpin>(
    writer: &mut W,
    messages: &[Message],
) -> Result<(), SyncError> {
    write_length(writer, messages.len()).await?;
    for message in messages {
        for text in [
            &message.timestamp,
            &message.budget,
            &message.entity,
            &message.id,
            &message.field,
        ] {
            write_text(writer, text).await?;
        }
        match &message.value {
            None => writer.write_u8(0).await?,
            Some(value) => {
                writer.write_u8(1).await?;
                write_text(writer, value).await?;
            }
        }
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
        messages.push(Message {
            timestamp: read_text(reader).await?,
            budget: read_text(reader).await?,
            entity: read_text(reader).await?,
            id: read_text(reader).await?,
            field: read_text(reader).await?,
            value: match reader.read_u8().await? {
                0 => None,
                1 => Some(read_text(reader).await?),
                other => {
                    return Err(SyncError::Protocol(format!("bad option tag {other}")));
                }
            },
        });
    }
    Ok(messages)
}

/// A length as a 4-byte number; more than 4 billion items is a bug.
async fn write_length<W: AsyncWrite + Unpin>(
    writer: &mut W,
    length: usize,
) -> Result<(), SyncError> {
    let length = u32::try_from(length).expect("fewer than 2^32 items");
    writer.write_u32(length).await?;
    Ok(())
}

async fn write_text<W: AsyncWrite + Unpin>(writer: &mut W, text: &str) -> Result<(), SyncError> {
    write_length(writer, text.len()).await?;
    writer.write_all(text.as_bytes()).await?;
    Ok(())
}

async fn read_text<R: AsyncRead + Unpin>(reader: &mut R) -> Result<String, SyncError> {
    let length = reader.read_u32().await?;
    if length > MAX_TEXT {
        return Err(SyncError::Protocol(format!("text of {length} bytes")));
    }
    // `vec![0; n]`: n zero bytes, filled by `read_exact`.
    let mut bytes = vec![0; length as usize];
    reader.read_exact(&mut bytes).await?;
    String::from_utf8(bytes).map_err(|_| SyncError::Protocol("text is not UTF-8".to_string()))
}
