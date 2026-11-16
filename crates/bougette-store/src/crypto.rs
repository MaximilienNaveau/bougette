//! The cryptography (ADR 0005), in one place and with standard building
//! blocks only:
//! - a member's **identity** is a 32-byte secret seed; from it come an
//!   Ed25519 signing key (its public half is the member's identifier) and
//!   an X25519 key to receive budget keys;
//! - each budget has symmetric **budget keys** (one per generation, a new
//!   one when a member leaves), sealed for each member's X25519 key;
//! - contents are encrypted with XChaCha20-Poly1305 (random 24-byte
//!   nonces), every message is signed with Ed25519;
//! - the recovery phrase is stretched with Argon2id.

use argon2::{Algorithm, Argon2, Params, Version};
// `aead` is the traits' crate, re-exported by chacha20poly1305: `Aead`
// gives `encrypt`/`decrypt`, `KeyInit` gives `new`.
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use x25519_dalek::{PublicKey, StaticSecret};

/// `N` random bytes from the operating system. A const generic: `N` is a
/// compile-time number, like `template <size_t N>`.
pub fn random<const N: usize>() -> [u8; N] {
    let mut bytes = [0; N];
    // Fails only if the OS has no random source at all: nothing safe can
    // be done then.
    getrandom::fill(&mut bytes).expect("the system has a random source");
    bytes
}

/// SHA-256 of `parts`, one after the other.
pub fn sha256(parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part);
    }
    // `finalize` gives a fixed-size array type of its own; `into` converts
    // it to a plain `[u8; 32]`.
    hasher.finalize().into()
}

/// HMAC-SHA-256 of `data` with `key`: proves knowledge of `key`.
pub fn hmac(key: &[u8], data: &[u8]) -> [u8; 32] {
    // `<Hmac<Sha256> as KeyInit>::new_from_slice` names the trait method
    // explicitly, because `Mac` has one with the same name. Any key length
    // works for HMAC, so it can't fail.
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(key).expect("HMAC takes any key");
    mac.update(data);
    mac.finalize().into_bytes().into()
}

/// A member's keys, all derived from one secret seed: the seed is what
/// moves to the member's other devices and into the encrypted backup.
pub struct Identity {
    seed: [u8; 32],
    signing: SigningKey,
    receiving: StaticSecret,
}

impl Identity {
    pub fn generate() -> Identity {
        Identity::from_seed(random())
    }

    pub fn from_seed(seed: [u8; 32]) -> Identity {
        Identity {
            seed,
            signing: SigningKey::from_bytes(&seed),
            // A second key from the same seed: hashing with a label keeps
            // the two independent.
            receiving: StaticSecret::from(sha256(&[b"bougette receiving key", &seed])),
        }
    }

    pub fn seed(&self) -> [u8; 32] {
        self.seed
    }

    /// The member's identifier: their public signing key, in hexadecimal.
    pub fn member(&self) -> String {
        hex(self.signing.verifying_key().as_bytes())
    }

    /// The public key others seal budget keys for, in hexadecimal.
    pub fn receiving_key(&self) -> String {
        hex(PublicKey::from(&self.receiving).as_bytes())
    }

    pub fn sign(&self, data: &[u8]) -> Vec<u8> {
        self.signing.sign(data).to_bytes().to_vec()
    }

    /// Opens what `seal` sealed for this member; `None` if it wasn't for
    /// them or was tampered with.
    pub fn open(&self, sealed: &[u8]) -> Option<Vec<u8>> {
        // `split_first_chunk::<32>` cuts off the first 32 bytes as an
        // array: the sender's one-time public key.
        let (theirs, ciphertext) = sealed.split_first_chunk::<32>()?;
        let theirs = PublicKey::from(*theirs);
        let shared = self.receiving.diffie_hellman(&theirs);
        let ours = PublicKey::from(&self.receiving);
        let key = sha256(&[
            b"bougette seal",
            shared.as_bytes(),
            theirs.as_bytes(),
            ours.as_bytes(),
        ]);
        decrypt(&key, b"", ciphertext)
    }
}

