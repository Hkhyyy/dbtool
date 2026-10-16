//! 프론트엔드에서 호출하는 Tauri 커맨드. 실제 동작은 dbtool_core::app::AppState 에 있다.
use crate::app::{AppState, ConnectionView, ImportResult};
use crate::db::QueryResult;
use crate::error::AppResult;
use crate::store::ConnectionConfig;
use tauri::State;
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub async fn list_connections(state: State<'_, AppState>) -> AppResult<Vec<ConnectionView>> {
    state.list_connections().await
}

/// `password`: None이면 기존 비밀번호 유지, 빈 문자열이면 삭제.
#[tauri::command]
pub async fn save_connection(state: State<'_, AppState>, config: ConnectionConfig, password: Option<String>, ssh_secret: Option<String>) -> AppResult<ConnectionConfig> {
    state.save_connection(config, password, ssh_secret).await
}

#[tauri::command]
pub async fn delete_connection(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.delete_connection(id).await
}

/// 저장하지 않고 접속만 시험한다. `password`가 None이면 저장된 비밀번호를 쓴다.
#[tauri::command]
pub async fn test_connection(state: State<'_, AppState>, config: ConnectionConfig, password: Option<String>, ssh_secret: Option<String>) -> AppResult<String> {
    state.test_connection(config, password, ssh_secret).await
}

/// SSH/SSL 탭의 Test Connection: SSH 로그인만 확인한다.
#[tauri::command]
pub async fn test_ssh(state: State<'_, AppState>, config: ConnectionConfig, ssh_secret: Option<String>) -> AppResult<String> {
    state.test_ssh(config, ssh_secret).await
}

#[tauri::command]
pub async fn connect(state: State<'_, AppState>, id: String) -> AppResult<String> {
    state.connect(id).await
}

#[tauri::command]
pub async fn disconnect(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.disconnect(id).await
}

/// `tab_id`가 있으면 그 콘솔의 세션에서 실행한다 (커서·취소 지원).
#[tauri::command]
pub async fn run_query(state: State<'_, AppState>, id: String, sql: String, limit: Option<usize>, tab_id: Option<String>) -> AppResult<QueryResult> {
    state.run_query(id, sql, limit, tab_id).await
}

#[tauri::command]
pub async fn fetch_more(state: State<'_, AppState>, tab_id: String, limit: Option<usize>) -> AppResult<QueryResult> {
    state.fetch_more(tab_id, limit).await
}

#[tauri::command]
pub async fn cancel_query(state: State<'_, AppState>, tab_id: String) -> AppResult<bool> {
    state.cancel_query(tab_id).await
}

/// Tx: Auto / Manual
#[tauri::command]
pub async fn set_tx_mode(state: State<'_, AppState>, id: String, tab_id: String, manual: bool) -> AppResult<()> {
    state.set_tx_mode(id, tab_id, manual).await
}

#[tauri::command]
pub async fn commit_tx(state: State<'_, AppState>, tab_id: String) -> AppResult<()> {
    state.commit_tx(tab_id).await
}

#[tauri::command]
pub async fn rollback_tx(state: State<'_, AppState>, tab_id: String) -> AppResult<()> {
    state.rollback_tx(tab_id).await
}

#[tauri::command]
pub async fn close_session(state: State<'_, AppState>, tab_id: String) -> AppResult<()> {
    state.close_session(tab_id).await
}

#[tauri::command]
/// exact_one = 표 편집 저장 (UPDATE/DELETE 가 정확히 1행이 아니면 전부 롤백)
pub async fn execute_batch(state: State<'_, AppState>, id: String, statements: Vec<String>, exact_one: Option<bool>) -> AppResult<u64> {
    state.execute_batch(id, statements, exact_one.unwrap_or(false)).await
}

/// 접속하면 검색 캐시를 채운다 (프론트는 기다리지 않는다)
#[tauri::command]
pub async fn warm_search_cache(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.warm_search_cache(id).await
}

