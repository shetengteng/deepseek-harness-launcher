//! dsh 安装完整性校验（设计 §M3.2）。
//!
//! 设计要求两层校验：
//! 1. **入口存在性**：`node_modules/@deepseek-ai/dsh/lib/bin.js` 必须存在
//! 2. **tarball SHA-512**：与 registry `dist.integrity` 字段（`sha512-<base64>`）比对
//!
//! 注意：子依赖的 integrity 由 npm 自查，壳子不重复。这里只校验 dsh 主包。

use std::path::Path;

use sha2::{Digest, Sha512};

use crate::error::{LauncherError, Result};

/// dsh 主包入口相对路径（相对 `node_modules/@deepseek-ai/dsh/`）。
pub const DSH_ENTRY_REL: &str = "lib/bin.js";

/// 校验入口：`node_modules/@deepseek-ai/dsh/lib/bin.js` 必须存在。
pub fn verify_entry_exists(dsh_module_dir: &Path) -> Result<()> {
    let entry = dsh_module_dir.join(DSH_ENTRY_REL);
    if !entry.exists() {
        return Err(LauncherError::DshInstall(format!(
            "dsh entry not found: expected {}",
            entry.display()
        )));
    }
    Ok(())
}

/// 解析 npm `dist.integrity` 字段。
///
/// npm 格式：`sha512-<base64>`。
/// 返回 `(算法名, 二进制 digest)`。
/// 非 sha512 算法 → `Err`（壳子只支持 sha512）。
pub fn parse_integrity(integrity: &str) -> Result<(&'static str, Vec<u8>)> {
    let (algo, b64) = integrity
        .split_once('-')
        .ok_or_else(|| LauncherError::DshInstall(format!("malformed integrity: {integrity}")))?;
    if algo != "sha512" {
        return Err(LauncherError::DshInstall(format!(
            "unsupported integrity algorithm: {algo} (only sha512 supported)"
        )));
    }
    let digest = base64_decode(b64)?;
    Ok(("sha512", digest))
}

/// 标准 base64 解码（支持 padding）。
fn base64_decode(s: &str) -> Result<Vec<u8>> {
    // 简单实现：用 `base64` crate 替代更省事，但避免新增依赖，手写
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut buf = Vec::with_capacity(s.len() * 3 / 4);
    let mut bits: u32 = 0;
    let mut count = 0;
    for c in s.bytes() {
        if c == b'=' {
            break;
        }
        let Some(i) = ALPHABET.iter().position(|&x| x == c) else {
            return Err(LauncherError::DshInstall(format!(
                "invalid base64 char {} in integrity",
                c as char
            )));
        };
        bits = (bits << 6) | (i as u32);
        count += 6;
        if count >= 8 {
            count -= 8;
            buf.push((bits >> count) as u8);
        }
    }
    Ok(buf)
}

/// 计算 `data` 的 SHA-512 二进制 digest。
pub fn sha512(data: &[u8]) -> Vec<u8> {
    let mut hasher = Sha512::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}

/// 校验 tarball 字节流：计算 SHA-512 并与 `integrity` 字段比对。
///
/// 失败时返回 `Err(DshInstall)`，调用方应删除已下载/解压的内容。
pub fn verify_tarball_integrity(tarball_bytes: &[u8], integrity: &str) -> Result<()> {
    let (algo, expected) = parse_integrity(integrity)?;
    debug_assert_eq!(algo, "sha512");
    let actual = sha512(tarball_bytes);
    if actual != expected {
        return Err(LauncherError::DshInstall(format!(
            "tarball integrity mismatch: expected {} bytes, got {} bytes (sha512)",
            expected.len(),
            actual.len()
        )));
    }
    Ok(())
}

