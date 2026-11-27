//! П5-2: метка времени входа — форма ядра (`agent/hamlet/src/daemon.rs`,
//! `time_tag`): `[Day YYYY-MM-DD HH:MM:SS TZ] [client_timezone=…]`.

use agent_hamlet::daemon::time_tag;

/// 2027-01-15 08:00:00 UTC — пятница.
const T0: i64 = 1_800_000_000_000;

#[test]
fn time_tag_follows_core_form() {
    assert_eq!(time_tag(T0, Some("UTC")), "[Fri 2027-01-15 08:00:00 UTC] [client_timezone=UTC]");
    assert_eq!(time_tag(T0, Some("Europe/Berlin")), "[Fri 2027-01-15 09:00:00 CET] [client_timezone=Europe/Berlin]");
    assert_eq!(time_tag(T0 + 3_600_000 * 16, Some("Asia/Tbilisi")), "[Sat 2027-01-16 04:00:00 +04] [client_timezone=Asia/Tbilisi]");
    // Своё: без пояса (или с неизвестным) — UTC.
    assert_eq!(time_tag(T0, None), "[Fri 2027-01-15 08:00:00 UTC] [client_timezone=UTC]");
    assert_eq!(time_tag(T0, Some("Nowhere/City")), "[Fri 2027-01-15 08:00:00 UTC] [client_timezone=UTC]");
}
