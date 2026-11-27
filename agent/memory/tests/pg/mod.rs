//! Временный Postgres для тестов: свой кластер на прогон (initdb во
//! временной папке), только TCP на 127.0.0.1, без unix-сокета; у каждого
//! теста своя база. При выходе процесса кластер останавливается, папка
//! удаляется.
//!
//! Бинарники — `AGENT_PG_BIN`, иначе Homebrew `postgresql@18`, иначе
//! `pg_config --bindir`. Нужен pgvector (см. README).

use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

struct Cluster {
    bin: PathBuf,
    dir: PathBuf,
    port: u16,
}

static CLUSTER: OnceLock<Cluster> = OnceLock::new();
static NEXT_DB: AtomicUsize = AtomicUsize::new(0);

fn bin_dir() -> PathBuf {
    if let Ok(b) = std::env::var("AGENT_PG_BIN") {
        return b.into();
    }
    let brew = PathBuf::from("/opt/homebrew/opt/postgresql@18/bin");
    if brew.join("initdb").exists() {
        return brew;
    }
    let out = Command::new("pg_config")
        .arg("--bindir")
        .output()
        .expect("Postgres не найден: поставьте postgresql@18 и pgvector (README) или задайте AGENT_PG_BIN");
    PathBuf::from(String::from_utf8_lossy(&out.stdout).trim())
}

extern "C" fn stop_cluster() {
    if let Some(c) = CLUSTER.get() {
        let _ = Command::new(c.bin.join("pg_ctl"))
            .args(["-D"])
            .arg(c.dir.join("data"))
            .args(["-m", "immediate", "-w", "stop"])
            .output();
        let _ = std::fs::remove_dir_all(&c.dir);
    }
}

fn cluster() -> &'static Cluster {
    CLUSTER.get_or_init(|| {
        let bin = bin_dir();
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("agent-memory-pg-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let init = Command::new(bin.join("initdb"))
            .arg("-D")
            .arg(dir.join("data"))
            .args(["-U", "agent", "-A", "trust", "--no-sync", "-E", "UTF8", "--locale=C"])
            .output()
            .expect("initdb");
        assert!(init.status.success(), "initdb: {}", String::from_utf8_lossy(&init.stderr));
        let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let start = Command::new(bin.join("pg_ctl"))
            .arg("-D")
            .arg(dir.join("data"))
            .arg("-o")
            .arg(format!("-c listen_addresses=127.0.0.1 -p {port} -c unix_socket_directories='' -c fsync=off"))
            .arg("-l")
            .arg(dir.join("log"))
            .args(["-w", "start"])
            .output()
            .expect("pg_ctl start");
        let c = Cluster { bin, dir, port };
        if !start.status.success() {
            let log = std::fs::read_to_string(c.dir.join("log")).unwrap_or_default();
            let _ = std::fs::remove_dir_all(&c.dir);
            panic!("pg_ctl start: {}\n{log}", String::from_utf8_lossy(&start.stderr));
        }
        unsafe {
            libc::atexit(stop_cluster);
        }
        c
    })
}

/// Новая пустая база в кластере прогона; адрес подключения.
pub fn database() -> String {
    let c = cluster();
    let name = format!("t{}", NEXT_DB.fetch_add(1, Ordering::SeqCst));
    let out = Command::new(c.bin.join("createdb"))
        .args(["-h", "127.0.0.1", "-p", &c.port.to_string(), "-U", "agent", &name])
        .output()
        .expect("createdb");
    assert!(out.status.success(), "createdb: {}", String::from_utf8_lossy(&out.stderr));
    format!("postgres://agent@127.0.0.1:{}/{name}", c.port)
}