/// 完整校验：入口存在 + tarball 完整性。
pub fn verify_installation(
    dsh_module_dir: &Path,
    tarball_bytes: &[u8],
    integrity: &str,
) -> Result<()> {
    verify_entry_exists(dsh_module_dir)?;
    verify_tarball_integrity(tarball_bytes, integrity)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 32 字节测试数据 → SHA-512 → base64 编码 → 模拟 npm integrity 字段。
    fn make_integrity(data: &[u8]) -> String {
        let digest = sha512(data);
        let b64 = base64_encode(&digest);
        format!("sha512-{b64}")
    }

    /// 标准 base64 编码（仅测试用）。
    fn base64_encode(data: &[u8]) -> String {
        const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
        let mut i = 0;
        while i + 3 <= data.len() {
            let n = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8) | (data[i + 2] as u32);
            out.push(ALPHABET[(n >> 18) as usize & 0x3f] as char);
            out.push(ALPHABET[(n >> 12) as usize & 0x3f] as char);
            out.push(ALPHABET[(n >> 6) as usize & 0x3f] as char);
            out.push(ALPHABET[n as usize & 0x3f] as char);
            i += 3;
        }
        let rem = data.len() - i;
        if rem == 1 {
            let n = (data[i] as u32) << 16;
            out.push(ALPHABET[(n >> 18) as usize & 0x3f] as char);
            out.push(ALPHABET[(n >> 12) as usize & 0x3f] as char);
            out.push('=');
            out.push('=');
        } else if rem == 2 {
            let n = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8);
            out.push(ALPHABET[(n >> 18) as usize & 0x3f] as char);
            out.push(ALPHABET[(n >> 12) as usize & 0x3f] as char);
            out.push(ALPHABET[(n >> 6) as usize & 0x3f] as char);
            out.push('=');
        }
        out
    }

    #[test]
    fn parse_integrity_sha512_extracts_digest() {
        let data = b"hello dsh";
        let s = make_integrity(data);
        let (algo, digest) = parse_integrity(&s).expect("parse");
        assert_eq!(algo, "sha512");
        assert_eq!(digest, sha512(data));
    }

    #[test]
    fn parse_integrity_rejects_non_sha512() {
        let s = "sha256-abc==";
        let err = parse_integrity(s).unwrap_err();
        assert!(err.to_string().contains("sha256"));
    }

    #[test]
    fn parse_integrity_rejects_malformed() {
        // "nodashhere" 没有 `-`，应该报 malformed
        let err = parse_integrity("nodashhere").unwrap_err();
        assert!(err.to_string().contains("malformed"));
    }

    #[test]
    fn verify_tarball_matches() {
        let data = b"tarball content";
        let s = make_integrity(data);
        verify_tarball_integrity(data, &s).expect("should match");
    }

    #[test]
    fn verify_tarball_mismatch_errors() {
        let data = b"tarball content";
        let s = make_integrity(b"different content");
        let err = verify_tarball_integrity(data, &s).unwrap_err();
        assert!(err.to_string().contains("mismatch"));
    }

    #[test]
    fn verify_entry_exists_passes_when_present() {
        let dir = tempfile::tempdir().unwrap();
        let lib = dir.path().join("lib");
        std::fs::create_dir_all(&lib).unwrap();
        std::fs::write(lib.join("bin.js"), "// dsh entry").unwrap();
        verify_entry_exists(dir.path()).expect("entry should exist");
    }

    #[test]
    fn verify_entry_exists_errors_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let err = verify_entry_exists(dir.path()).unwrap_err();
        assert!(err.to_string().contains("not found"));
        assert!(err.to_string().contains("lib/bin.js"));
    }

    #[test]
    fn verify_installation_combined_passes() {
        let dir = tempfile::tempdir().unwrap();
        let lib = dir.path().join("lib");
        std::fs::create_dir_all(&lib).unwrap();
        std::fs::write(lib.join("bin.js"), "// dsh").unwrap();

        let data = b"tarball";
        let s = make_integrity(data);
        verify_installation(dir.path(), data, &s).expect("ok");
    }

    #[test]
    fn verify_installation_fails_when_entry_missing() {
        let dir = tempfile::tempdir().unwrap();
        let data = b"tarball";
        let s = make_integrity(data);
        let err = verify_installation(dir.path(), data, &s).unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn verify_installation_fails_when_integrity_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let lib = dir.path().join("lib");
        std::fs::create_dir_all(&lib).unwrap();
        std::fs::write(lib.join("bin.js"), "// dsh").unwrap();

        let s = make_integrity(b"original");
        let err = verify_installation(dir.path(), b"tampered", &s).unwrap_err();
        assert!(err.to_string().contains("mismatch"));
    }

    /// npm 真实 integrity 字段格式：64 字节 digest → 88 字符 base64（无 padding）。
    #[test]
    fn npm_real_integrity_format_round_trip() {
        let data = b"real tarball bytes 0123456789";
        let s = make_integrity(data);
        // 真实 npm integrity base64 长度 88（64 字节 → ceil(64/3)*4 = 88，无 padding 因为 64 % 3 == 1 应有 == padding）
        // 实际 64 字节 mod 3 = 1，base64 末尾会有两个 = padding。总长 88。
        assert!(s.starts_with("sha512-"));
        let (algo, digest) = parse_integrity(&s).expect("parse");
        assert_eq!(algo, "sha512");
        assert_eq!(digest, sha512(data));
    }
}
