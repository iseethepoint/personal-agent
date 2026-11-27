//! Расписание задания — форма muse `CronScheduleInput` / `ScheduleExpr`
//! (`strings-all.txt:48681`, описание поля `schedule`, вариант без `timing`,
//! 0xbe2…; проверки — тексты ошибок ядра, `strings-all.txt:84932`, `:84961`).
//!
//! Виды: `runonce` (`at`), `interval` (`every`, `at` — необязательная точка
//! отсчёта), `daily` (`time`), `weekly` (`time`, `dow`), `monthly` (`time`,
//! `dom`), `yearly` (`time`, `dom`, `month`). `timezone` — IANA; у
//! повторяющихся видов без пояса хранится `@user.current`, у `runonce` и
//! `interval` — пояс, известный при создании.
//!
//! Своё (в ядре не видно): ключ расписания в имени файла для daily–yearly
//! (`daily@0730`, `weekly@mon.fri-0730`, `monthly@1.15-0730`,
//! `yearly@3.1-0730`); «не круглая» минута `flexible_time` — из отпечатка id
//! задания. Поля `timing` и `window` (второй вариант описания muse) не
//! делаются.

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Timelike, Utc, Weekday};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const USER_CURRENT: &str = "@user.current";
const DAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schedule {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub every: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dow: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dom: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub month: Vec<u32>,
    pub timezone: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flexible_time: Option<bool>,
}

fn err(s: &str) -> String {
    s.to_string()
}

/// `55s`, `15m`, `1h` → секунды (`(?i)^(\d+)\s*([smh])$` у muse).
pub fn every_secs(raw: &str) -> Result<i64, String> {
    let t = raw.trim().to_ascii_lowercase();
    let unit = t.chars().last().ok_or_else(|| err("schedule.every must look like 55s, 15m, or 1h"))?;
    let n: i64 = t[..t.len() - 1].trim().parse().map_err(|_| err("schedule.every must look like 55s, 15m, or 1h"))?;
    let mul = match unit {
        's' => 1,
        'm' => 60,
        'h' => 3600,
        _ => return Err(err("schedule.every must look like 55s, 15m, or 1h")),
    };
    if n <= 0 {
        return Err(err("interval must be greater than zero"));
    }
    Ok(n * mul)
}

