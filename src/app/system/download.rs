//! O download da atualização: só endereços do GitHub (nos redirecionamentos também), com limite de
//! tempo, e a conferência do sha256 que o GitHub publica (`digest`) antes de o instalador rodar.

use crate::prelude::*;
use std::io::Read;
use std::path::Path;

const MAX_REDIRECTS: usize = 5;
/// An installer bigger than this is not ours (the MSI is about 20 MB).
const MAX_BYTES: u64 = 512 * 1024 * 1024;

/// Whether `host` serves GitHub's releases (their assets redirect to githubusercontent.com).
pub(crate) fn allowed_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    host == "github.com" || host == "api.github.com" || host.ends_with(".githubusercontent.com")
}

/// An https address on an allowed host, without credentials: `https://github.com@evil.com/` is evil.com.
pub(crate) fn allowed_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else { return false };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = authority.rsplit_once(':').filter(|(_, port)| port.bytes().all(|byte| byte.is_ascii_digit())).map_or(authority, |(host, _)| host);
    !authority.contains(['@', '\\']) && !host.is_empty() && allowed_host(host)
}

/// `sha256:<64 hex>` as GitHub writes a release asset's digest → the hex, in lowercase.
pub(crate) fn parse_digest(digest: &str) -> Option<String> {
    let hex = digest.strip_prefix("sha256:")?;
    (hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit())).then(|| hex.to_ascii_lowercase())
}

const ROUNDS: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74,
    0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d,
    0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e,
    0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
    0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// One 64-byte block into the hash state.
fn compress(state: &mut [u32; 8], block: &[u8]) {
    let mut words = [0u32; 64];
    for (index, bytes) in block.chunks_exact(4).enumerate() {
        words[index] = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    }
    for index in 16..64 {
        let (a, b) = (words[index - 15], words[index - 2]);
        let small0 = a.rotate_right(7) ^ a.rotate_right(18) ^ (a >> 3);
        let small1 = b.rotate_right(17) ^ b.rotate_right(19) ^ (b >> 10);
        words[index] = words[index - 16].wrapping_add(small0).wrapping_add(words[index - 7]).wrapping_add(small1);
    }
    let mut v = *state;
    for index in 0..64 {
        let big1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
        let choose = (v[4] & v[5]) ^ (!v[4] & v[6]);
        let first = v[7].wrapping_add(big1).wrapping_add(choose).wrapping_add(ROUNDS[index]).wrapping_add(words[index]);
        let big0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
        let majority = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
        let second = big0.wrapping_add(majority);
        v = [first.wrapping_add(second), v[0], v[1], v[2], v[3].wrapping_add(first), v[4], v[5], v[6]];
    }
    for (slot, value) in state.iter_mut().zip(v) {
        *slot = slot.wrapping_add(value);
    }
}

/// SHA-256 of what `reader` gives, as 64 lowercase hex digits (FIPS 180-4).
pub(crate) fn sha256_hex(mut reader: impl Read) -> std::io::Result<String> {
    let mut state: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let mut total: u64 = 0;
    let mut pending: Vec<u8> = Vec::new();
    let mut chunk = vec![0u8; 1 << 16];
    loop {
        let read = reader.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        total += read as u64;
        pending.extend_from_slice(&chunk[..read]);
        let whole = pending.len() / 64 * 64;
        for block in pending[..whole].chunks_exact(64) {
            compress(&mut state, block);
        }
        pending.drain(..whole);
    }
    pending.push(0x80);
    while pending.len() % 64 != 56 {
        pending.push(0);
    }
    pending.extend_from_slice(&(total * 8).to_be_bytes());
    for block in pending.chunks_exact(64) {
        compress(&mut state, block);
    }
    Ok(state.iter().map(|word| format!("{word:08x}")).collect())
}

/// The file against the release's digest. No digest, or one that does not match: refused.
pub(crate) fn verify_file(path: &Path, digest: Option<&str>) -> Result<(), String> {
    let wanted = digest.and_then(parse_digest).ok_or("o GitHub não publicou o sha256 do instalador: por segurança ele não será executado")?;
    let found = fs::File::open(path).and_then(sha256_hex).map_err(|error| format!("não foi possível ler o instalador baixado: {error}"))?;
    if found == wanted { Ok(()) } else { Err("o instalador baixado não confere com o sha256 do GitHub: descartado".into()) }
}

/// What curl's `-D` file says about the last response: the status and the `Location`.
pub(crate) fn parse_head(head: &str) -> Option<(u16, Option<String>)> {
    let head = head.replace('\r', "");
    let last = head.split("\n\n").map(str::trim).filter(|part| part.starts_with("HTTP/")).last()?;
    let status = last.lines().next()?.split_whitespace().nth(1)?.parse().ok()?;
    let location = last.lines().skip(1).find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.trim().eq_ignore_ascii_case("location").then(|| value.trim().to_string())
    });
    Some((status, location))
}

