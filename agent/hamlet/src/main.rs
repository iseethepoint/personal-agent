//! `hamlet --agent <папка>` — запуск агента. Вход для проверки — строки из
//! stdin, ответ — в stdout (настоящий вход и мессенджер — П6).

use std::path::PathBuf;
use std::sync::Arc;

use agent_hamlet::daemon::Services;
use agent_hamlet::{AgentDir, Hamlet, Options};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;

fn usage() -> ! {
    eprintln!("использование: hamlet --agent <папка агента>");
    std::process::exit(2);
}

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let dir = match (args.next().as_deref(), args.next()) {
        (Some("--agent"), Some(d)) => PathBuf::from(d),
        _ => usage(),
    };
    let dir = match AgentDir::load(&dir) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let opts = Options::default();
    let h = match Hamlet::build(dir, &opts).await {
        Ok(h) => Arc::new(h),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let services = Services::start(&h, opts.tick);
    let (tx, rx) = mpsc::channel(16);
    let (out_tx, mut out_rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        let mut lines = BufReader::new(tokio::io::stdin()).lines();
        while let Ok(Some(l)) = lines.next_line().await {
            if !l.trim().is_empty() && tx.send(l).await.is_err() {
                break;
            }
        }
    });
    let printer = tokio::spawn(async move {
        while let Some(t) = out_rx.recv().await {
            println!("{t}");
        }
    });
    h.run(rx, out_tx).await;
    services.stop(&h).await;
    let _ = printer.await;
}
