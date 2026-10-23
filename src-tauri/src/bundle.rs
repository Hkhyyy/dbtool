//! macOS 앱 안 업데이트: Qurry.app 폴더는 그대로 두고 안의 Contents 만 바꾼다.
//! 폴더째 바꾸면(Tauri updater 기본) 같은 경로라도 다른 파일이 되어, Dock 에 고정한 아이콘이 옛 앱을 가리킨 채
//! 새로 켠 앱이 Dock 에 하나 더 생긴다.
use std::io::{Error, ErrorKind, Result};
use std::path::{Component, Path, PathBuf};

/// 실행 파일(…/Qurry.app/Contents/MacOS/qurry) 이 든 앱 폴더. 앱 묶음 밖에서 돌면(cargo 빌드) None
pub fn app_bundle(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let app = contents.parent()?;
    (macos.file_name()? == "MacOS" && contents.file_name()? == "Contents" && app.extension()? == "app").then(|| app.to_path_buf())
}

/// 업데이트 묶음(tar.gz, 맨 위가 Qurry.app/) 의 Contents 를 app 의 Contents 와 바꾼다.
/// 서명 확인은 부르는 쪽(updater 의 download)이 이미 했다. 실패하면 지금 앱은 그대로다
pub fn install_contents(app: &Path, package: &[u8]) -> Result<()> {
    // 같은 파일 시스템 안에서 바꿔야 한 번에 바뀐다: 작업 폴더를 앱 안에 둔다
    let work = app.join(format!(".update-{}", std::process::id()));
    if work.exists() {
        std::fs::remove_dir_all(&work)?;
    }
    std::fs::create_dir(&work)?;
    let result = (|| {
        let new = unpack(package, &work)?;
        let current = app.join("Contents");
        if let Err(e) = swap(&new, &current) {
            if e.kind() == ErrorKind::PermissionDenied {
                return Err(e);
            }
            // 바꾸기(RENAME_SWAP)를 못 하는 파일 시스템: 옮겨 두고 넣고, 실패하면 되돌린다
            let old = work.join("old-Contents");
            std::fs::rename(&current, &old)?;
            if let Err(e) = std::fs::rename(&new, &current) {
                let _ = std::fs::rename(&old, &current);
                return Err(e);
            }
        }
        Ok(())
    })();
    // 바꾼 뒤에는 옛 Contents 가 작업 폴더에 있다
    let _ = std::fs::remove_dir_all(&work);
    if result.is_ok() {
        // Finder·LaunchServices 가 바뀐 앱을 다시 읽게
        let _ = std::fs::File::open(app).and_then(|f| f.set_modified(std::time::SystemTime::now()));
    }
    result
}

/// 묶음을 work 아래에 풀고 새 Contents 경로를 돌려준다. 모든 항목은 <앱>.app/Contents/… 여야 한다
fn unpack(package: &[u8], work: &Path) -> Result<PathBuf> {
    let bad = |p: &Path| Error::new(ErrorKind::InvalidData, format!("unexpected path in the update: {}", p.display()));
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(package));
    let mut top: Option<std::ffi::OsString> = None;
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        let comps: Vec<Component> = path.components().collect();
        let ok = comps.iter().all(|c| matches!(c, Component::Normal(_)))
            && match comps.as_slice() {
                [Component::Normal(t)] => top.get_or_insert_with(|| t.to_os_string()) == t,
                [Component::Normal(t), Component::Normal(c), ..] => top.get_or_insert_with(|| t.to_os_string()) == t && *c == "Contents",
                _ => false,
            };
        // unpack_in 도 work 밖으로 나가는 항목(링크를 거치는 것 포함)은 풀지 않는다
        if !ok || !entry.unpack_in(work)? {
            return Err(bad(&path));
        }
    }
    let new = work.join(top.ok_or_else(|| bad(Path::new("")))?).join("Contents");
    if !new.is_dir() {
        return Err(bad(&new));
    }
    Ok(new)
}

