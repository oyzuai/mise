use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::Path;

use crate::file;
use crate::file::display_path;
use crate::progress::SingleReport;
use blake3::Hasher as Blake3Hasher;
use digest::Digest;
use eyre::{Result, bail, ensure};
use md5::Md5;
use sha1::Sha1;
use sha2::{Sha224, Sha256, Sha384, Sha512};
use siphasher::sip::SipHasher;

/// A downloaded file's hash differs from the expected checksum.
#[derive(Debug, thiserror::Error)]
#[error(
    "Checksum mismatch for file {path}:\nExpected: {algo}:{expected}\nActual:   {algo}:{actual}"
)]
pub struct ChecksumMismatch {
    path: String,
    algo: String,
    expected: String,
    actual: String,
}

pub fn hash_to_str<T: Hash>(t: &T) -> String {
    let mut s = SipHasher::new();
    t.hash(&mut s);
    format!("{:x}", s.finish())
}

pub fn hash_sha256_to_str(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn file_hash_sha256(path: &Path, pr: Option<&dyn SingleReport>) -> Result<String> {
    let use_external_hasher = file::size(path).unwrap_or_default() > 50 * 1024 * 1024;
    if use_external_hasher && file::which("sha256sum").is_some() {
        let out = cmd!("sha256sum", path).read()?;
        Ok(out.split_whitespace().next().unwrap().to_string())
    } else {
        file_hash_prog::<Sha256>(path, pr)
    }
}

pub fn file_hash_prog<D>(path: &Path, pr: Option<&dyn SingleReport>) -> Result<String>
where
    D: Digest,
{
    let mut file = file::open(path)?;
    if let Some(pr) = pr {
        pr.set_length(file.metadata()?.len());
    }
    let mut hasher = D::new();
    let mut buf = [0; 32 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        if let Some(pr) = pr {
            pr.inc(n as u64);
        }
    }
    let hash = hasher.finalize();
    Ok(hash.iter().map(|b| format!("{b:02x}")).collect())
}

pub fn hash_blake3_to_str(s: &str) -> String {
    let mut hasher = Blake3Hasher::new();
    hasher.update(s.as_bytes());
    hasher.finalize().to_hex().to_string()
}

pub fn file_hash_blake3(path: &Path, pr: Option<&dyn SingleReport>) -> Result<String> {
    let mut file = file::open(path)?;
    if let Some(pr) = pr {
        pr.set_length(file.metadata()?.len());
    }
    let mut hasher = Blake3Hasher::new();
    let mut buf = [0; 32 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        if let Some(pr) = pr {
            pr.inc(n as u64);
        }
    }
    let hash = hasher.finalize();
    Ok(format!("{}", hash.to_hex()))
}

pub fn ensure_checksum(
    path: &Path,
    checksum: &str,
    pr: Option<&dyn SingleReport>,
    algo: &str,
) -> Result<()> {
    let use_external_hasher = file::size(path).unwrap_or(u64::MAX) > 10 * 1024 * 1024;
    let actual = match algo {
        "blake3" => file_hash_blake3(path, pr)?,
        "sha512" => {
            if use_external_hasher && file::which("sha512sum").is_some() {
                let out = cmd!("sha512sum", path).read()?;
                out.split_whitespace().next().unwrap().to_string()
            } else {
                file_hash_prog::<Sha512>(path, pr)?
            }
        }
        "sha256" => file_hash_prog::<Sha256>(path, pr)?,
        "sha224" => file_hash_prog::<Sha224>(path, pr)?,
        "sha384" => file_hash_prog::<Sha384>(path, pr)?,
        "sha1" => {
            if use_external_hasher && file::which("sha1sum").is_some() {
                let out = cmd!("sha1sum", path).read()?;
                out.split_whitespace().next().unwrap().to_string()
            } else {
                file_hash_prog::<Sha1>(path, pr)?
            }
        }
        "md5" => {
            if use_external_hasher && file::which("md5sum").is_some() {
                let out = cmd!("md5sum", path).read()?;
                out.split_whitespace().next().unwrap().to_string()
            } else {
                file_hash_prog::<Md5>(path, pr)?
            }
        }
        _ => bail!("Unknown checksum algorithm: {}", algo),
    };
    let checksum = checksum.to_lowercase();
    if actual != checksum {
        return Err(ChecksumMismatch {
            path: display_path(path),
            algo: algo.to_string(),
            expected: checksum,
            actual,
        }
        .into());
    }
    Ok(())
}

pub fn parse_shasums(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(shasum_fields)
        .map(|(hash, name, _)| (name.into(), hash.into()))
        .collect()
}

fn shasum_fields(line: &str) -> Option<(&str, &str, bool)> {
    let mut parts = line.split_whitespace();
    let hash = parts.next()?;
    let name = parts.next()?;
    // Coreutils binary-mode marker; shared with the permissive legacy parser.
    Some((
        hash,
        name.strip_prefix('*').unwrap_or(name),
        parts.next().is_some(),
    ))
}

/// Bounded SHA-256 manifest decoding for metadata admission. Reject duplicate
/// names (even identical hashes), malformed/extra fields and non-SHA-256 values.
/// This validates syntax, not the publisher or the bytes of any named artifact.
pub fn parse_sha256sums_checked(text: &str) -> Result<HashMap<String, String>> {
    ensure!(
        text.len() <= 8 * 1024 * 1024,
        "SHA-256 manifest exceeds byte limit"
    );
    let mut records = HashMap::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let (hash, name, extra) =
            shasum_fields(line).ok_or_else(|| eyre::eyre!("malformed SHA-256 manifest entry"))?;
        ensure!(
            !extra && !name.is_empty() && !name.chars().any(char::is_control),
            "invalid SHA-256 manifest filename/fields"
        );
        ensure!(
            hash.len() == 64 && hash.bytes().all(|c| c.is_ascii_hexdigit()),
            "invalid SHA-256 manifest digest"
        );
        ensure!(records.len() < 4096, "SHA-256 manifest exceeds entry limit");
        ensure!(
            records
                .insert(name.to_owned(), hash.to_ascii_lowercase())
                .is_none(),
            "duplicate SHA-256 manifest filename"
        );
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_corepack_checksum_algorithms() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("package.tgz");
        file::write(&path, b"corepack").unwrap();

        ensure_checksum(
            &path,
            "8aba612a6193520492b81c698962a0d81c3609da7ab498d028ac452e",
            None,
            "sha224",
        )
        .unwrap();
        ensure_checksum(
            &path,
            "cbedf1fe9f759bba045da15cecdfb24340308ffdae357bbb0a10bed2535aa957c7cad13d8081847203cd409a7a7e3cda",
            None,
            "sha384",
        )
        .unwrap();
    }
}
