//! Папка цели `~/workspace/goals/<slug>/` — `hatch-goals/src/workspace/*`
//! (`strings-all.txt:59672-59677`, имена `227342`): `GOAL.md` — заметки
//! агента, `files/` — документы, о которых просил владелец,
//! `hidden_files/` — служебное; `briefs/` и `crons/` появляются при
//! использовании.
//!
//! При создании рантайм пишет в `GOAL.md` заголовок с названием, id, slug и
//! описанием и больше файл не трогает (описание пространства `user_goal`,
//! `strings-all.txt:57897-57898`). Точного вида заголовка в разборе нет:
//! строки «Goal ID: », «Goal slug: », «## Description» (`:19767-19769`) и
//! метка `<!-- hatch:goal-metadata:start -->` (`:227000`) — вывод; метка у
//! нас `hamlet:goal-metadata`.
//!
//! При удалении цели `files/` с документами владельца сохраняется
//! (ответ muse несёт `preserved_files_paths` и `deleted_paths`,
//! `strings-all.txt:55731`), остальное удаляется. Вывод.

use std::path::{Path, PathBuf};

use crate::store::Goal;

pub fn dir(home: &Path, slug: &str) -> PathBuf {
    home.join("workspace/goals").join(slug)
}

pub fn header(g: &Goal) -> String {
    format!(
        "# {}\n\n<!-- hamlet:goal-metadata:start -->\nGoal ID: {}\nGoal slug: {}\n<!-- hamlet:goal-metadata:end -->\n\n## Description\n\n{}\n",
        g.title,
        g.goal_id,
        g.slug,
        g.description.as_deref().unwrap_or("").trim()
    )
}

/// Создать папку и засеять `GOAL.md`, если его ещё нет.
pub async fn ensure(home: &Path, g: &Goal) -> Result<(), String> {
    let d = dir(home, &g.slug);
    for sub in ["files", "hidden_files"] {
        tokio::fs::create_dir_all(d.join(sub)).await.map_err(|e| format!("ensure goal workspace: {e}"))?;
    }
    let guide = d.join("GOAL.md");
    if !guide.exists() {
        tokio::fs::write(&guide, header(g)).await.map_err(|e| format!("ensure goal workspace: {e}"))?;
    }
    Ok(())
}

/// Удалить папку цели, кроме непустой `files/`. Возвращает
/// (сохранённые, удалённые) пути от дома.
pub async fn remove(home: &Path, slug: &str) -> Result<(Vec<String>, Vec<String>), String> {
    let d = dir(home, slug);
    let rel = |p: &Path| format!("~/{}", p.strip_prefix(home).unwrap_or(p).display());
    let mut kept = Vec::new();
    let mut deleted = Vec::new();
    let Ok(mut rd) = tokio::fs::read_dir(&d).await else { return Ok((kept, deleted)) };
    while let Ok(Some(e)) = rd.next_entry().await {
        let p = e.path();
        let is_files = e.file_name() == "files";
        if is_files && std::fs::read_dir(&p).map(|mut r| r.next().is_some()).unwrap_or(false) {
            kept.push(rel(&p));
            continue;
        }
        let r = if p.is_dir() { tokio::fs::remove_dir_all(&p).await } else { tokio::fs::remove_file(&p).await };
        r.map_err(|e| format!("delete goal workspace: {e}"))?;
        deleted.push(rel(&p));
    }
    if kept.is_empty() {
        let _ = tokio::fs::remove_dir(&d).await;
    }
    kept.sort();
    deleted.sort();
    Ok((kept, deleted))
}
