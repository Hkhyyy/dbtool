//! dbtool 엔진. GUI(Tauri) · TUI · MCP 서버가 모두 이 crate 의 `app::AppState` 를 쓴다.
/// 진단 로그 끄기 (TUI 는 화면을 쓰므로 stderr 로그를 막는다)
pub static QUIET: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[macro_export]
macro_rules! log {
    ($($t:tt)*) => {
        if !$crate::QUIET.load(std::sync::atomic::Ordering::Relaxed) {
            eprintln!($($t)*);
        }
    };
}

pub mod app;
pub mod db;
pub mod error;
pub mod meta;
pub mod mysql;
pub mod redisdb;
pub mod secrets;
pub mod session;
pub mod sql;
pub mod store;
pub mod tunnel;

/// Tauri 앱과 같은 설정 폴더 (Data Source 목록·암호화된 비밀번호 공유)
/// macOS: ~/Library/Application Support/com.hyunkyo.dbtool
pub const APP_IDENTIFIER: &str = "com.hyunkyo.dbtool";

pub fn config_dir() -> std::path::PathBuf {
    dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from(".")).join(APP_IDENTIFIER)
}

/// 설정 폴더의 Data Source·비밀번호로 앱 상태를 연다
pub fn open_state(dir: Option<std::path::PathBuf>) -> error::AppResult<app::AppState> {
    let dir = dir.unwrap_or_else(config_dir);
    std::fs::create_dir_all(&dir)?;
    let secrets = secrets::SecretFile::open(&dir)?;
    Ok(app::AppState::new(store::Store::new(dir), secrets))
}
