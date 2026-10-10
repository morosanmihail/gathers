//! Self-contained test environments: every e2e test deploys its own
//! server — and, when it needs them, a mirror and the dummy plugin — in a
//! fresh temp directory on free ports, so tests never touch a real install,
//! never depend on one running, and can't collide with each other or with
//! a server already running on the default ports (e.g. via `tilt up`).
//!
//! The binaries are built on first use, unless `GATHERS_BIN_DIR` points at
//! prebuilt ones (as `just e2e` does, to build once for every test).
//! Everything is killed when the `Harness` is dropped. On failure,
//! `Harness::conclude` prints the end of every process's log and keeps the
//! temp directory for inspection.

use std::{
    future::Future,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use crate::GathersClient;

/// Environment variables a server reads that, inherited from the shell
/// running the tests, could point it at real data or change its behaviour.
const INHERITED_SERVER_VARS: &[&str] = &[
    "GATHERS_SYSTEMS",
    "GATHERS_PRICE_HISTORY",
    "GATHERS_CORS_ORIGINS",
    "GATHERS_MIRRORS_PATH",
    "GATHERS_NO_AUTO_UPDATE",
    "DEMO_MODE",
    "MTG_DB_PATH",
    "MTG_PRICES_PATH",
    "RIFTBOUND_DB_PATH",
    "RIFTBOUND_PRICES_PATH",
    "POKEMON_DB_PATH",
    "POKEMON_PRICES_PATH",
    "STORAGE_DB_PATH",
    "PRICE_HISTORY_DB_PATH",
];

/// Components the mirror would otherwise refresh from upstream.
const MIRROR_STEMS: &[&str] = &[
    "AllPrintings.sqlite",
    "AllPricesToday.sqlite",
    "pokemon_prices_tcgcsv.sqlite",
    "riftbound_prices_tcgcsv.sqlite",
    "riftbound.sqlite",
    "pokemon.sqlite",
];

pub struct Harness {
    dir: Option<tempfile::TempDir>,
    root: PathBuf,
    workspace: PathBuf,
    bin_dir: PathBuf,
    children: Vec<(String, Child)>,
}

impl Harness {
    /// Creates the temp directory and makes sure the binaries exist.
    pub fn new(title: &str) -> eyre::Result<Self> {
        println!("=== GatheRs {title} e2e ===");
        let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").canonicalize()?;
        let bin_dir = match std::env::var("GATHERS_BIN_DIR") {
            Ok(dir) => workspace.join(dir),
            Err(_) => {
                let status = Command::new(env!("CARGO"))
                    .args(["build", "-q", "-p", "server", "-p", "mirror", "-p", "dummy-plugin"])
                    .current_dir(&workspace)
                    .status()?;
                eyre::ensure!(status.success(), "building server, mirror and dummy-plugin failed");
                workspace.join("target/debug")
            }
        };
        let dir = tempfile::Builder::new().prefix("gathers-e2e-").tempdir()?;
        let root = dir.path().to_path_buf();
        for sub in ["home", "db"] {
            std::fs::create_dir_all(root.join(sub))?;
        }
        Ok(Self { dir: Some(dir), root, workspace, bin_dir, children: vec![] })
    }

    /// The temp directory everything lives in.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// A file from the repo's `data/` test fixtures.
    pub fn data_file(&self, file: &str) -> PathBuf {
        self.workspace.join("data").join(file)
    }

    /// Copies a `data/` fixture into the temp directory (opening a card
    /// database adds indexes to it, so tests never use the originals).
    pub fn copy_data_file(&self, file: &str, to: &str) -> eyre::Result<PathBuf> {
        let dest = self.root.join(to);
        std::fs::copy(self.data_file(file), &dest)?;
        Ok(dest)
    }

    /// Starts `name`, logging to `{root}/{name}.log`.
    pub fn spawn(&mut self, name: &str, mut cmd: Command) -> eyre::Result<()> {
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.root.join(format!("{name}.log")))?;
        let child = cmd
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log))
            .spawn()
            .map_err(|e| eyre::eyre!("failed to start {name}: {e}"))?;
        self.children.push((name.to_string(), child));
        Ok(())
    }

    /// Kills `name` if it's running.
    pub fn stop(&mut self, name: &str) {
        self.children.retain_mut(|(n, child)| {
            if n != name {
                return true;
            }
            let _ = child.kill();
            let _ = child.wait();
            false
        });
    }

    /// Fails if `name` has exited, e.g. a server that refused its config.
    fn ensure_running(&mut self, name: &str) -> eyre::Result<()> {
        if let Some((_, child)) = self.children.iter_mut().find(|(n, _)| n == name)
            && let Some(status) = child.try_wait()?
        {
            eyre::bail!("{name} exited early ({status})");
        }
        Ok(())
    }

    /// Starts a server as `setup` describes and waits until it's serving
    /// with every expected system loaded and no download in progress.
    pub async fn start_server(&mut self, setup: &ServerSetup) -> eyre::Result<GathersClient> {
        let mut cmd = Command::new(self.bin_dir.join("server"));
        cmd.args(["--port", &setup.port.to_string()]);
        for var in INHERITED_SERVER_VARS {
            cmd.env_remove(var);
        }
        cmd.env("HOME", self.root.join("home"))
            .env("STORAGE_DB_PATH", self.root.join("db/storage.db"));
        if !setup.auto_update {
            cmd.env("GATHERS_NO_AUTO_UPDATE", "1");
        }
        if let Some(config) = &setup.config {
            let dir = self.root.join("home/.local/share/gathers");
            std::fs::create_dir_all(&dir)?;
            std::fs::write(dir.join("server.toml"), config)?;
        }
        for (key, value) in &setup.env {
            cmd.env(key, value);
        }
        self.spawn("server", cmd)?;

        let client = GathersClient::new(format!("http://127.0.0.1:{}", setup.port));
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            self.ensure_running("server")?;
            if let Ok(info) = client.system_info().await
                && info.downloading.is_empty()
                && setup.expect_systems.iter().all(|s| info.systems.contains(s))
            {
                return Ok(client);
            }
            eyre::ensure!(Instant::now() < deadline, "timed out waiting for the server to start");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Starts the dummy plugin on a free port, returning its base URL.
    pub async fn start_dummy_plugin(&mut self) -> eyre::Result<String> {
        let port = free_port()?;
        let mut cmd = Command::new(self.bin_dir.join("dummy-plugin"));
        cmd.env("DUMMY_PLUGIN_PORT", port.to_string());
        self.spawn("dummy-plugin", cmd)?;
        let base = format!("http://127.0.0.1:{port}");
        let info = format!("{base}/gathers-plugin/v1/info");
        wait_until("the dummy plugin to start", || async {
            Ok(reqwest::get(&info).await.is_ok_and(|r| r.status().is_success()))
        })
        .await?;
        Ok(base)
    }

    /// Where `publish_to_mirror` puts files and the mirror serves them from.
    pub fn mirror_dir(&self) -> PathBuf {
        self.root.join("mirror")
    }

    /// Compresses `src` into the mirror as `{stem}.bz2` plus its `.sha256`,
    /// exactly as the real mirror publishes.
    pub fn publish_to_mirror(&self, src: &Path, stem: &str) -> eyre::Result<()> {
        std::fs::create_dir_all(self.mirror_dir())?;
        let staging = tempfile::tempdir_in(&self.root)?;
        let bz2 = staging.path().join(format!("{stem}.bz2"));
        retrieval::mirror::compress_bz2(src, &bz2)?;
        retrieval::mirror::write_with_sha256(&bz2, &self.mirror_dir(), stem)
    }

    /// Starts the real mirror serving `mirror_dir()`, with its upstream
    /// refresh suppressed, and returns a `mirrors.toml` pointing at it for
    /// a server's `GATHERS_MIRRORS_PATH`.
    pub async fn start_mirror(&mut self) -> eyre::Result<PathBuf> {
        let dir = self.mirror_dir();
        std::fs::create_dir_all(&dir)?;
        // Fresh markers make the mirror skip refreshing anything upstream.
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
        for stem in MIRROR_STEMS {
            std::fs::write(dir.join(format!("{stem}.last_update")), now.to_string())?;
        }
        let port = free_port()?;
        let mut cmd = Command::new(self.bin_dir.join("mirror"));
        cmd.env("MIRROR_DATA_DIR", &dir)
            .env("MIRROR_PORT", port.to_string())
            .env("MIRROR_INTERVAL_HOURS", "8760")
            .env("HOME", self.root.join("home"));
        self.spawn("mirror", cmd)?;

        let base = format!("http://127.0.0.1:{port}");
        // Any marker file is enough to know it's serving.
        let probe = format!("{base}/{}.last_update", MIRROR_STEMS[0]);
        wait_until("the mirror to start", || async {
            Ok(reqwest::get(&probe).await.is_ok_and(|r| r.status().is_success()))
        })
        .await?;

        let mirrors_toml = self.root.join("mirrors.toml");
        std::fs::write(&mirrors_toml, format!("mirrors = [\"{base}\"]\n"))?;
        Ok(mirrors_toml)
    }

    /// Passes `result` through; on failure, prints each process's log tail
    /// and keeps the temp directory.
    pub fn conclude(mut self, result: eyre::Result<()>) -> eyre::Result<()> {
        match &result {
            Ok(()) => {}
            Err(_) => {
                for (name, _) in &self.children {
                    let content = std::fs::read_to_string(self.root.join(format!("{name}.log"))).unwrap_or_default();
                    let lines: Vec<&str> = content.lines().collect();
                    println!("\n--- last lines of {name}.log ---");
                    for line in &lines[lines.len().saturating_sub(40)..] {
                        println!("{line}");
                    }
                }
                if let Some(dir) = self.dir.take() {
                    println!("\nTest files kept in {}", dir.keep().display());
                }
            }
        }
        result
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        for (_, child) in &mut self.children {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// How to run a server under test.
pub struct ServerSetup {
    pub port: u16,
    /// Extra environment, applied last.
    pub env: Vec<(String, String)>,
    /// A `server.toml` to start from; otherwise the server writes its own.
    pub config: Option<String>,
    /// Systems the server must have loaded before it counts as started.
    pub expect_systems: Vec<String>,
    /// Let the server fetch missing card databases on startup.
    pub auto_update: bool,
}

impl ServerSetup {
    pub fn new() -> eyre::Result<Self> {
        Ok(Self { port: free_port()?, env: vec![], config: None, expect_systems: vec![], auto_update: false })
    }

    /// MTG (`sql`) on a copy of `data/testPrintings.db`.
    pub fn mtg(harness: &Harness) -> eyre::Result<Self> {
        let db = harness.copy_data_file("testPrintings.db", "db/AllPrintings.db")?;
        Ok(Self::new()?
            .env("GATHERS_SYSTEMS", "sql")
            .env("MTG_DB_PATH", db.to_string_lossy())
            .env("MTG_PRICES_PATH", harness.root().join("db/AllPricesToday.db").to_string_lossy())
            .expect_system("MagicSQLite"))
    }

    pub fn env(mut self, key: &str, value: impl AsRef<str>) -> Self {
        self.env.push((key.to_string(), value.as_ref().to_string()));
        self
    }

    pub fn expect_system(mut self, system: &str) -> Self {
        self.expect_systems.push(system.to_string());
        self
    }

    pub fn config(mut self, toml: impl Into<String>) -> Self {
        self.config = Some(toml.into());
        self
    }

    pub fn auto_update(mut self) -> Self {
        self.auto_update = true;
        self
    }
}

pub fn free_port() -> eyre::Result<u16> {
    Ok(std::net::TcpListener::bind("127.0.0.1:0")?.local_addr()?.port())
}

/// Polls `check` until it returns true, for up to a minute.
pub async fn wait_until<F, Fut>(what: &str, mut check: F) -> eyre::Result<()>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = eyre::Result<bool>>,
{
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if check().await.unwrap_or(false) {
            return Ok(());
        }
        eyre::ensure!(Instant::now() < deadline, "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}