/// 접속하지 않은 Data Source 도 Search Everywhere 에서 찾도록 저장해 둔 객체·컬럼 목록
#[tauri::command]
pub fn cached_index(state: State<'_, AppState>) -> AppResult<Vec<dbtool_core::search_cache::CachedIndex>> {
    state.cached_index()
}

#[tauri::command]
pub async fn object_index(state: State<'_, AppState>, id: String) -> AppResult<Vec<crate::meta::IndexEntry>> {
    state.object_index(id).await
}

#[tauri::command]
pub async fn full_text_search(state: State<'_, AppState>, id: String, req: crate::meta::FtsRequest) -> AppResult<Vec<crate::meta::FtsHit>> {
    state.full_text_search(id, req).await
}

#[tauri::command]
pub async fn redis_scan(state: State<'_, AppState>, id: String, cursor: String, pattern: String, count: usize, type_filter: Option<String>) -> AppResult<crate::redisdb::ScanPage> {
    state.redis_scan(id, cursor, pattern, count, type_filter).await
}

#[tauri::command]
pub async fn redis_key(state: State<'_, AppState>, id: String, key: String, limit: Option<usize>) -> AppResult<crate::redisdb::KeyDetail> {
    state.redis_key(id, key, limit).await
}

/// 터미널: redis-cli 처럼 한 줄을 그대로
#[tauri::command]
pub async fn redis_exec_line(state: State<'_, AppState>, id: String, line: String) -> AppResult<serde_json::Value> {
    state.redis_exec_line(id, line).await
}

#[tauri::command]
pub async fn redis_exec(state: State<'_, AppState>, id: String, args: Vec<String>) -> AppResult<serde_json::Value> {
    state.redis_exec(id, args).await
}

#[tauri::command]
pub async fn export_connections(app: tauri::AppHandle, state: State<'_, AppState>) -> AppResult<Option<(usize, String)>> {
    let Some(path) = save_dialog(&app, "dbtool-datasources.json", "JSON", &["json"]) else { return Ok(None) };
    Ok(Some((state.export_connections(path.clone()).await?, path)))
}

/// 가져올 파일 고르기 → 무엇이 바뀌는지 미리보기. 실제 가져오기는 import_picked (사용자가 확인한 뒤)
#[tauri::command]
pub async fn pick_import_file(app: tauri::AppHandle, state: State<'_, AppState>, pending: State<'_, PendingImport>) -> AppResult<Option<Vec<String>>> {
    let picked = app.dialog().file().add_filter("JSON", &["json"]).blocking_pick_file();
    let Some(path) = picked.and_then(|p| p.into_path().ok()) else { return Ok(None) };
    let lines = state.import_preview(path.display().to_string())?;
    *pending.0.lock().unwrap() = Some(path);
    Ok(Some(lines))
}

#[tauri::command]
pub async fn import_picked(state: State<'_, AppState>, pending: State<'_, PendingImport>) -> AppResult<ImportResult> {
    let path = pending.0.lock().unwrap().take().ok_or_else(|| crate::error::AppError::Msg("Choose a file first".into()))?;
    state.import_connections(path.display().to_string()).await
}

/// 고른 가져오기 파일 (미리보기와 실제 가져오기 사이)
#[derive(Default)]
pub struct PendingImport(pub std::sync::Mutex<Option<std::path::PathBuf>>);

/// 저장 대화상자는 여기(백엔드)에서 연다: 웹뷰가 임의 경로를 넘겨 파일을 쓰지 못하게, 사용자가 고른 경로에만 쓴다
fn save_dialog(app: &tauri::AppHandle, default_name: &str, filter: &str, exts: &[&str]) -> Option<String> {
    app.dialog().file().set_file_name(default_name).add_filter(filter, exts).blocking_save_file().and_then(|p| p.into_path().ok()).map(|p| p.display().to_string())
}