/// Downloads `url` into `part`, hop by hop: each redirect is looked at before it is followed, so
/// nothing outside GitHub is ever asked. Connect and total timeouts, a size cap, and `--fail`.
pub(crate) fn fetch(url: &str, part: &Path) -> Result<(), String> {
    let head_file = part.with_extension("head");
    let mut current = url.to_string();
    let result = (|| {
        for _ in 0..=MAX_REDIRECTS {
            if !allowed_url(&current) {
                return Err(format!("o download foi para fora do GitHub ({current}): recusado"));
            }
            let status = crate::editor::command("curl")
                .args(["-sS", "--fail", "--connect-timeout", "15", "--max-time", "900", "--max-redirs", "0", "--proto", "=https", "--max-filesize"])
                .arg(MAX_BYTES.to_string())
                .arg("-D")
                .arg(&head_file)
                .arg("-o")
                .arg(part)
                .arg(&current)
                .status()
                .map_err(|error| format!("curl: {error}"))?;
            if !status.success() {
                return Err("não foi possível baixar o instalador (sem conexão, tempo esgotado ou recusado); confira a internet".into());
            }
            match parse_head(&fs::read_to_string(&head_file).unwrap_or_default()) {
                Some((200, _)) => return Ok(()),
                Some((301 | 302 | 303 | 307 | 308, Some(next))) => current = next,
                other => return Err(format!("o GitHub respondeu de um jeito inesperado ({other:?})")),
            }
        }
        Err("redirecionamentos demais".into())
    })();
    let _ = fs::remove_file(&head_file);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[test]
    fn sha256_matches_the_known_vectors() {
        assert_eq!(sha256_hex(&b"abc"[..]).unwrap(), ABC);
        assert_eq!(sha256_hex(&b""[..]).unwrap(), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        // two blocks plus padding, and more than one read chunk
        assert_eq!(
            sha256_hex(&b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"[..]).unwrap(),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        assert_eq!(sha256_hex(std::io::repeat(b'a').take(1_000_000)).unwrap(), "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0");
    }

    #[test]
    fn only_a_matching_digest_lets_the_installer_run() {
        let file = std::env::temp_dir().join(format!("magoagent-sha-{}.bin", std::process::id()));
        fs::write(&file, b"abc").unwrap();
        assert!(verify_file(&file, Some(&format!("sha256:{ABC}"))).is_ok());
        assert!(verify_file(&file, Some(&format!("sha256:{}", ABC.to_uppercase()))).is_ok());
        assert!(verify_file(&file, Some(&format!("sha256:{}", "0".repeat(64)))).unwrap_err().contains("não confere"));
        // no digest, or one in another format: refused
        assert!(verify_file(&file, None).unwrap_err().contains("sha256"));
        assert!(verify_file(&file, Some("md5:abc")).is_err() && verify_file(&file, Some("sha256:abc")).is_err());
        assert!(verify_file(&file.with_extension("nao"), Some(&format!("sha256:{ABC}"))).is_err());
        let _ = fs::remove_file(&file);
    }

    #[test]
    fn only_github_hosts_are_followed() {
        assert!(allowed_url("https://github.com/edsuuu/magoagent/releases/download/v1/x.msi"));
        assert!(allowed_url("https://objects.githubusercontent.com/a?b=c") && allowed_url("https://release-assets.githubusercontent.com/x"));
        assert!(allowed_url("https://api.github.com/repos"));
        for bad in [
            "http://github.com/x",
            "https://evil.com/x",
            "https://github.com.evil.com/x",
            "https://evilgithub.com/x",
            "https://github.com@evil.com/x",
            "https://evil.com\\@github.com/x",
            "https://githubusercontent.com.evil.io/x",
            "file:///C:/x.msi",
            "https:///x",
            "github.com/x",
        ] {
            assert!(!allowed_url(bad), "{bad}");
        }
    }

    #[test]
    fn the_head_gives_the_status_and_the_redirect() {
        assert_eq!(
            parse_head("HTTP/2 302 \r\nlocation: https://objects.githubusercontent.com/x\r\nserver: GitHub\r\n\r\n"),
            Some((302, Some("https://objects.githubusercontent.com/x".into())))
        );
        assert_eq!(parse_head("HTTP/1.1 200 Connection established\r\n\r\nHTTP/2 200 \r\ncontent-length: 5\r\n\r\n"), Some((200, None)));
        assert_eq!(parse_head("HTTP/2 404 \r\n\r\n"), Some((404, None)));
        assert_eq!(parse_head(""), None);
    }
}