/// Seals `plaintext` for the owner of `receiving_key` (hexadecimal): only
/// their `Identity::open` can read it. A one-time key pair is drawn for
/// each seal, so the sender keeps nothing.
pub fn seal(receiving_key: &str, plaintext: &[u8]) -> Option<Vec<u8>> {
    let theirs = PublicKey::from(from_hex_array::<32>(receiving_key)?);
    let once = StaticSecret::from(random::<32>());
    let ours = PublicKey::from(&once);
    let shared = once.diffie_hellman(&theirs);
    let key = sha256(&[
        b"bougette seal",
        shared.as_bytes(),
        ours.as_bytes(),
        theirs.as_bytes(),
    ]);
    let mut sealed = ours.as_bytes().to_vec();
    sealed.extend(encrypt(&key, b"", plaintext));
    Some(sealed)
}

/// Whether `signature` is `member`'s signature of `data`.
pub fn verify(member: &str, data: &[u8], signature: &[u8]) -> bool {
    let Some(key) = from_hex_array::<32>(member) else {
        return false;
    };
    let Ok(signature) = <[u8; 64]>::try_from(signature) else {
        return false;
    };
    // `verify_strict` also refuses the rare malleable signatures.
    VerifyingKey::from_bytes(&key).is_ok_and(|key| {
        key.verify_strict(data, &Signature::from_bytes(&signature))
            .is_ok()
    })
}

/// Encrypts `plaintext` with `key`, binding it to `context` (encrypted
/// with another context, it won't decrypt). Returns nonce + ciphertext.
pub fn encrypt(key: &[u8; 32], context: &[u8], plaintext: &[u8]) -> Vec<u8> {
    let cipher = XChaCha20Poly1305::new(&Key::from(*key));
    // 24 random bytes: large enough that drawing the same twice is
    // practically impossible, so no counter needs keeping across devices.
    let nonce: [u8; 24] = random();
    let payload = Payload {
        msg: plaintext,
        aad: context,
    };
    let ciphertext = cipher
        .encrypt(&XNonce::from(nonce), payload)
        .expect("encryption of a buffer in memory doesn't fail");
    let mut sealed = nonce.to_vec();
    sealed.extend(ciphertext);
    sealed
}

/// The reverse of `encrypt`; `None` with the wrong key or context, or if
/// anything was changed.
pub fn decrypt(key: &[u8; 32], context: &[u8], sealed: &[u8]) -> Option<Vec<u8>> {
    let (nonce, ciphertext) = sealed.split_first_chunk::<24>()?;
    let cipher = XChaCha20Poly1305::new(&Key::from(*key));
    let payload = Payload {
        msg: ciphertext,
        aad: context,
    };
    cipher.decrypt(&XNonce::from(*nonce), payload).ok()
}

/// A budget's identifier, derived from its creator and a random nonce.
/// Anyone can check who created a budget, and nobody else can claim it:
/// that would take finding another key with the same hash.
pub fn budget_id(creator: &str, nonce: &[u8]) -> Uuid {
    let hash = sha256(&[b"bougette budget", creator.as_bytes(), nonce]);
    // A version 8 UUID is "custom": 122 of its bits are ours.
    Uuid::new_v8(*hash.first_chunk::<16>().expect("32 bytes"))
}

/// How hard stretching a phrase is: memory in KiB and passes. Stored
/// with the backup, so it can change without breaking old backups.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cost {
    pub memory: u32,
    pub passes: u32,
}

/// For recovery phrases: 64 MiB and 3 passes (Argon2id as recommended by
/// OWASP), about a second on a phone, so guessing is very slow.
pub const RECOVERY_COST: Cost = Cost {
    memory: 64 * 1024,
    passes: 3,
};

/// A key from a recovery phrase and a random salt, with Argon2id.
pub fn stretch(phrase: &str, salt: &[u8], cost: Cost) -> [u8; 32] {
    let params =
        Params::new(cost.memory, cost.passes, 1, Some(32)).expect("valid Argon2 parameters");
    let mut key = [0; 32];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(normalize(phrase).as_bytes(), salt, &mut key)
        .expect("valid Argon2 input");
    key
}

