//! Разбор сценария: `export const meta = { … };` в начале, затем тело.
//! Тексты ошибок — ядро muse (`strings-all.txt:337746`, «invalid workflow
//! metadata: », «invalid workflow script: »).
//!
//! Своё: литерал meta вычисляется движком в чистом контексте (у muse —
//! свой разборщик литерала: «expected `,` or `}` in meta object»); имя без
//! литерала (`{ name }`) так же не проходит — в чистом контексте имени нет.

use serde_json::{json, Value};

use crate::js;

#[derive(Debug, Clone, PartialEq)]
pub struct Meta {
    pub name: String,
    pub description: String,
    /// Имена фаз по порядку.
    pub phases: Vec<String>,
    /// Фазы как объекты `{name, title?, detail?, description?}`.
    pub phase_details: Vec<Value>,
}

#[derive(Debug, Clone)]
pub struct Script {
    pub meta: Meta,
    /// Тело без `export const meta`.
    pub body: String,
}

fn meta_err(e: impl std::fmt::Display) -> String {
    format!("invalid workflow metadata: {e}")
}

/// Пропуск пробелов и комментариев в начале.
fn skip_trivia(s: &str) -> usize {
    let mut i = 0;
    let b = s.as_bytes();
    loop {
        while i < b.len() && (b[i] as char).is_whitespace() {
            i += 1;
        }
        if s[i..].starts_with("//") {
            i += s[i..].find('\n').map(|n| n + 1).unwrap_or(s.len() - i);
        } else if s[i..].starts_with("/*") {
            i += s[i..].find("*/").map(|n| n + 2).unwrap_or(s.len() - i);
        } else {
            return i;
        }
    }
}

/// Конец литерала объекта, начинающегося с `{` в `s[start]`.
fn object_end(s: &str, start: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (i, c) in s[start..].char_indices() {
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' | '`' => quote = Some(c),
            '{' | '[' => depth += 1,
            '}' | ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(start + i + 1);
                }
            }
            _ => {}
        }
    }
    None
}

fn phase(v: &Value) -> Result<(String, Value), String> {
    match v {
        Value::String(s) if !s.trim().is_empty() => Ok((s.clone(), json!({"name": s}))),
        Value::Object(o) => {
            let name = o.get("name").or_else(|| o.get("title")).and_then(Value::as_str).filter(|s| !s.trim().is_empty());
            let name = name.ok_or_else(|| meta_err("meta.phases object must include `name` or `title`"))?;
            let mut d = o.clone();
            d.insert("name".into(), name.into());
            Ok((name.to_string(), Value::Object(d)))
        }
        _ => Err(meta_err("expected string literal or phase object in meta.phases")),
    }
}

pub fn parse(source: &str) -> Result<Script, String> {
    let start = skip_trivia(source);
    let rest = &source[start..];
    let Some(after) = rest.strip_prefix("export const meta") else {
        return Err(meta_err("missing `export const meta`"));
    };
    let eq = after.trim_start();
    let Some(eq) = eq.strip_prefix('=') else {
        return Err(meta_err("missing `export const meta`"));
    };
    let obj_start = source.len() - eq.trim_start().len();
    if !source[obj_start..].starts_with('{') {
        return Err(meta_err("meta must be an object literal"));
    }
    let end = object_end(source, obj_start).ok_or_else(|| meta_err("expected `,` or `}` in meta object"))?;
    let v = js::literal(&source[obj_start..end]).map_err(meta_err)?;
    let mut body_start = end;
    let tail = &source[end..];
    let t = tail.trim_start();
    if let Some(x) = t.strip_prefix(';') {
        body_start = source.len() - x.len();
    }
    let body = source[body_start..].to_string();
    if body.lines().any(|l| l.trim_start().starts_with("import ") || l.trim_start().starts_with("import{")) {
        return Err("invalid workflow script: imports are not available in workflow scripts".into());
    }
    let text = |k: &str| v.get(k).and_then(Value::as_str).filter(|s| !s.trim().is_empty()).map(String::from);
    let name = text("name").ok_or_else(|| meta_err("missing meta.name"))?;
    let description = text("description").ok_or_else(|| meta_err("missing meta.description"))?;
    let raw = v.get("phases").and_then(Value::as_array).ok_or_else(|| meta_err("missing meta.phases"))?;
    if raw.is_empty() {
        return Err(meta_err("meta.phases must contain at least one phase"));
    }
    let mut phases = vec![];
    let mut phase_details = vec![];
    for p in raw {
        let (n, d) = phase(p)?;
        phases.push(n);
        phase_details.push(d);
    }
    Ok(Script { meta: Meta { name, description, phases, phase_details }, body })
}

/// Имя сохранённой работы: только ASCII-буквы, цифры, `_`, `-`
/// (`strings-all.txt:337746`).
pub fn check_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("workflow name must not be empty".into());
    }
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(format!("invalid workflow name `{name}`: must not contain path separators or traversal"));
    }
    if name == "." || name.starts_with('.') {
        return Err(format!("invalid workflow name `{name}`: reserved path segment"));
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err(format!("invalid workflow name `{name}`: expected only ASCII letters, numbers, `_`, or `-`"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meta_literal_and_errors() {
        let s = parse("// note\nexport const meta = { name: \"trip\", description: \"Plan {a} trip\", phases: [\"gather\", { title: \"plan\", detail: \"d\" }] };\nawait phase(\"gather\");\nreturn 1;").unwrap();
        assert_eq!(s.meta.name, "trip");
        assert_eq!(s.meta.phases, vec!["gather", "plan"]);
        assert_eq!(s.meta.phase_details[1]["detail"], "d");
        assert!(s.body.trim_start().starts_with("await phase"));
        assert_eq!(parse("return 1;").unwrap_err(), "invalid workflow metadata: missing `export const meta`");
        assert_eq!(parse("export const meta = { name: \"x\", phases: [\"a\"] };").unwrap_err(), "invalid workflow metadata: missing meta.description");
        assert_eq!(parse("export const meta = { name: \"x\", description: \"d\", phases: [] };").unwrap_err(), "invalid workflow metadata: meta.phases must contain at least one phase");
        assert!(parse("const name = \"x\";\nexport const meta = { name };").is_err());
        assert!(parse("export const meta = { name, description: \"d\", phases: [\"a\"] };").is_err());
        assert_eq!(check_name("a/b").unwrap_err(), "invalid workflow name `a/b`: must not contain path separators or traversal");
        assert!(check_name("trip-2_x").is_ok());
        assert!(check_name("trip plan").is_err());
    }
}
