//! 앱 상태: Data Source 풀·세션·SSH 터널·비밀번호. GUI(Tauri)·TUI·MCP 가 같은 메서드를 쓴다.
use crate::db::{self, Pool, QueryResult};
use crate::error::{AppError, AppResult};
use crate::session::{self, Session};
use crate::store::{self, ConnectionConfig, Store};
use serde::Serialize;
use std::collections::HashMap;
use tokio::sync::RwLock;

pub struct SessionEntry {
    pub conn_id: String,
    pub pid: i64,
    pub session: std::sync::Arc<tokio::sync::Mutex<Session>>,
}

pub struct AppState {
    pub store: Store,
    pub pools: RwLock<HashMap<String, Pool>>,
    /// 콘솔 탭 id → 세션
    pub sessions: RwLock<HashMap<String, SessionEntry>>,
    /// 접속 id → SSH 터널 (풀과 수명을 같이 함)
    pub tunnels: RwLock<HashMap<String, crate::tunnel::Tunnel>>,
    /// 키체인에서 한 번 읽은 비밀값 (앱이 켜져 있는 동안 다시 묻지 않도록)
    pub secrets: std::sync::Mutex<HashMap<String, Option<String>>>,
    /// 비밀번호 저장소 (암호화 파일). 키체인 팝업을 피한다.
    pub secret_file: crate::secrets::SecretFile,
    /// Data Source id → 마지막 사용 시각 (Auto-disconnect)
    pub last_used: std::sync::Mutex<HashMap<String, std::time::Instant>>,
    /// Data Source id → 마지막 keep-alive 시각
    pub last_ping: std::sync::Mutex<HashMap<String, std::time::Instant>>,
}

impl AppState {
    /// 키체인 조회는 처음 한 번만. 이후에는 메모리 캐시.
    pub fn secret(&self, key: &str) -> AppResult<Option<String>> {
        if let Some(v) = self.secrets.lock().unwrap().get(key) {
            return Ok(v.clone());
        }
        // 키체인은 절대 읽지 않는다 (macOS 팝업 방지). 암호화 파일만 사용.
        let v = self.secret_file.get(key);
        self.secrets.lock().unwrap().insert(key.to_string(), v.clone());
        Ok(v)
    }

    pub fn set_secret(&self, key: &str, value: Option<&str>) -> AppResult<()> {
        self.secret_file.set(key, value)?;
        self.secrets.lock().unwrap().insert(key.to_string(), value.map(str::to_string));
        Ok(())
    }
}

