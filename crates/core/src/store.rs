//! 접속 설정 저장소 (JSON 파일). 비밀번호는 secrets.rs 의 암호화 파일에 둔다.
use crate::error::AppResult;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};


#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DbKind {
    Postgres,
    Mysql,
    Redis,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionConfig {
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub kind: DbKind,
    pub host: String,
    pub port: u16,
    pub user: String,
    #[serde(default)]
    pub database: String,
    /// 트리에 보일 DB·스키마 (DataGrip Data Source 속성의 Schemas 탭). 비어 있으면 전부.
    #[serde(default)]
    pub scope: Scope,
    /// 키체인에 비밀번호를 저장했는지 (목록 표시용 — 키체인을 읽지 않기 위해)
    #[serde(default)]
    pub password_saved: bool,
    #[serde(default)]
    pub ssh_secret_saved: bool,
    /// SSH 터널 (DataGrip의 SSH/SSL 탭)
    #[serde(default)]
    pub ssh: Option<SshConfig>,
    /// DataGrip처럼 접속을 색으로 구분 (예: "#5c2b2b"). 없으면 기본색.
    #[serde(default)]
    pub color: Option<String>,
    /// DataGrip Options 탭의 Read-only: 세션을 읽기 전용 트랜잭션으로 연다 (서버가 쓰기를 거부)
    #[serde(default)]
    pub read_only: bool,
    /// DataGrip Options 탭의 나머지 Connection 설정
    #[serde(default)]
    pub options: ConnOptions,
    /// SSH/SSL 탭의 SSL (PostgreSQL·MySQL·Redis 공통)
    #[serde(default)]
    pub ssl: Option<SslConfig>,
    /// Redis 연결 방식 (Standalone / Sentinel / Cluster)
    #[serde(default)]
    pub redis: Option<RedisMode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SslConfig {
    #[serde(default)]
    pub enabled: bool,
    /// require (암호화만, 인증서 검증 안 함) | verify-ca | verify-full (호스트 이름까지)
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub ca_file: String,
    #[serde(default)]
    pub cert_file: String,
    #[serde(default)]
    pub key_file: String,
}

impl SslConfig {
    pub fn mode(&self) -> &str {
        if self.mode.is_empty() { "require" } else { &self.mode }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct RedisMode {
    /// standalone | sentinel | cluster
    #[serde(default)]
    pub mode: String,
    /// Sentinel 이 감시하는 마스터 이름
    #[serde(default)]
    pub sentinel_master: String,
    /// sentinel: Sentinel 노드들 / cluster: 추가 시드 노드들 ("host:port", 쉼표·공백 구분)
    #[serde(default)]
    pub nodes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConnOptions {
    /// 새 콘솔의 기본 Tx 모드 (Transaction control: Manual)
    #[serde(default)]
    pub tx_manual: bool,
    /// 기본 격리 수준: "read uncommitted" | "read committed" | "repeatable read" | "serializable" (None = 서버 기본)
    #[serde(default)]
    pub isolation: Option<String>,
    /// Switch schema: "auto" | "manual" | "disable" (프론트에서 사용)
    #[serde(default)]
    pub switch_schema: Option<String>,
    /// 세션 TimeZone (None = 서버 기본)
    #[serde(default)]
    pub time_zone: Option<String>,
    /// Run keep-alive query each N seconds
    #[serde(default)]
    pub keep_alive_secs: Option<u32>,
    /// Auto-disconnect after N seconds (유휴)
    #[serde(default)]
    pub auto_disconnect_secs: Option<u32>,
    /// Single database mode: 기본 DB만 보이고 쓴다
    #[serde(default)]
    pub single_db: bool,
    /// Startup script: 연결마다 접속 직후 실행
    #[serde(default)]
    pub startup_script: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Scope {
    /// None = 모든 DB
    #[serde(default)]
    pub databases: Option<Vec<String>>,
    /// DB 이름 → 보일 스키마 (없으면 전부)
    #[serde(default)]
    pub schemas: std::collections::HashMap<String, Vec<String>>,
}

pub const SCOPE_ALL: &str = "*";
pub const SCOPE_DEFAULT: &str = "@default";

/// 트리에 보일 DB·스키마 규칙 (GUI src/lib/api.ts 의 dbVisible / schemaVisible 와 같다)
impl Scope {
    pub fn db_visible(&self, db: &str, default_db: &str) -> bool {
        match self.databases.as_deref() {
            None | Some([]) => true,
            Some(d) => d.iter().any(|x| x == SCOPE_ALL || x == db || (x == SCOPE_DEFAULT && db == default_db)),
        }
    }
    pub fn schema_visible(&self, db: &str, schema: &str, is_system: bool, show_system: bool) -> bool {
        match self.schemas.get(db).map(|v| v.as_slice()) {
            None | Some([]) => show_system || !is_system,
            Some(s) if s.iter().any(|x| x == SCOPE_ALL) => show_system || !is_system,
            Some(s) => s.iter().any(|x| x == schema || (x == SCOPE_DEFAULT && schema == "public")),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum SshAuth {
    #[default]
    Password,
    Key,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SshConfig {
    #[serde(default)]
    pub enabled: bool,
    pub host: String,
    #[serde(default = "default_ssh_port")]
    pub port: u16,
    pub user: String,
    #[serde(default)]
    pub auth: SshAuth,
    #[serde(default)]
    pub key_path: String,
}

fn default_ssh_port() -> u16 {
    22
}

impl ConnectionConfig {
    pub fn active_ssl(&self) -> Option<&SslConfig> {
        self.ssl.as_ref().filter(|s| s.enabled)
    }

    pub fn redis_mode(&self) -> &str {
        match self.redis.as_ref().map(|r| r.mode.as_str()) {
            Some("sentinel") => "sentinel",
            Some("cluster") => "cluster",
            _ => "standalone",
        }
    }

    pub fn active_ssh(&self) -> Option<&SshConfig> {
        self.ssh.as_ref().filter(|s| s.enabled && !s.host.is_empty())
    }
}

/// SSH 비밀번호/키 passphrase 의 저장 키
pub fn ssh_secret_key(id: &str) -> String {
    format!("{id}#ssh")
}

/// Export 파일 형식. 비밀번호는 포함하지 않는다.
#[derive(Debug, Serialize, Deserialize)]
pub struct ExportFile {
    pub app: String,
    pub version: u32,
    pub connections: Vec<ConnectionConfig>,
}

pub struct Store {
    path: PathBuf,
}

impl Store {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            path: dir.join("connections.json"),
        }
    }

    pub fn list(&self) -> AppResult<Vec<ConnectionConfig>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        Ok(serde_json::from_str(&fs::read_to_string(&self.path)?)?)
    }

    pub fn get(&self, id: &str) -> AppResult<Option<ConnectionConfig>> {
        Ok(self.list()?.into_iter().find(|c| c.id == id))
    }

    pub fn save(&self, config: &ConnectionConfig) -> AppResult<()> {
        let mut all = self.list()?;
        match all.iter_mut().find(|c| c.id == config.id) {
            Some(existing) => *existing = config.clone(),
            None => all.push(config.clone()),
        }
        self.write(&all)
    }

    pub fn delete(&self, id: &str) -> AppResult<()> {
        let all: Vec<_> = self.list()?.into_iter().filter(|c| c.id != id).collect();
        self.write(&all)
    }

    pub fn export_to(&self, path: &std::path::Path) -> AppResult<usize> {
        let connections = self.list()?;
        let n = connections.len();
        let file = ExportFile { app: "dbtool".into(), version: 1, connections };
        fs::write(path, serde_json::to_string_pretty(&file)?)?;
        Ok(n)
    }

    /// 같은 id는 덮어쓰고 새 id는 추가한다. 반환값: (추가, 갱신)
    pub fn import_from(&self, path: &std::path::Path) -> AppResult<(usize, usize)> {
        let text = fs::read_to_string(path)?;
        let incoming: Vec<ConnectionConfig> = match serde_json::from_str::<ExportFile>(&text) {
            Ok(f) => f.connections,
            Err(_) => serde_json::from_str(&text)?,
        };
        let mut all = self.list()?;
        let (mut added, mut updated) = (0, 0);
        for mut c in incoming {
            if c.id.is_empty() {
                c.id = uuid::Uuid::new_v4().to_string();
            }
            match all.iter_mut().find(|e| e.id == c.id) {
                Some(e) => {
                    *e = c;
                    updated += 1;
                }
                None => {
                    all.push(c);
                    added += 1;
                }
            }
        }
        self.write(&all)?;
        Ok((added, updated))
    }

    fn write(&self, all: &[ConnectionConfig]) -> AppResult<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(&self.path, serde_json::to_string_pretty(all)?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_rules_match_gui() {
        let sc = Scope {
            databases: Some(vec!["sales".into(), "billing".into()]),
            schemas: [("billing".to_string(), vec!["public".to_string(), "crm".to_string()])].into_iter().collect(),
        };
        assert!(!sc.db_visible("postgres", "postgres"), "default DB hidden when not selected");
        assert!(sc.db_visible("billing", "postgres"));
        assert!(sc.schema_visible("billing", "crm", false, false));
        assert!(!sc.schema_visible("billing", "audit", false, false));
        assert!(sc.schema_visible("sales", "anything", false, false));
        assert!(!sc.schema_visible("sales", "pg_catalog", true, false));
        let def = Scope { databases: Some(vec![SCOPE_DEFAULT.into()]), ..Default::default() };
        assert!(def.db_visible("postgres", "postgres") && !def.db_visible("x", "postgres"));
        assert!(Scope::default().db_visible("anything", "postgres"));
    }

    fn conn(id: &str, name: &str) -> ConnectionConfig {
        ConnectionConfig {
            id: id.into(),
            name: name.into(),
            kind: DbKind::Postgres,
            host: "localhost".into(),
            port: 5432,
            user: "u".into(),
            database: String::new(),
            color: Some("#5c2b2b".into()),
            ssh: None,
            password_saved: false,
            ssh_secret_saved: false,
            scope: Default::default(), read_only: false, options: Default::default(), ssl: None, redis: None,
        }
    }

    #[test]
    fn export_import_roundtrip() {
        let base = std::env::temp_dir().join(format!("dbtool-store-{}", uuid::Uuid::new_v4()));
        let a = Store::new(base.join("a"));
        a.save(&conn("1", "DEV")).unwrap();
        a.save(&conn("2", "PROD")).unwrap();
        let file = base.join("export.json");
        std::fs::create_dir_all(&base).unwrap();
        assert_eq!(a.export_to(&file).unwrap(), 2);

        let b = Store::new(base.join("b"));
        b.save(&conn("2", "old name")).unwrap();
        assert_eq!(b.import_from(&file).unwrap(), (1, 1));
        let names: Vec<_> = b.list().unwrap().into_iter().map(|c| c.name).collect();
        assert_eq!(names, vec!["PROD", "DEV"]);
        assert_eq!(b.get("1").unwrap().unwrap().color.as_deref(), Some("#5c2b2b"));
        let _ = std::fs::remove_dir_all(&base);
    }
}
