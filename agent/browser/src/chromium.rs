//! Запуск Chromium с отладочным портом. Своё: у muse браузер облачный или
//! на устройстве владельца, запуск в разборе не виден. Путь к браузеру —
//! `HAMLET_CHROMIUM`, иначе известные места установки.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

use crate::cdp::Cdp;

/// Где искать браузер, если `HAMLET_CHROMIUM` не задан.
pub fn find() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("HAMLET_CHROMIUM") {
        return Some(PathBuf::from(p)).filter(|p| p.exists());
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let mut candidates: Vec<PathBuf> = vec![
        "/Applications/Chromium.app/Contents/MacOS/Chromium".into(),
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".into(),
        "/usr/bin/chromium".into(),
        "/usr/bin/chromium-browser".into(),
        "/usr/bin/google-chrome".into(),
    ];
    // Сборки Playwright, если стоят.
    if let Ok(rd) = std::fs::read_dir(Path::new(&home).join("Library/Caches/ms-playwright")) {
        for e in rd.flatten() {
            let p = e.path();
            candidates.push(p.join("chrome-mac/Chromium.app/Contents/MacOS/Chromium"));
            candidates.push(p.join("chrome-mac-arm64/Chromium.app/Contents/MacOS/Chromium"));
        }
    }
    candidates.into_iter().find(|p| p.exists())
}

pub struct Chromium {
    child: Child,
    pub cdp: Arc<Cdp>,
}

impl Chromium {
    /// Запустить браузер с профилем `profile_dir`. `headless` — без окна.
    pub async fn launch(bin: &Path, profile_dir: &Path, headless: bool) -> Result<Chromium, String> {
        tokio::fs::create_dir_all(profile_dir).await.map_err(|e| format!("browser profile: {e}"))?;
        let mut cmd = Command::new(bin);
        cmd.arg("--remote-debugging-port=0")
            .arg(format!("--user-data-dir={}", profile_dir.display()))
            .args(["--no-first-run", "--no-default-browser-check", "--disable-background-networking", "--disable-sync", "--window-size=1280,900", "--password-store=basic", "--use-mock-keychain"]);
        if headless {
            cmd.arg("--headless=new");
        }
        cmd.arg("about:blank").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).kill_on_drop(true);
        let mut child = cmd.spawn().map_err(|e| format!("start browser {}: {e}", bin.display()))?;
        let stderr = child.stderr.take().ok_or("browser stderr")?;
        let mut lines = BufReader::new(stderr).lines();
        let url = tokio::time::timeout(Duration::from_secs(30), async {
            while let Ok(Some(l)) = lines.next_line().await {
                if let Some(i) = l.find("ws://") {
                    return Some(l[i..].trim().to_string());
                }
            }
            None
        })
        .await
        .ok()
        .flatten()
        .ok_or("browser did not report its DevTools address")?;
        // Остаток вывода браузера читаем, чтобы труба не заполнилась.
        tokio::spawn(async move { while let Ok(Some(_)) = lines.next_line().await {} });
        let cdp = Cdp::connect(&url).await?;
        Ok(Chromium { child, cdp })
    }

    pub async fn close(mut self) {
        let _ = self.cdp.call(None, "Browser.close", serde_json::json!({})).await;
        let _ = tokio::time::timeout(Duration::from_secs(5), self.child.wait()).await;
        let _ = self.child.kill().await;
    }
}
