//! 비밀번호 저장소: 앱 설정 폴더의 암호화 파일 (키체인 팝업을 피하기 위해).
//! - secrets.json: { key: base64(nonce(12) || AES-256-GCM ciphertext) }
//! - secret.key  : 32바이트 랜덤 키
//! 두 파일 모두 권한 0600. 같은 사용자 권한으로 파일을 읽으면 복호화할 수 있다 (DBeaver 수준).
use crate::error::{AppError, AppResult};
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::Engine;
use rand::RngCore;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub struct SecretFile {
    path: PathBuf,
    cipher: Aes256Gcm,
}

/// 본인만 읽는 파일로 쓴다: 처음부터 0600 으로 만든 임시 파일에 쓰고 rename (잠깐이라도 0644 로 열려 있지 않게, 도중에 깨지지 않게)
pub(crate) fn write_private(path: &Path, data: &[u8]) -> AppResult<()> {
    use std::io::Write;
    if let Some(dir) = path.parent() {
        private_dir(dir)?;
    }
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(&tmp)?;
    f.write_all(data)?;
    f.sync_all()?;
    drop(f);
    fs::rename(&tmp, path)?;
    Ok(())
}

/// 설정 폴더: 본인만 들어갈 수 있게 0700
pub(crate) fn private_dir(dir: &Path) -> AppResult<()> {
    fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
    }
    Ok(())
}

impl SecretFile {
    pub fn open(dir: &Path) -> AppResult<Self> {
        private_dir(dir)?;
        let key_path = dir.join("secret.key");
        let secrets_path = dir.join("secrets.json");
        let key: Vec<u8> = match fs::read(&key_path) {
            Ok(k) if k.len() == 32 => k,
            // 있는데 못 읽거나 길이가 틀리면 새로 만들지 않는다 (덮어쓰면 저장된 비밀번호를 영영 못 푼다)
            Ok(_) => return Err(AppError::Msg(format!("{} is corrupted — restore it from a backup, or delete it and secrets.json to start over", key_path.display()))),
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(AppError::Msg(format!("Cannot read {}: {e}", key_path.display()))),
            Err(_) => {
                // 키 없이 비밀번호 파일만 남아 있으면 지우지 말고 옆으로 치워 둔다
                if secrets_path.exists() {
                    let _ = fs::rename(&secrets_path, dir.join(format!("secrets.json.orphaned-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0))));
                }
                let mut k = vec![0u8; 32];
                rand::thread_rng().fill_bytes(&mut k);
                write_private(&key_path, &k)?;
                k
            }
        };
        Ok(Self { path: dir.join("secrets.json"), cipher: Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key)) })
    }

    fn load(&self) -> HashMap<String, String> {
        fs::read_to_string(&self.path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn get(&self, name: &str) -> Option<String> {
        let enc = self.load().remove(name)?;
        let raw = base64::engine::general_purpose::STANDARD.decode(enc).ok()?;
        if raw.len() < 12 {
            return None;
        }
        let (nonce, ct) = raw.split_at(12);
        // 이름을 AAD 로 묶어 둔다 (다른 항목의 암호문을 옮겨 붙이면 풀리지 않게). 예전 형식(AAD 없음)도 읽는다
        let pt = self
            .cipher
            .decrypt(Nonce::from_slice(nonce), Payload { msg: ct, aad: name.as_bytes() })
            .or_else(|_| self.cipher.decrypt(Nonce::from_slice(nonce), ct))
            .ok()?;
        String::from_utf8(pt).ok()
    }

    pub fn set(&self, name: &str, value: Option<&str>) -> AppResult<()> {
        let mut all = self.load();
        match value {
            Some(v) => {
                let mut nonce = [0u8; 12];
                rand::thread_rng().fill_bytes(&mut nonce);
                let ct = self
                    .cipher
                    .encrypt(Nonce::from_slice(&nonce), Payload { msg: v.as_bytes(), aad: name.as_bytes() })
                    .map_err(|_| AppError::Msg("encryption failed".into()))?;
                let mut raw = nonce.to_vec();
                raw.extend(ct);
                all.insert(name.to_string(), base64::engine::general_purpose::STANDARD.encode(raw));
            }
            None => {
                all.remove(name);
            }
        }
        write_private(&self.path, serde_json::to_string_pretty(&all)?.as_bytes())
    }

    pub fn contains(&self, name: &str) -> bool {
        self.load().contains_key(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_permissions() {
        let dir = std::env::temp_dir().join(format!("dbtool-secrets-{}", uuid::Uuid::new_v4()));
        let s = SecretFile::open(&dir).unwrap();
        assert_eq!(s.get("a"), None);
        s.set("a", Some("p@ss 한글")).unwrap();
        s.set("b#ssh", Some("x")).unwrap();
        // 다시 열어도(키 파일 재사용) 복호화돼야 함
        let s2 = SecretFile::open(&dir).unwrap();
        assert_eq!(s2.get("a").as_deref(), Some("p@ss 한글"));
        assert!(!std::fs::read_to_string(dir.join("secrets.json")).unwrap().contains("p@ss"));
        s2.set("a", None).unwrap();
        assert_eq!(s2.get("a"), None);
        assert!(s2.contains("b#ssh"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for f in ["secrets.json", "secret.key"] {
                assert_eq!(std::fs::metadata(dir.join(f)).unwrap().permissions().mode() & 0o777, 0o600);
            }
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}
