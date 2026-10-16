//! `dbtool-tui --update`: 최신 릴리스의 CLI 묶음(dbtool-tui + dbtool-mcp)을 받아 지금 실행 파일 자리에 바꿔 넣는다.
//! 소스에서 빌드한 실행 파일(…/target/release 등)은 건드리지 않고 다시 빌드하라고 안내한다
use dbtool_core::update;
use std::path::{Path, PathBuf};

/// 이 실행 파일이 받을 릴리스 파일 이름의 끝 (지금은 Apple Silicon macOS 만 빌드한다)
const ASSET_SUFFIX: Option<&str> = if cfg!(all(target_os = "macos", target_arch = "aarch64")) { Some("aarch64-apple-darwin.tar.gz") } else { None };

/// cargo 로 빌드한 실행 파일인지 (…/target/release/dbtool-tui, …/target/debug/…)
pub fn from_source(exe: &Path) -> bool {
    let parts: Vec<String> = exe.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    parts.windows(2).any(|w| w[0] == "target" && (w[1] == "release" || w[1] == "debug"))
}

pub fn current_exe() -> Option<PathBuf> {
    std::env::current_exe().ok().and_then(|p| p.canonicalize().ok())
}

fn err(e: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::other(e.to_string())
}

fn run(cmd: &mut std::process::Command) -> std::io::Result<()> {
    let st = cmd.status()?;
    if st.success() { Ok(()) } else { Err(err(format!("{cmd:?} failed ({st})"))) }
}

pub async fn run_update() -> std::io::Result<()> {
    let cur = env!("CARGO_PKG_VERSION");
    let exe = current_exe().ok_or_else(|| err("cannot find this executable"))?;
    if from_source(&exe) {
        println!("This dbtool-tui was built from source:\n  {}\nUpdate it with:\n  git pull && cargo build --release -p dbtool-tui -p dbtool-mcp", exe.display());
        return Ok(());
    }
    let rel = update::latest_release().await.map_err(err)?;
    if !update::is_newer(cur, &rel.version) {
        println!("dbtool-tui {cur} is up to date.");
        return Ok(());
    }
    let Some(asset) = ASSET_SUFFIX.and_then(|s| rel.asset("dbtool-cli_", s)) else {
        println!("dbtool {} is available, but there is no prebuilt binary for this platform.\nSee {}", rel.version, rel.url);
        return Ok(());
    };
    println!("Updating dbtool-tui {cur} → {} …", rel.version);
    let tmp = std::env::temp_dir().join(format!("dbtool-update-{}", std::process::id()));
    std::fs::create_dir_all(&tmp)?;
    let tgz = tmp.join("cli.tar.gz");
    let result = (|| -> std::io::Result<Vec<PathBuf>> {
        run(std::process::Command::new("curl").args(["-fL", "--progress-bar", "-o"]).arg(&tgz).arg(&asset.url))?;
        run(std::process::Command::new("tar").arg("-xzf").arg(&tgz).arg("-C").arg(&tmp))?;
        // 지금 실행 파일과 같은 폴더에 바꿔 넣는다 (임시 이름으로 복사 → rename 이라 도중에 깨지지 않는다)
        let dir = exe.parent().ok_or_else(|| err("no install folder"))?;
        let mut done = vec![];
        for name in ["dbtool-tui", "dbtool-mcp"] {
            let src = tmp.join(name);
            if !src.exists() {
                continue;
            }
            let staged = dir.join(format!(".{name}.new"));
            std::fs::copy(&src, &staged)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755))?;
            }
            let dst = dir.join(name);
            std::fs::rename(&staged, &dst)?;
            done.push(dst);
        }
        Ok(done)
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    let done = result?;
    for p in &done {
        println!("  updated {}", p.display());
    }
    println!("dbtool {} installed. Restart dbtool-tui (and your MCP client, if it runs dbtool-mcp).", rel.version);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::from_source;
    use std::path::Path;

    #[test]
    fn detects_cargo_builds() {
        assert!(from_source(Path::new("/Users/me/dbtool/target/release/dbtool-tui")));
        assert!(from_source(Path::new("/x/target/debug/dbtool-tui")));
        assert!(!from_source(Path::new("/Users/me/.local/bin/dbtool-tui")));
        assert!(!from_source(Path::new("/opt/target-tools/release/dbtool-tui")));
    }
}