fn parse_at(raw: &str) -> Result<NaiveDateTime, String> {
    NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S")
        .or_else(|_| NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M"))
        .map_err(|_| err("schedule.at must use YYYY-MM-DDTHH:MM[:SS]"))
}

fn parse_time(raw: &str) -> Result<NaiveTime, String> {
    NaiveTime::parse_from_str(raw, "%H:%M:%S")
        .or_else(|_| NaiveTime::parse_from_str(raw, "%H:%M"))
        .map_err(|_| err("schedule.time must use HH:MM[:SS]"))
}

fn weekday(d: &str) -> Option<Weekday> {
    DAYS.iter().position(|x| x.eq_ignore_ascii_case(d)).map(|i| Weekday::try_from(i as u8).unwrap())
}

pub fn tz(name: &str) -> Result<Tz, String> {
    name.parse::<Tz>().map_err(|_| err("invalid timezone (expected IANA timezone such as America/New_York or UTC)"))
}

/// Отпечаток строки (FNV-1a) — для устойчивой «не круглой» минуты.
fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Минута, не кратная пяти, одна и та же для задания.
fn non_round_minute(id: &str) -> u32 {
    let choices: Vec<u32> = (1..60).filter(|m| m % 5 != 0).collect();
    choices[(fnv(id) % choices.len() as u64) as usize]
}

impl Schedule {
    pub fn recurring(&self) -> bool {
        !matches!(self.kind.as_str(), "runonce" | "interval")
    }

    /// Разобрать `schedule` из аргументов. `prev` — сохранённое расписание
    /// (при `cron.update` пропущенные поля берутся из него); `user_tz` —
    /// текущий пояс владельца, если известен.
    pub fn from_input(v: &Value, prev: Option<&Schedule>, user_tz: Option<&str>, id: &str) -> Result<Schedule, String> {
        let o = v.as_object().ok_or_else(|| err("schedule must be a JSON object"))?;
        let s = |k: &str| o.get(k).and_then(Value::as_str).map(String::from);
        let kind = s("kind").or_else(|| prev.map(|p| p.kind.clone())).ok_or_else(|| err("schedule required"))?;
        if !["runonce", "interval", "daily", "weekly", "monthly", "yearly"].contains(&kind.as_str()) {
            return Err(err("schedule.kind must be one of runonce|interval|daily|weekly|monthly|yearly"));
        }
        let same_kind = prev.filter(|p| p.kind == kind);
        let list_u32 = |k: &str, lo: u32, hi: u32, msg: &str| -> Result<Option<Vec<u32>>, String> {
            match o.get(k) {
                None => Ok(None),
                Some(Value::Array(a)) => a
                    .iter()
                    .map(|x| x.as_u64().map(|n| n as u32).filter(|n| (lo..=hi).contains(n)).ok_or_else(|| err(msg)))
                    .collect::<Result<Vec<_>, _>>()
                    .map(Some),
                Some(x) => x.as_u64().map(|n| n as u32).filter(|n| (lo..=hi).contains(n)).map(|n| Some(vec![n])).ok_or_else(|| err(msg)),
            }
        };
        let mut out = Schedule {
            kind: kind.clone(),
            at: s("at").or_else(|| same_kind.and_then(|p| p.at.clone())),
            every: s("every").or_else(|| same_kind.and_then(|p| p.every.clone())),
            time: s("time").or_else(|| same_kind.and_then(|p| p.time.clone())),
            dow: match o.get("dow") {
                Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).map(String::from).collect(),
                Some(Value::String(d)) => vec![d.clone()],
                _ => same_kind.map(|p| p.dow.clone()).unwrap_or_default(),
            },
            dom: list_u32("dom", 1, 31, "schedule.dom entries must be 1..31")?.or_else(|| same_kind.map(|p| p.dom.clone())).unwrap_or_default(),
            month: list_u32("month", 1, 12, "schedule.month entries must be 1..12")?
                .or_else(|| same_kind.map(|p| p.month.clone()))
                .unwrap_or_default(),
            timezone: String::new(),
            flexible_time: o.get("flexible_time").and_then(Value::as_bool).or_else(|| prev.and_then(|p| p.flexible_time)),
        };
        // Пояс: явный, сохранённый или текущий владельца.
        let given = s("timezone").or_else(|| prev.map(|p| p.timezone.clone()));
        out.timezone = match given {
            Some(t) if t.starts_with("@user.") => {
                if !out.recurring() {
                    return Err(err("symbolic timezone (@user.*) is only valid for recurring schedules; use a literal IANA timezone for one-shot reminders and intervals"));
                }
                if t != USER_CURRENT {
                    return Err(err("Retry with @user.current, @user.home, @user.work, or a literal IANA timezone."));
                }
                t
            }
            Some(t) => {
                tz(&t).map_err(|_| err("schedule.timezone must be a literal IANA timezone"))?;
                t
            }
            None => {
                let current = user_tz.ok_or_else(|| {
                    err("schedule.timezone required (the user's current timezone is not known yet); pass an IANA timezone such as America/New_York")
                })?;
                if out.recurring() {
                    USER_CURRENT.to_string()
                } else {
                    current.to_string()
                }
            }
        };
        match kind.as_str() {
            "runonce" => {
                parse_at(out.at.as_deref().ok_or_else(|| err("schedule.at must use YYYY-MM-DDTHH:MM[:SS]"))?)?;
            }
            "interval" => {
                every_secs(out.every.as_deref().ok_or_else(|| err("schedule.every must look like 55s, 15m, or 1h"))?)?;
                if let Some(a) = &out.at {
                    parse_at(a)?;
                }
            }
            _ => {
                let t = parse_time(out.time.as_deref().ok_or_else(|| err("schedule.time must use HH:MM[:SS]"))?)?;
                if out.flexible_time == Some(true) && t.minute() % 5 == 0 {
                    let fixed = t.with_minute(non_round_minute(id)).unwrap();
                    out.time = Some(fixed.format("%H:%M").to_string());
                }
            }
        }
        match kind.as_str() {
            "weekly" => {
                if out.dow.is_empty() {
                    return Err(err("schedule.dow required"));
                }
                for d in &out.dow {
                    weekday(d).ok_or_else(|| err("schedule.dow entries must be Mon..Sun"))?;
                }
            }
            "monthly" if out.dom.is_empty() => return Err(err("schedule.dom required")),
            "yearly" if out.dom.is_empty() => return Err(err("schedule.dom required")),
            "yearly" if out.month.is_empty() => return Err(err("schedule.month required")),
            _ => {}
        }
        Ok(out)
    }

    fn zone(&self, user_tz: Option<&str>) -> Result<Tz, String> {
        if self.timezone == USER_CURRENT {
            tz(user_tz.ok_or_else(|| err("schedule.timezone required (the user's current timezone is not known yet); pass an IANA timezone such as America/New_York"))?)
        } else {
            tz(&self.timezone)
        }
    }

    /// Ближайший срок строго позже `after_ms`. `anchor_ms` — точка отсчёта
    /// интервала без `at` (время создания). `None` — сроков больше нет
    /// (разовое уже сработало).
    pub fn next_after(&self, after_ms: i64, anchor_ms: i64, user_tz: Option<&str>) -> Result<Option<i64>, String> {
        let zone = self.zone(user_tz)?;
        let local = |ndt: NaiveDateTime| zone.from_local_datetime(&ndt).earliest().map(|d| d.timestamp_millis());
        match self.kind.as_str() {
            "runonce" => {
                let at = local(parse_at(self.at.as_deref().unwrap_or_default())?).ok_or_else(|| err("unable to compute local run"))?;
                Ok((at > after_ms).then_some(at))
            }
            "interval" => {
                let step = every_secs(self.every.as_deref().unwrap_or_default())? * 1000;
                let anchor = match &self.at {
                    Some(a) => local(parse_at(a)?).ok_or_else(|| err("unable to compute local run"))?,
                    None => anchor_ms,
                };
                if anchor > after_ms {
                    return Ok(Some(anchor));
                }
                let k = (after_ms - anchor) / step + 1;
                Ok(Some(anchor + k * step))
            }
            _ => {
                let time = parse_time(self.time.as_deref().unwrap_or_default())?;
                let start = Utc.timestamp_millis_opt(after_ms).single().ok_or_else(|| err("schedule anchor is out of range"))?;
                let mut day: NaiveDate = start.with_timezone(&zone).date_naive();
                // Ищем не дальше пяти лет вперёд (29 февраля — раз в четыре года).
                for _ in 0..(366 * 5) {
                    let ok = match self.kind.as_str() {
                        "daily" => true,
                        "weekly" => self.dow.iter().filter_map(|d| weekday(d)).any(|w| w == day.weekday()),
                        "monthly" => self.dom.contains(&day.day()),
                        _ => self.dom.contains(&day.day()) && self.month.contains(&day.month()),
                    };
                    if ok {
                        if let Some(ms) = local(day.and_time(time)) {
                            if ms > after_ms {
                                return Ok(Some(ms));
                            }
                        }
                    }
                    day += Duration::days(1);
                }
                Err(err("unable to compute local run"))
            }
        }
    }

    /// Папка вида в `cron.d` (`CRON_SCHEDULE_DIRS` muse): интервал — по
    /// единице `every`.
    pub fn folder(&self) -> &'static str {
        match self.kind.as_str() {
            "runonce" => "runonce",
            "interval" => match self.every.as_deref().unwrap_or("").trim().to_ascii_lowercase().chars().last() {
                Some('s') => "secondly",
                Some('h') => "hourly",
                _ => "minutely",
            },
            "daily" => "daily",
            "weekly" => "weekly",
            "monthly" => "monthly",
            _ => "yearly",
        }
    }

    /// Ключ расписания в имени файла `<id>__<ключ>.md`.
    pub fn key(&self) -> String {
        let hm = || self.time.as_deref().unwrap_or("").replace(':', "").chars().take(4).collect::<String>();
        let join = |v: &[u32]| v.iter().map(u32::to_string).collect::<Vec<_>>().join(".");
        match self.kind.as_str() {
            "runonce" => {
                let at = self.at.clone().unwrap_or_default();
                format!("runonce@{}", if at.len() == 16 { format!("{at}:00") } else { at })
            }
            "interval" => format!("interval@{}", self.every.as_deref().unwrap_or("").trim().to_ascii_lowercase()),
            "daily" => format!("daily@{}", hm()),
            "weekly" => format!("weekly@{}-{}", self.dow.iter().map(|d| d.to_ascii_lowercase()).collect::<Vec<_>>().join("."), hm()),
            "monthly" => format!("monthly@{}-{}", join(&self.dom), hm()),
            _ => format!("yearly@{}.{}-{}", join(&self.month), join(&self.dom), hm()),
        }
    }

    /// Промежуток до следующего срока после `from_ms` — для бюджета времени
    /// повторяющихся заданий.
    pub fn gap_secs(&self, from_ms: i64, anchor_ms: i64, user_tz: Option<&str>) -> Option<i64> {
        let a = self.next_after(from_ms, anchor_ms, user_tz).ok()??;
        let b = self.next_after(a, anchor_ms, user_tz).ok()??;
        Some((b - a) / 1000)
    }
}

