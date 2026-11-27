//! Временный Postgres для тестов: свой кластер на прогон (initdb во
//! временной папке), только TCP на 127.0.0.1, без unix-сокета; у каждого
//! теста своя база. Кластер гасит сторож — отдельный процесс в своей
//! группе: ждёт, пока процесс теста исчезнет (обычный выход, паника, сигнал,
//! SIGKILL), останавливает кластер и удаляет папку. Перед запуском убираются
//! осиротевшие кластеры прошлых прогонов (процесс-владелец мёртв).
//!
//! Бинарники — `AGENT_PG_BIN`, иначе Homebrew `postgresql@18`, иначе
//! `pg_config --bindir`. Нужен pgvector (см. README).

use std::net::TcpListener;
use std::path::PathBuf;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
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

/// Сторож: `kill -0` раз в секунду; когда процесса теста нет —
/// `pg_ctl stop` и удаление папки. Своя группа процессов — Ctrl-C по
/// группе теста его не задевает.
fn watch(bin: &std::path::Path, dir: &std::path::Path) {
    Command::new("/bin/sh")
        .arg("-c")
        .arg(r#"while kill -0 "$1" 2>/dev/null; do sleep 1; done; "$2" -D "$3/data" -m immediate -w stop; rm -rf "$3""#)
        .arg("pg-watch")
        .arg(std::process::id().to_string())
        .arg(bin.join("pg_ctl"))
        .arg(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .expect("сторож кластера");
}

/// Кластеры прошлых прогонов, чей процесс-владелец (число в имени папки)
/// уже мёртв: остановить и удалить.
fn sweep_orphans(bin: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else { return };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let Some(pid) = name.strip_prefix("agent-memory-pg-").and_then(|r| r.split('-').next()).and_then(|p| p.parse::<i32>().ok()) else {
            continue;
        };
        if unsafe { libc::kill(pid, 0) } == 0 || std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH) {
            continue;
        }
        let _ = Command::new(bin.join("pg_ctl")).arg("-D").arg(e.path().join("data")).args(["-m", "immediate", "-w", "stop"]).output();
        let _ = std::fs::remove_dir_all(e.path());
    }
}

fn cluster() -> &'static Cluster {
    CLUSTER.get_or_init(|| {
        let bin = bin_dir();
        sweep_orphans(&bin);
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
            .arg(format!("-c listen_addresses=127.0.0.1 -p {port} -c unix_socket_directories='' -c fsync=off -c max_connections=400"))
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
        watch(&c.bin, &c.dir);
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