/// The 32 symbols of a generated phrase: Crockford's base 32, without I,
/// L, O and U, which are easy to misread.
const SYMBOLS: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// A new recovery phrase: 8 groups of 4 symbols, 160 random bits, e.g.
/// `7K2Q-M9XD-...`.
pub fn new_phrase() -> String {
    let bytes: [u8; 32] = random();
    // Each byte gives one symbol: 256 is a multiple of 32, so `% 32` keeps
    // every symbol equally likely.
    let symbols: Vec<char> = bytes
        .iter()
        .map(|byte| SYMBOLS[usize::from(byte % 32)] as char)
        .collect();
    // `chunks(4)` cuts the list into groups of 4.
    symbols
        .chunks(4)
        .map(|group| group.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("-")
}

/// Whether a phrase chosen by the member is strong enough (ADR 0003): at
/// least 4 words and 20 characters, not counting spaces. A rough rule, to
/// refine with a strength estimator.
pub fn strong_enough(phrase: &str) -> bool {
    let phrase = normalize(phrase);
    phrase.split(' ').count() >= 4 && phrase.chars().filter(|c| *c != ' ').count() >= 20
}

/// The phrase as compared: words separated by single spaces, so a stray
/// space doesn't lock someone out.
fn normalize(phrase: &str) -> String {
    phrase.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn from_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || !text.is_ascii() {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&text[index..index + 2], 16).ok())
        .collect()
}

/// Hexadecimal text of exactly `N` bytes.
pub fn from_hex_array<const N: usize>(text: &str) -> Option<[u8; N]> {
    from_hex(text)?.try_into().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signatures_check_the_author_and_the_data() {
        let alice = Identity::generate();
        let bob = Identity::generate();
        let signature = alice.sign(b"rent: 900");
        assert!(verify(&alice.member(), b"rent: 900", &signature));
        assert!(!verify(&alice.member(), b"rent: 9000", &signature));
        assert!(!verify(&bob.member(), b"rent: 900", &signature));
        assert!(!verify("not hex", b"rent: 900", &signature));
    }

    #[test]
    fn only_the_recipient_opens_a_seal() {
        let alice = Identity::generate();
        let bob = Identity::generate();
        let sealed = seal(&alice.receiving_key(), b"budget key").unwrap();
        assert_eq!(alice.open(&sealed).as_deref(), Some(&b"budget key"[..]));
        assert_eq!(bob.open(&sealed), None);
        // The same seed gives the same keys, on any device.
        let again = Identity::from_seed(alice.seed());
        assert_eq!(again.member(), alice.member());
        assert_eq!(again.open(&sealed).as_deref(), Some(&b"budget key"[..]));
    }

    #[test]
    fn encryption_needs_the_key_and_the_context() {
        let key: [u8; 32] = random();
        let sealed = encrypt(&key, b"message 1", b"groceries");
        assert_eq!(
            decrypt(&key, b"message 1", &sealed).as_deref(),
            Some(&b"groceries"[..])
        );
        assert_eq!(decrypt(&key, b"message 2", &sealed), None);
        assert_eq!(decrypt(&random(), b"message 1", &sealed), None);
        let mut tampered = sealed.clone();
        *tampered.last_mut().unwrap() ^= 1;
        assert_eq!(decrypt(&key, b"message 1", &tampered), None);
    }

    #[test]
    fn budget_identifiers_bind_their_creator() {
        let alice = Identity::generate();
        let nonce: [u8; 16] = random();
        let id = budget_id(&alice.member(), &nonce);
        assert_eq!(id, budget_id(&alice.member(), &nonce));
        assert_ne!(id, budget_id(&Identity::generate().member(), &nonce));
    }

    #[test]
    fn phrases() {
        let phrase = new_phrase();
        assert_eq!(phrase.len(), 8 * 4 + 7);
        assert!(phrase.split('-').all(|group| group.len() == 4));
        assert!(
            phrase
                .bytes()
                .all(|byte| byte == b'-' || SYMBOLS.contains(&byte))
        );
        assert_ne!(phrase, new_phrase());
        assert!(strong_enough("correct horse battery staple"));
        assert!(!strong_enough("password"));
        assert!(!strong_enough("a b c d"));
        // Cheap parameters: the real ones take seconds in a debug build.
        let cheap = Cost {
            memory: 64,
            passes: 1,
        };
        let salt: [u8; 16] = random();
        assert_eq!(
            stretch("my  phrase ", &salt, cheap),
            stretch("my phrase", &salt, cheap)
        );
        assert_ne!(
            stretch("my phrase", &salt, cheap),
            stretch("my phrase!", &salt, cheap)
        );
    }
}