/// Местное время срока с днём недели: `Tue 2026-11-27 09:00` (`next_run_local`).
pub fn local_label(ms: i64, zone: &str, user_tz: Option<&str>) -> String {
    let name = if zone == USER_CURRENT { user_tz.unwrap_or("UTC") } else { zone };
    match (tz(name), Utc.timestamp_millis_opt(ms).single()) {
        (Ok(z), Some(t)) => t.with_timezone(&z).format("%a %Y-%m-%d %H:%M").to_string(),
        _ => String::new(),
    }
}

/// Сколько ждать: `9h 12m` (`next_run_in`).
pub fn wait_label(ms: i64) -> String {
    let m = (ms.max(0) + 59_999) / 60_000;
    let (d, h, mm) = (m / 1440, (m % 1440) / 60, m % 60);
    match (d, h) {
        (0, 0) => format!("{mm}m"),
        (0, _) => format!("{h}h {mm}m"),
        _ => format!("{d}d {h}h {mm}m"),
    }
}

/// Час владельца сейчас (для предупреждения о «small hours»).
pub fn local_hour(ms: i64, user_tz: Option<&str>) -> Option<u32> {
    let z = tz(user_tz?).ok()?;
    Some(Utc.timestamp_millis_opt(ms).single()?.with_timezone(&z).hour())
}
