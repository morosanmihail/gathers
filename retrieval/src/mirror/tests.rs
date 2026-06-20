use super::*;
use std::collections::HashMap;
use tempfile::TempDir;

/// Minimal raw-socket HTTP server for tests: serves a fixed byte response
/// per exact path, 404 otherwise. Returns the base URL.
async fn serve_routes(routes: HashMap<String, Vec<u8>>) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (mut socket, _) = match listener.accept().await {
                Ok(v) => v,
                Err(_) => break,
            };
            let routes = routes.clone();
            tokio::spawn(async move {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut buf = [0u8; 1024];
                let n = match socket.read(&mut buf).await {
                    Ok(n) => n,
                    Err(_) => return,
                };
                let req = String::from_utf8_lossy(&buf[..n]);
                let path = req
                    .lines()
                    .next()
                    .and_then(|l| l.split_whitespace().nth(1))
                    .unwrap_or("/")
                    .to_string();
                if let Some(body) = routes.get(&path) {
                    let header = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = socket.write_all(header.as_bytes()).await;
                    let _ = socket.write_all(body).await;
                } else {
                    let _ = socket
                        .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                        .await;
                }
            });
        }
    });
    format!("http://{addr}")
}

#[tokio::test]
async fn test_download_bz2_verified_skips_when_local_matches_remote() {
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("AllPrintings.sqlite");
    fs::write(&target, b"OLD DECOMPRESSED CONTENT").unwrap();
    let sidecar = dir.path().join("AllPrintings.sqlite.bz2.sha256");
    fs::write(&sidecar, "matching-crc").unwrap();

    // The bz2 route is intentionally left unregistered (404): if the skip
    // check didn't fire, the download would fail and the test would
    // catch it via the Err result below.
    let mut routes = HashMap::new();
    routes.insert(
        "/AllPrintings.sqlite.bz2.sha256".to_string(),
        b"matching-crc".to_vec(),
    );
    let base = serve_routes(routes).await;

    let result = download_bz2_verified(
        &format!("{base}/AllPrintings.sqlite.bz2"),
        &format!("{base}/AllPrintings.sqlite.bz2.sha256"),
        "AllPrintings.sqlite.bz2",
        &target,
        None,
    )
    .await;

    assert!(
        result.is_ok(),
        "skip path must succeed without hitting the bz2 endpoint"
    );
    assert_eq!(
        fs::read_to_string(&target).unwrap(),
        "OLD DECOMPRESSED CONTENT",
        "target must be untouched when local sidecar matches remote crc"
    );
}

#[tokio::test]
async fn test_download_bz2_verified_downloads_when_crc_differs() {
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("AllPrintings.sqlite");
    fs::write(&target, b"OLD DECOMPRESSED CONTENT").unwrap();
    let sidecar = dir.path().join("AllPrintings.sqlite.bz2.sha256");
    fs::write(&sidecar, "stale-crc").unwrap();

    let bz2_path = dir.path().join("new.bz2");
    let plain_path = dir.path().join("new.txt");
    fs::write(&plain_path, b"NEW DECOMPRESSED CONTENT").unwrap();
    compress_bz2(&plain_path, &bz2_path).unwrap();
    let bz2_bytes = fs::read(&bz2_path).unwrap();
    let real_crc = calculate_sha256(&bz2_path).unwrap();

    let mut routes = HashMap::new();
    routes.insert(
        "/AllPrintings.sqlite.bz2.sha256".to_string(),
        real_crc.clone().into_bytes(),
    );
    routes.insert("/AllPrintings.sqlite.bz2".to_string(), bz2_bytes);
    let base = serve_routes(routes).await;

    let result = download_bz2_verified(
        &format!("{base}/AllPrintings.sqlite.bz2"),
        &format!("{base}/AllPrintings.sqlite.bz2.sha256"),
        "AllPrintings.sqlite.bz2",
        &target,
        None,
    )
    .await;

    assert!(result.is_ok());
    assert_eq!(
        fs::read_to_string(&target).unwrap(),
        "NEW DECOMPRESSED CONTENT"
    );
    assert_eq!(fs::read_to_string(&sidecar).unwrap(), real_crc);
}

#[test]
fn test_state_dir_is_sibling_not_nested() {
    let data_dir = Path::new("/home/app/.local/share/gathers/mirror");
    let state = state_dir(data_dir);
    assert_eq!(
        state,
        Path::new("/home/app/.local/share/gathers/mirror-state")
    );
    assert!(
        !state.starts_with(data_dir),
        "state dir must not live under the publicly-served data dir"
    );
}

#[test]
fn test_last_update_for_missing_returns_none() {
    let dir = TempDir::new().unwrap();
    assert!(last_update_for(dir.path(), "AllPrintings.sqlite").is_none());
}

#[test]
fn test_last_update_for_round_trip() {
    let dir = TempDir::new().unwrap();
    let before = SystemTime::now();
    write_last_update_for(dir.path(), "AllPrintings.sqlite").unwrap();
    let after = SystemTime::now();

    let recorded = last_update_for(dir.path(), "AllPrintings.sqlite").unwrap();
    assert!(recorded >= before - Duration::from_secs(1));
    assert!(recorded <= after + Duration::from_secs(1));

    // Tracked independently per component.
    assert!(last_update_for(dir.path(), "riftbound.sqlite").is_none());
}