/// SQL Generator·DDL "Save to File" → 저장한 경로 (취소하면 None)
#[tauri::command]
pub async fn save_text_file(app: tauri::AppHandle, default_name: String, text: String) -> AppResult<Option<String>> {
    let Some(path) = save_dialog(&app, &default_name, "SQL", &["sql"]) else { return Ok(None) };
    std::fs::write(&path, text)?;
    Ok(Some(path))
}

#[tauri::command]
pub async fn export_csv(app: tauri::AppHandle, state: State<'_, AppState>, id: String, sql: String, default_name: String) -> AppResult<Option<(u64, String)>> {
    let Some(path) = save_dialog(&app, &default_name, "CSV (Excel)", &["csv"]) else { return Ok(None) };
    Ok(Some((state.export_csv(id, sql, path.clone()).await?, path)))
}

/// 탭 세션에 커밋 안 한 변경이 있는지 (오류 뒤 Commit/Roll back 표시)
#[tauri::command]
pub async fn tx_state(state: State<'_, AppState>, tab_id: String) -> AppResult<bool> {
    Ok(state.tx_state(tab_id).await)
}

/// 처음 보는 SSH 서버 키를 신뢰 (사용자가 지문을 확인한 뒤)
#[tauri::command]
pub async fn trust_ssh_host(state: State<'_, AppState>, host: String, port: u16) -> AppResult<String> {
    state.trust_ssh_host(host, port)
}

/// 실행 전에 확인을 받을 위험한 문장이면 이유 (WHERE 없는 UPDATE/DELETE, DROP, TRUNCATE, Redis FLUSH*·DEL)
#[tauri::command]
pub fn unsafe_reason(kind: crate::store::DbKind, sql: String) -> Option<String> {
    dbtool_core::guard::unsafe_statement(&sql, kind)
}

#[tauri::command]
pub async fn list_databases(state: State<'_, AppState>, id: String) -> AppResult<Vec<String>> {
    state.list_databases(id).await
}

#[tauri::command]
pub async fn list_schemas(state: State<'_, AppState>, id: String) -> AppResult<Vec<crate::meta::SchemaInfo>> {
    state.list_schemas(id).await
}

#[tauri::command]
pub async fn list_objects(state: State<'_, AppState>, id: String, schema: String) -> AppResult<crate::meta::SchemaObjects> {
    state.list_objects(id, schema).await
}

#[tauri::command]
pub async fn table_detail(state: State<'_, AppState>, id: String, schema: String, table: String) -> AppResult<crate::meta::TableDetail> {
    state.table_detail(id, schema, table).await
}

#[tauri::command]
pub async fn object_ddl(state: State<'_, AppState>, id: String, schema: String, name: String, kind: String) -> AppResult<String> {
    state.object_ddl(id, schema, name, kind).await
}

#[tauri::command]
pub async fn completion_meta(state: State<'_, AppState>, id: String) -> AppResult<Vec<crate::meta::CompletionTable>> {
    state.completion_meta(id).await
}

/// 단축키 사용자 설정 (keymap.json 의 "gui" 구역)
#[tauri::command]
pub fn load_keymap(state: State<'_, AppState>) -> AppResult<crate::store::Keymap> {
    state.store.keymap("gui")
}

#[tauri::command]
pub fn save_keymap(state: State<'_, AppState>, map: crate::store::Keymap) -> AppResult<()> {
    state.store.save_keymap("gui", &map)
}

/// 새 버전 확인 결과 (current = 이 앱 버전)
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    current: String,
    latest: Option<dbtool_core::update::Release>,
}