/// 두 경로를 한 번에 맞바꾼다 (APFS 의 renamex_np RENAME_SWAP)
fn swap(a: &Path, b: &Path) -> Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let a = std::ffi::CString::new(a.as_os_str().as_bytes())?;
    let b = std::ffi::CString::new(b.as_os_str().as_bytes())?;
    // SAFETY: 둘 다 호출 동안 살아 있는 NUL 로 끝나는 문자열이고, renamex_np 는 읽기만 한다
    if unsafe { libc::renamex_np(a.as_ptr(), b.as_ptr(), libc::RENAME_SWAP) } == 0 { Ok(()) } else { Err(Error::last_os_error()) }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::os::unix::fs::MetadataExt;

    /// 업데이트 묶음과 같은 모양 (맨 위가 Qurry.app/)
    fn tar_gz(files: &[(&str, &str)]) -> Vec<u8> {
        let mut b = tar::Builder::new(flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast()));
        for (path, body) in files {
            let mut h = tar::Header::new_gnu();
            h.set_size(body.len() as u64);
            h.set_mode(0o755);
            h.set_entry_type(tar::EntryType::Regular);
            // set_path 는 ".." 를 거부하므로 이름 칸에 직접 쓴다 (나쁜 묶음 흉내)
            h.as_old_mut().name[..path.len()].copy_from_slice(path.as_bytes());
            h.set_cksum();
            b.append(&h, body.as_bytes()).unwrap();
        }
        let mut gz = b.into_inner().unwrap();
        gz.flush().unwrap();
        gz.finish().unwrap()
    }

    fn app_with(dir: &std::path::Path, version: &str) -> std::path::PathBuf {
        let app = dir.join("Qurry.app");
        std::fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
        std::fs::write(app.join("Contents/Info.plist"), version).unwrap();
        std::fs::write(app.join("Contents/MacOS/qurry"), version).unwrap();
        app
    }

    /// Dock 고정 아이콘이 계속 같은 앱을 가리키게: Qurry.app 폴더는 같은 파일(inode) 그대로, 내용만 새 버전
    #[test]
    fn replaces_contents_but_keeps_the_app_folder() {
        let dir = tempfile::tempdir().unwrap();
        let app = app_with(dir.path(), "old");
        let inode = std::fs::metadata(&app).unwrap().ino();
        let pkg = tar_gz(&[("Qurry.app/Contents/Info.plist", "new"), ("Qurry.app/Contents/MacOS/qurry", "new-bin")]);

        super::install_contents(&app, &pkg).unwrap();

        assert_eq!(std::fs::metadata(&app).unwrap().ino(), inode);
        assert_eq!(std::fs::read_to_string(app.join("Contents/Info.plist")).unwrap(), "new");
        assert_eq!(std::fs::read_to_string(app.join("Contents/MacOS/qurry")).unwrap(), "new-bin");
        // 옛 내용이나 작업 폴더가 남지 않는다
        let left: Vec<_> = std::fs::read_dir(&app).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(left, ["Contents"]);
    }

    /// 묶음 밖으로 나가는 경로나 Contents 가 없는 묶음은 거부하고, 지금 앱은 그대로 둔다
    #[test]
    fn rejects_bad_packages_and_keeps_the_app() {
        let dir = tempfile::tempdir().unwrap();
        let app = app_with(dir.path(), "old");
        for pkg in [
            tar_gz(&[("Qurry.app/Contents/Info.plist", "new"), ("Qurry.app/../../evil", "x")]),
            tar_gz(&[("Qurry.app/Other/file", "x")]),
        ] {
            assert!(super::install_contents(&app, &pkg).is_err());
            assert_eq!(std::fs::read_to_string(app.join("Contents/Info.plist")).unwrap(), "old");
            assert!(!dir.path().join("evil").exists());
            let left: Vec<_> = std::fs::read_dir(&app).unwrap().map(|e| e.unwrap().file_name()).collect();
            assert_eq!(left, ["Contents"]);
        }
    }

    #[test]
    fn app_folder_of_the_executable() {
        assert_eq!(super::app_bundle(std::path::Path::new("/Applications/Qurry.app/Contents/MacOS/qurry")).unwrap(), std::path::Path::new("/Applications/Qurry.app"));
        assert!(super::app_bundle(std::path::Path::new("/Users/me/dbtool/target/release/qurry")).is_none());
    }
}