#[test]
fn test_is_fresh() {
    let dir = TempDir::new().unwrap();
    assert!(!is_fresh(
        dir.path(),
        "AllPrintings.sqlite",
        Duration::from_secs(3600)
    ));

    write_last_update_for(dir.path(), "AllPrintings.sqlite").unwrap();
    assert!(is_fresh(
        dir.path(),
        "AllPrintings.sqlite",
        Duration::from_secs(3600)
    ));
    assert!(!is_fresh(dir.path(), "AllPrintings.sqlite", Duration::ZERO));
}

#[tokio::test]
async fn test_refresh_if_stale_skips_when_fresh() {
    let dir = TempDir::new().unwrap();
    write_last_update_for(dir.path(), "AllPrintings.sqlite").unwrap();
    let before = last_update_for(dir.path(), "AllPrintings.sqlite").unwrap();

    let mut ran = false;
    refresh_if_stale(
        dir.path(),
        "AllPrintings.sqlite",
        Duration::from_secs(3600),
        async {
            ran = true;
            Ok(())
        },
    )
    .await;

    assert!(!ran, "fresh component must not be re-refreshed");
    assert_eq!(
        last_update_for(dir.path(), "AllPrintings.sqlite").unwrap(),
        before
    );
}

#[tokio::test]
async fn test_refresh_if_stale_records_only_on_success() {
    let dir = TempDir::new().unwrap();

    refresh_if_stale(
        dir.path(),
        "riftbound.sqlite",
        Duration::from_secs(3600),
        async { eyre::bail!("simulated failure") },
    )
    .await;
    assert!(
        last_update_for(dir.path(), "riftbound.sqlite").is_none(),
        "a failed refresh must not be recorded as up to date"
    );

    refresh_if_stale(
        dir.path(),
        "riftbound.sqlite",
        Duration::from_secs(3600),
        async { Ok(()) },
    )
    .await;
    assert!(last_update_for(dir.path(), "riftbound.sqlite").is_some());
}

#[test]
fn test_compress_decompress_round_trip() {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join("input.txt");
    fs::write(&src, b"hello mirror world").unwrap();

    let bz2 = dir.path().join("input.txt.bz2");
    compress_bz2(&src, &bz2).unwrap();
    assert!(bz2.exists());

    let restored = dir.path().join("restored.txt");
    decompress_bz2(&bz2, &restored).unwrap();
    assert_eq!(fs::read(&restored).unwrap(), b"hello mirror world");
}

#[test]
fn test_write_with_sha256_writes_sidecar() {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join("input.txt");
    fs::write(&src, b"some data").unwrap();
    let bz2 = dir.path().join("staged.bz2");
    compress_bz2(&src, &bz2).unwrap();

    write_with_sha256(&bz2, dir.path(), "thing.sqlite").unwrap();

    let target = dir.path().join("thing.sqlite.bz2");
    let sidecar = dir.path().join("thing.sqlite.bz2.sha256");
    assert!(target.exists());
    let crc = fs::read_to_string(&sidecar).unwrap();
    assert_eq!(crc, calculate_sha256(&target).unwrap());
}

#[test]
fn test_load_mirror_urls_missing_file_returns_empty() {
    // SAFETY: test runs in its own process slot; no other test in this
    // crate reads GATHERS_MIRRORS_PATH concurrently.
    unsafe {
        std::env::set_var("GATHERS_MIRRORS_PATH", "/nonexistent/path/mirrors.toml");
    }
    let urls = load_mirror_urls();
    assert!(urls.is_empty());
    unsafe {
        std::env::remove_var("GATHERS_MIRRORS_PATH");
    }
}

#[test]
fn test_load_mirror_urls_parses_ordered_list() {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("mirrors.toml");
    fs::write(
        &config_path,
        "mirrors = [\"https://mirror1.example.com\", \"https://mirror2.example.com\"]\n",
    )
    .unwrap();

    // SAFETY: see above.
    unsafe {
        std::env::set_var("GATHERS_MIRRORS_PATH", &config_path);
    }
    let urls = load_mirror_urls();
    unsafe {
        std::env::remove_var("GATHERS_MIRRORS_PATH");
    }

    assert_eq!(
        urls,
        vec![
            "https://mirror1.example.com".to_string(),
            "https://mirror2.example.com".to_string(),
        ]
    );
}

#[tokio::test]
async fn test_try_mirrors_no_config_returns_false() {
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("AllPrintings.sqlite");

    // SAFETY: see above.
    unsafe {
        std::env::set_var("GATHERS_MIRRORS_PATH", "/nonexistent/path/mirrors.toml");
    }
    let ok = try_mirrors("AllPrintings.sqlite", &target, None).await;
    unsafe {
        std::env::remove_var("GATHERS_MIRRORS_PATH");
    }

    assert!(!ok);
    assert!(!target.exists());
}

#[tokio::test]
async fn test_try_mirrors_unreachable_host_falls_through() {
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("AllPrintings.sqlite");
    let config_path = dir.path().join("mirrors.toml");
    fs::write(&config_path, "mirrors = [\"http://127.0.0.1:1\"]\n").unwrap();

    // SAFETY: see above.
    unsafe {
        std::env::set_var("GATHERS_MIRRORS_PATH", &config_path);
    }
    let ok = try_mirrors("AllPrintings.sqlite", &target, None).await;
    unsafe {
        std::env::remove_var("GATHERS_MIRRORS_PATH");
    }

    assert!(!ok, "unreachable mirror must not report success");
    assert!(!target.exists());
}