impl AppState {
    pub fn new(store: Store, secret_file: crate::secrets::SecretFile) -> Self {
        Self { store, pools: RwLock::new(HashMap::new()), sessions: RwLock::new(HashMap::new()), tunnels: RwLock::new(HashMap::new()), secrets: Default::default(), secret_file, last_used: Default::default(), last_ping: Default::default() }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionView {
    #[serde(flatten)]
    pub config: ConnectionConfig,
    pub has_password: bool,
    pub connected: bool,
}

/// 접속 대상 id: "<data source id>" 또는 "<data source id>::<database>"
pub fn split_target(id: &str) -> (&str, Option<&str>) {
    match id.split_once("::") {
        Some((base, db)) if !db.is_empty() => (base, Some(db)),
        _ => (id, None),
    }
}

pub fn belongs(key: &str, id: &str) -> bool {
    key == id || key.starts_with(&format!("{id}::"))
}

/// 해당 Data Source의 모든 DB 풀·세션·터널을 닫는다.
pub async fn drop_pool(state: &AppState, id: &str) {
    state.sessions.write().await.retain(|_, e| !belongs(&e.conn_id, id));
    let removed: Vec<Pool> = {
        let mut pools = state.pools.write().await;
        let keys: Vec<String> = pools.keys().filter(|k| belongs(k, id)).cloned().collect();
        keys.into_iter().filter_map(|k| pools.remove(&k)).collect()
    };
    for p in removed {
        p.close().await;
    }
    state.tunnels.write().await.retain(|k, _| !belongs(k, id));
}

/// SSH 설정이 있으면 터널을 먼저 연 뒤 접속한다.
pub async fn connect_config(
    cfg: &ConnectionConfig,
    password: Option<&str>,
    ssh_secret: Option<&str>,
) -> AppResult<(Pool, Option<crate::tunnel::Tunnel>)> {
    match cfg.active_ssh() {
        Some(ssh) => {
            let t = crate::tunnel::open(ssh, ssh_secret, &cfg.host, cfg.port).await?;
            let pool = db::connect_via(cfg, password, Some(t.local_port)).await?;
            Ok((pool, Some(t)))
        }
        None => Ok((db::connect(cfg, password).await?, None)),
    }
}

pub fn touch(state: &AppState, id: &str) {
    let base = split_target(id).0.to_string();
    state.last_used.lock().unwrap().insert(base, std::time::Instant::now());
}

/// 한 번 돌 때마다: keep-alive 쿼리 / 유휴 접속 끊기. 끊은 Data Source id 목록을 돌려준다.
pub async fn housekeeping(state: &AppState) -> Vec<String> {
    use std::time::{Duration, Instant};
    let bases: Vec<String> = {
        let pools = state.pools.read().await;
        let mut b: Vec<String> = pools.keys().map(|k| split_target(k).0.to_string()).collect();
        b.sort();
        b.dedup();
        b
    };
    let mut dropped = Vec::new();
    for base in bases {
        let Ok(Some(cfg)) = state.store.get(&base) else { continue };
        let now = Instant::now();
        if let Some(secs) = cfg.options.auto_disconnect_secs.filter(|s| *s > 0) {
            let last = *state.last_used.lock().unwrap().entry(base.clone()).or_insert(now);
            // 실행 중인 쿼리(세션 잠김)가 있으면 끊지 않는다
            let busy = state.sessions.read().await.values().any(|e| belongs(&e.conn_id, &base) && e.session.try_lock().is_err());
            if !busy && now.duration_since(last) >= Duration::from_secs(secs as u64) {
                drop_pool(state, &base).await;
                state.last_used.lock().unwrap().remove(&base);
                dropped.push(base);
                continue;
            }
        }
        if let Some(secs) = cfg.options.keep_alive_secs.filter(|s| *s > 0) {
            let due = {
                let mut m = state.last_ping.lock().unwrap();
                let last = *m.entry(base.clone()).or_insert(now);
                if now.duration_since(last) >= Duration::from_secs(secs as u64) {
                    m.insert(base.clone(), now);
                    true
                } else {
                    false
                }
            };
            if due {
                let pools: Vec<Pool> = state.pools.read().await.iter().filter(|(k, _)| belongs(k, &base)).map(|(_, p)| p.clone()).collect();
                for pool in pools {
                    pool.ping().await;
                }
                let sessions: Vec<_> = state.sessions.read().await.values().filter(|e| belongs(&e.conn_id, &base)).map(|e| e.session.clone()).collect();
                for s in sessions {
                    if let Ok(mut g) = s.try_lock() {
                        g.ping().await;
                    }
                }
            }
        }
    }
    dropped
}

pub async fn ensure_pool(state: &AppState, id: &str) -> AppResult<Pool> {
    touch(state, id);
    if let Some(p) = state.pools.read().await.get(id) {
        return Ok(p.clone());
    }
    let (base, db) = split_target(id);
    let mut cfg = state
        .store
        .get(base)?
        .ok_or_else(|| AppError::Msg("Data source not found".into()))?;
    if let Some(db) = db {
        cfg.database = db.to_string();
    }
    let password = state.secret(base)?;
    let ssh_secret = if cfg.active_ssh().is_some() { state.secret(&store::ssh_secret_key(base))? } else { None };
    let (pool, tunnel) = connect_config(&cfg, password.as_deref(), ssh_secret.as_deref()).await?;
    if let Some(t) = tunnel {
        state.tunnels.write().await.insert(id.to_string(), t);
    }
    state
        .pools
        .write()
        .await
        .insert(id.to_string(), pool.clone());
    Ok(pool)
}

pub async fn ensure_session(state: &AppState, id: &str, tab_id: &str) -> AppResult<std::sync::Arc<tokio::sync::Mutex<Session>>> {
    if let Some(e) = state.sessions.read().await.get(tab_id) {
        if e.conn_id == id {
            return Ok(e.session.clone());
        }
    }
    let pool = ensure_pool(state, id).await?;
    let s = Session::open(&pool).await?;
    let entry = SessionEntry { conn_id: id.to_string(), pid: s.backend_id(), session: std::sync::Arc::new(tokio::sync::Mutex::new(s)) };
    let arc = entry.session.clone();
    state.sessions.write().await.insert(tab_id.to_string(), entry);
    Ok(arc)
}

pub async fn redis_pool(state: &AppState, id: &str) -> AppResult<crate::redisdb::RedisConn> {
    match ensure_pool(state, id).await? {
        Pool::Redis(m, _) => Ok(m),
        _ => Err(AppError::Msg("Not a Redis data source".into())),
    }
}

#[derive(Serialize)]
pub struct ImportResult {
    pub added: usize,
    pub updated: usize,
}

impl AppState {
    /// 읽기 전용으로 실행 (MCP 기본). PG·MySQL: READ ONLY 트랜잭션 안에서 실행 후 롤백. Redis: 읽기 명령만
    pub async fn run_read_only(&self, id: String, sql: String, limit: usize) -> AppResult<QueryResult> {
        use sqlx::Executor;
        let kind = self.kind_of(&id)?;
        if kind != crate::store::DbKind::Redis {
            // 한 문장만, 읽기 키워드로 시작해야 한다 (COMMIT; DELETE … 같은 트랜잭션 탈출 방지)
            let stmts = crate::sql::split_statements(&sql, kind);
            if stmts.len() != 1 {
                return Err(AppError::Msg("Read-only mode runs exactly one statement".into()));
            }
            let first = session::strip_comments_lead(&sql);
            let word = first.split(|c: char| !c.is_ascii_alphabetic()).next().unwrap_or("");
            if !["select", "with", "values", "table", "show", "explain", "describe", "desc"].contains(&word) {
                return Err(AppError::Msg(format!("Read-only mode: '{word}' statements are not allowed (start the MCP server with --allow-write)")));
            }
        }
        match ensure_pool(self, &id).await? {
            Pool::Pg(p) => {
                let mut tx = p.begin().await?;
                tx.execute("SET TRANSACTION READ ONLY").await?;
                let r = db::run_on(&mut tx, &sql, limit).await;
                let _ = tx.rollback().await;
                r
            }
            Pool::My(p) => {
                let mut conn = p.acquire().await?;
                conn.execute("START TRANSACTION READ ONLY").await?;
                let r = crate::mysql::run_on(&mut conn, &sql, limit).await;
                let _ = conn.execute("ROLLBACK").await;
                r
            }
            Pool::Redis(m, _) => {
                let args = crate::redisdb::parse_command(&sql)?;
                let cmd = String::from_utf8_lossy(&args[0]).to_uppercase();
                let sub = args.get(1).map(|a| String::from_utf8_lossy(a).to_uppercase()).unwrap_or_default();
                if !crate::redisdb::is_read_command(&cmd, &sub) {
                    return Err(AppError::Msg(format!("{cmd} is not a read-only command")));
                }
                crate::redisdb::run(&m, &sql, limit).await
            }
        }
    }

    /// 데이터 소스 종류
    pub fn kind_of(&self, id: &str) -> AppResult<crate::store::DbKind> {
        let (base, _) = split_target(id);
        Ok(self.store.get(base)?.ok_or_else(|| AppError::Msg(format!("Data source not found: {base}")))?.kind)
    }
}

impl AppState {
    pub async fn list_connections(&self) -> AppResult<Vec<ConnectionView>> {
        crate::log!("[dbtool] list_connections");
        let pools = self.pools.read().await;
        self
            .store
            .list()?
            .into_iter()
            .map(|c| {
                Ok(ConnectionView {
                    has_password: c.password_saved || self.secret_file.contains(&c.id),
                    connected: pools.keys().any(|k| belongs(k, &c.id)),
                    config: c,
                })
            })
            .collect()
    }

    /// `password`: None이면 기존 비밀번호 유지, 빈 문자열이면 삭제.
    pub async fn save_connection(&self, mut config: ConnectionConfig, password: Option<String>, ssh_secret: Option<String>) -> AppResult<ConnectionConfig> {
        crate::log!("[dbtool] save_connection host={} user={} db={}", config.host, config.user, config.database);
        if config.id.is_empty() {
            config.id = uuid::Uuid::new_v4().to_string();
        }
        // 저장 여부 플래그는 서버가 관리한다 (None = 기존 값 유지)
        let prev = self.store.get(&config.id)?;
        config.password_saved = prev.as_ref().is_some_and(|p| p.password_saved);
        config.ssh_secret_saved = prev.as_ref().is_some_and(|p| p.ssh_secret_saved);
        match password.as_deref() {
            Some("") => {
                self.set_secret(&config.id, None)?;
                config.password_saved = false;
            }
            Some(p) => {
                self.set_secret(&config.id, Some(p))?;
                config.password_saved = true;
            }
            None => {}
        }
        let sk = store::ssh_secret_key(&config.id);
        match ssh_secret.as_deref() {
            Some("") => {
                self.set_secret(&sk, None)?;
                config.ssh_secret_saved = false;
            }
            Some(p) => {
                self.set_secret(&sk, Some(p))?;
                config.ssh_secret_saved = true;
            }
            None => {}
        }
        self.store.save(&config)?;
        drop_pool(self, &config.id).await; // 설정이 바뀌었을 수 있으므로 다시 접속하게 한다
        Ok(config)
    }

    pub async fn delete_connection(&self, id: String) -> AppResult<()> {
        drop_pool(self, &id).await;
        self.set_secret(&id, None)?;
        self.set_secret(&store::ssh_secret_key(&id), None)?;
        self.store.delete(&id)
    }

    /// 저장하지 않고 접속만 시험한다. `password`가 None이면 저장된 비밀번호를 쓴다.
    pub async fn test_connection(&self, config: ConnectionConfig, password: Option<String>, ssh_secret: Option<String>) -> AppResult<String> {
        let password = match password {
            Some(p) => Some(p),
            None if !config.id.is_empty() => self.secret(&config.id)?,
            None => None,
        };
        let ssh_secret = match ssh_secret {
            Some(p) => Some(p),
            None if !config.id.is_empty() && config.active_ssh().is_some() => self.secret(&store::ssh_secret_key(&config.id))?,
            None => None,
        };
        let (pool, tunnel) = connect_config(&config, password.as_deref(), ssh_secret.as_deref()).await?;
        let version = pool.server_version().await;
        pool.close().await;
        drop(tunnel);
        let via = config.active_ssh().map(|s| format!("  (via SSH {}@{})", s.user, s.host)).unwrap_or_default();
        version.map(|v| format!("{v}{via}"))
    }

    /// SSH/SSL 탭의 Test Connection: SSH 로그인만 확인한다.
    pub async fn test_ssh(&self, config: ConnectionConfig, ssh_secret: Option<String>) -> AppResult<String> {
        let ssh = config.ssh.as_ref().filter(|s| !s.host.is_empty()).ok_or_else(|| AppError::Msg("Enter the SSH host".into()))?;
        let secret = match ssh_secret {
            Some(p) => Some(p),
            None if !config.id.is_empty() => self.secret(&store::ssh_secret_key(&config.id))?,
            None => None,
        };
        crate::tunnel::test_login(ssh, secret.as_deref()).await
    }

    pub async fn connect(&self, id: String) -> AppResult<String> {
        crate::log!("[dbtool] connect {id}");
        ensure_pool(self, &id).await?.server_version().await
    }

    pub async fn disconnect(&self, id: String) -> AppResult<()> {
        drop_pool(self, &id).await;
        Ok(())
    }

    /// `tab_id`가 있으면 그 콘솔의 세션에서 실행한다 (커서·취소 지원).
    pub async fn run_query(&self, id: String, sql: String, limit: Option<usize>, tab_id: Option<String>) -> AppResult<QueryResult> {
        let limit = limit.unwrap_or(500);
        match tab_id {
            Some(tab) => {
                let sess = ensure_session(self, &id, &tab).await?;
                let mut s = sess.lock().await;
                s.run(&sql, limit).await
            }
            None => ensure_pool(self, &id).await?.run_query(&sql, limit).await,
        }
    }

    pub async fn fetch_more(&self, tab_id: String, limit: Option<usize>) -> AppResult<QueryResult> {
        if let Some(e) = self.sessions.read().await.get(&tab_id) {
            touch(self, &e.conn_id);
        }
        let sess = self.sessions.read().await.get(&tab_id).map(|e| e.session.clone()).ok_or_else(|| AppError::Msg("Session closed".into()))?;
        let mut s = sess.lock().await;
        s.fetch(limit.unwrap_or(500)).await
    }

    pub async fn cancel_query(&self, tab_id: String) -> AppResult<bool> {
        let (conn_id, pid) = match self.sessions.read().await.get(&tab_id) {
            Some(e) => (e.conn_id.clone(), e.pid),
            None => return Ok(false),
        };
        match ensure_pool(self, &conn_id).await? {
            Pool::Pg(pg) => session::cancel(&pg, pid as i32).await,
            Pool::My(my) => crate::mysql::cancel(&my, pid).await,
            Pool::Redis(..) => Ok(false),
        }
    }

    /// Tx: Auto / Manual
    pub async fn set_tx_mode(&self, id: String, tab_id: String, manual: bool) -> AppResult<()> {
        let sess = ensure_session(self, &id, &tab_id).await?;
        let mut s = sess.lock().await;
        s.set_manual(manual).await
    }

    pub async fn commit_tx(&self, tab_id: String) -> AppResult<()> {
        let sess = self.sessions.read().await.get(&tab_id).map(|e| e.session.clone()).ok_or_else(|| AppError::Msg("Session closed".into()))?;
        let mut s = sess.lock().await;
        s.commit().await
    }

    pub async fn rollback_tx(&self, tab_id: String) -> AppResult<()> {
        let sess = self.sessions.read().await.get(&tab_id).map(|e| e.session.clone()).ok_or_else(|| AppError::Msg("Session closed".into()))?;
        let mut s = sess.lock().await;
        s.rollback_tx().await
    }

    pub async fn close_session(&self, tab_id: String) -> AppResult<()> {
        if let Some(e) = self.sessions.write().await.remove(&tab_id) {
            e.session.lock().await.close_cursor().await;
        }
        Ok(())
    }

    pub async fn execute_batch(&self, id: String, statements: Vec<String>) -> AppResult<u64> {
        match ensure_pool(self, &id).await? {
            Pool::Pg(pg) => session::execute_batch(&pg, &statements).await,
            Pool::My(my) => crate::mysql::execute_batch(&my, &statements).await,
            Pool::Redis(m, _) => {
                for st in &statements {
                    crate::redisdb::run(&m, st, 1).await?;
                }
                Ok(statements.len() as u64)
            }
        }
    }

    pub async fn object_index(&self, id: String) -> AppResult<Vec<crate::meta::IndexEntry>> {
        crate::meta::object_index(&ensure_pool(self, &id).await?).await
    }

    pub async fn full_text_search(&self, id: String, req: crate::meta::FtsRequest) -> AppResult<Vec<crate::meta::FtsHit>> {
        crate::meta::full_text_search(&ensure_pool(self, &id).await?, &req).await
    }

    pub async fn redis_scan(&self, id: String, cursor: String, pattern: String, count: usize, type_filter: Option<String>) -> AppResult<crate::redisdb::ScanPage> {
        crate::redisdb::scan_page(&redis_pool(self, &id).await?, &cursor, &pattern, count, type_filter.as_deref()).await
    }

    pub async fn redis_key(&self, id: String, key: String, limit: Option<usize>) -> AppResult<crate::redisdb::KeyDetail> {
        crate::redisdb::key_detail(&redis_pool(self, &id).await?, &key, limit.unwrap_or(1000)).await
    }

    /// 터미널: redis-cli 처럼 한 줄을 그대로
    pub async fn redis_exec_line(&self, id: String, line: String) -> AppResult<serde_json::Value> {
        let args: Vec<String> = crate::redisdb::parse_command(&line)?.into_iter().map(|a| String::from_utf8_lossy(&a).into_owned()).collect();
        crate::redisdb::exec_args(&redis_pool(self, &id).await?, &args).await
    }

    pub async fn redis_exec(&self, id: String, args: Vec<String>) -> AppResult<serde_json::Value> {
        crate::redisdb::exec_args(&redis_pool(self, &id).await?, &args).await
    }

    pub async fn export_connections(&self, path: String) -> AppResult<usize> {
        self.store.export_to(std::path::Path::new(&path))
    }

    pub async fn import_connections(&self, path: String) -> AppResult<ImportResult> {
        let (added, updated) = self.store.import_from(std::path::Path::new(&path))?;
        Ok(ImportResult { added, updated })
    }

    pub async fn export_csv(&self, id: String, sql: String, path: String) -> AppResult<u64> {
        let pool = ensure_pool(self, &id).await?;
        pool.export_csv(&sql, std::path::Path::new(&path)).await
    }

    pub async fn list_databases(&self, id: String) -> AppResult<Vec<String>> {
        crate::log!("[dbtool] list_databases {id}");
        let r = async { crate::meta::list_databases(&ensure_pool(self, &id).await?).await }.await;
        if let Err(e) = &r {
            crate::log!("[dbtool] list_databases error: {e}");
        }
        r
    }

    pub async fn list_schemas(&self, id: String) -> AppResult<Vec<crate::meta::SchemaInfo>> {
        crate::log!("[dbtool] list_schemas {id}");
        let r = async { crate::meta::list_schemas(&ensure_pool(self, &id).await?).await }.await;
        match &r {
            Ok(v) => crate::log!("[dbtool] list_schemas → {} schemas", v.len()),
            Err(e) => crate::log!("[dbtool] list_schemas error: {e}"),
        }
        r
    }

    pub async fn list_objects(&self, id: String, schema: String) -> AppResult<crate::meta::SchemaObjects> {
        crate::meta::list_objects(&ensure_pool(self, &id).await?, &schema).await
    }

    pub async fn table_detail(&self, id: String, schema: String, table: String) -> AppResult<crate::meta::TableDetail> {
        crate::meta::table_detail(&ensure_pool(self, &id).await?, &schema, &table).await
    }

    pub async fn object_ddl(&self, id: String, schema: String, name: String, kind: String) -> AppResult<String> {
        crate::meta::object_ddl(&ensure_pool(self, &id).await?, &schema, &name, &kind).await
    }

    pub async fn completion_meta(&self, id: String) -> AppResult<Vec<crate::meta::CompletionTable>> {
        crate::meta::completion_meta(&ensure_pool(self, &id).await?).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Auto-disconnect: 유휴 시간이 지나면 풀·세션을 닫는다. keep-alive 는 세션을 살려 둔다.
    #[tokio::test]
    #[ignore]
    async fn housekeeping_keepalive_and_auto_disconnect() {
        let dir = std::env::temp_dir().join(format!("dbtool-hk-{}", uuid::Uuid::new_v4()));
        let state = AppState::new(Store::new(dir.clone()), crate::secrets::SecretFile::open(&dir).unwrap());
        let mut cfg = ConnectionConfig {
            id: "hk".into(), name: "hk".into(), kind: crate::store::DbKind::Postgres, host: "localhost".into(), port: 5432,
            user: std::env::var("USER").unwrap(), database: "postgres".into(), color: None, ssh: None, password_saved: false,
            ssh_secret_saved: false, scope: Default::default(), read_only: false, options: Default::default(), ssl: None, redis: None,
        };
        cfg.options.keep_alive_secs = Some(1);
        cfg.options.auto_disconnect_secs = Some(2);
        state.store.save(&cfg).unwrap();
        let sess = ensure_session(&state, "hk", "tab1").await.unwrap();
        drop(sess);
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
        assert!(housekeeping(&state).await.is_empty(), "not idle long enough yet");
        assert!(state.last_ping.lock().unwrap().contains_key("hk"), "keep-alive ran");
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
        assert_eq!(housekeeping(&state).await, vec!["hk".to_string()]);
        assert!(state.pools.read().await.is_empty() && state.sessions.read().await.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}
