//! П5-2: метка времени входа — форма ядра (`agent/hamlet/src/daemon.rs`,
//! `time_tag`): `[Day YYYY-MM-DD HH:MM:SS TZ] [client_timezone=…]`.

use agent_hamlet::daemon::time_tag;

/// 2027-01-15 08:00:00 UTC — пятница.
const T0: i64 = 1_800_000_000_000;

#[test]
fn time_tag_follows_core_form() {
    assert_eq!(time_tag(T0, Some("UTC")).unwrap(), "[Fri 2027-01-15 08:00:00 UTC] [client_timezone=UTC]");
    assert_eq!(time_tag(T0, Some("Europe/Berlin")).unwrap(), "[Fri 2027-01-15 09:00:00 CET] [client_timezone=Europe/Berlin]");
    assert_eq!(time_tag(T0 + 3_600_000 * 16, Some("Asia/Tbilisi")).unwrap(), "[Sat 2027-01-16 04:00:00 +04] [client_timezone=Asia/Tbilisi]");
    // Без пояса метки нет — как у muse (`0xbc4c4cd`).
    assert_eq!(time_tag(T0, None), None);
}

/// Неразборчивый пояс — ошибка загрузки `agent.toml`: агент не стартует.
#[test]
fn bad_timezone_fails_load() {
    let dir = std::env::temp_dir().join(format!("hamlet-tz-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("agent.toml"), "[agent]\ntimezone = \"Nowhere/City\"\n\n[database]\nurl_env = \"X\"\n").unwrap();
    let e = agent_hamlet::config::AgentDir::load(&dir).err().expect("должна быть ошибка");
    assert!(e.contains("[agent].timezone") && e.contains("Nowhere/City"), "{e}");
    std::fs::write(dir.join("agent.toml"), "[agent]\ntimezone = \"Europe/Berlin\"\n\n[database]\nurl_env = \"X\"\n").unwrap();
    let ok = agent_hamlet::config::AgentDir::load(&dir);
    assert!(ok.is_ok(), "{:?}", ok.err());
    std::fs::remove_dir_all(&dir).unwrap();
}