/// 새 버전 확인. force 가 아니면 하루 한 번만 GitHub 에 묻는다. 개발 빌드(pnpm tauri dev)는 force 일 때만
#[tauri::command]
pub async fn check_update(state: State<'_, AppState>, force: bool) -> AppResult<UpdateInfo> {
    let current = env!("CARGO_PKG_VERSION").to_string();
    if (cfg!(debug_assertions) && !force) || (dbtool_core::update::disabled() && !force) {
        return Ok(UpdateInfo { current, latest: None });
    }
    let latest = dbtool_core::update::check(&state.store.dir(), &current, force).await?;
    Ok(UpdateInfo { current, latest })
}

#[cfg(test)]
mod ipc_tests {
    use super::*;
    use crate::store::Store;
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{get_ipc_response, INVOKE_KEY};
    use tauri::webview::InvokeRequest;

    fn req(cmd: &str, body: serde_json::Value) -> InvokeRequest {
        InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "tauri://localhost".parse().unwrap(),
            body: InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        }
    }

    /// 프론트엔드가 보내는 것과 같은 JSON으로 커맨드를 호출해 본다.
    #[test]
    fn save_and_list_via_ipc() {
        let dir = std::env::temp_dir().join(format!("dbtool-ipc-{}", uuid::Uuid::new_v4()));
        let app = tauri::test::mock_builder()
            .manage(AppState::new(Store::new(dir.clone()), crate::secrets::SecretFile::open(&dir).unwrap()))
            .invoke_handler(tauri::generate_handler![save_connection, list_connections, test_connection, list_databases, list_schemas, run_query])
            .build(crate::app_context())
            .unwrap();
        let wv = tauri::WebviewWindowBuilder::new(&app, "main", Default::default()).build().unwrap();

        let config = serde_json::json!({
            "id": "", "name": "", "kind": "postgres", "host": "localhost", "port": 5432,
            "user": std::env::var("USER").unwrap(), "database": "postgres", "color": null, "ssh": null, "passwordSaved": false
        });
        let t = get_ipc_response(&wv, req("test_connection", serde_json::json!({ "config": config, "password": null })));
        println!("test_connection: {:?}", t.as_ref().map(|b| b.clone().deserialize::<serde_json::Value>().unwrap()));
        let r = get_ipc_response(&wv, req("save_connection", serde_json::json!({ "config": config, "password": null })));
        println!("save_connection: {:?}", r.as_ref().map(|b| b.clone().deserialize::<serde_json::Value>().unwrap()));
        let l = get_ipc_response(&wv, req("list_connections", serde_json::json!({})));
        println!("list_connections: {:?}", l.as_ref().map(|b| b.clone().deserialize::<serde_json::Value>().unwrap()));
        assert!(r.is_ok() && l.is_ok());

        // DB 목록과 "id::db" 대상으로 스키마 조회
        let id = r.as_ref().unwrap().clone().deserialize::<serde_json::Value>().unwrap()["id"].as_str().unwrap().to_string();
        let dbs = get_ipc_response(&wv, req("list_databases", serde_json::json!({ "id": id })))
            .map(|b| b.deserialize::<Vec<String>>().unwrap())
            .expect("list_databases");
        println!("databases: {dbs:?}");
        let target = format!("{id}::{}", dbs.iter().find(|d| d.as_str() != "postgres").unwrap_or(&dbs[0]));
        let sch = get_ipc_response(&wv, req("list_schemas", serde_json::json!({ "id": target })))
            .map(|b| b.deserialize::<serde_json::Value>().unwrap())
            .expect("list_schemas on target db");
        println!("schemas in {target}: {}", sch.as_array().unwrap().len());
        let q = get_ipc_response(&wv, req("run_query", serde_json::json!({ "id": target, "sql": "select current_database()", "limit": 5, "tabId": null })))
            .map(|b| b.deserialize::<serde_json::Value>().unwrap())
            .expect("run_query");
        println!("current_database on target: {}", q["rows"][0][0]);
        assert_eq!(q["rows"][0][0].as_str().unwrap(), target.split("::").nth(1).unwrap());
        let _ = std::fs::remove_dir_all(dir);
    }
}
