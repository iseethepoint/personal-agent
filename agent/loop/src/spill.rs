//! Большой вывод инструмента — в файл, модели — начало вывода и путь
//! (`tool_output_spill.rs`, `source-files.txt:1805`).
//!
//! Порог из ядра muse: `cmp rax, 0x19000; jbe` по адресу 0x26e045e3 — вывод
//! длиннее 102 400 байт уходит в файл. Сообщение модели — шаблон ядра
//! (адрес 0x3df026): «[Tool output exceeded 100KB ({n} bytes) and was saved
//! to {path}. Use read with path="{path}" offset={offset} to continue.]».

use std::path::{Path, PathBuf};

use agent_model::ToolResultPart;

/// Порог большого вывода в байтах текста (0x19000).
pub const SPILL_THRESHOLD_BYTES: usize = 0x19000;

/// Если текст вывода длиннее порога — весь текст пишется в
/// `dir/<call_id>.txt`, а модели остаются начало (целые строки в пределах
/// порога), сообщение muse и картинки вывода.
pub async fn bound(
    call_id: &str,
    content: Vec<ToolResultPart>,
    dir: &Path,
) -> Result<Vec<ToolResultPart>, String> {
    let text: String = content
        .iter()
        .filter_map(|p| match p {
            ToolResultPart::Text { text } => Some(text.as_str()),
            ToolResultPart::Image(_) => None,
        })
        .collect();
    if text.len() <= SPILL_THRESHOLD_BYTES {
        return Ok(content);
    }
    let path = file_for(dir, call_id);
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|e| format!("failed to create tool output spill dir: {e}"))?;
    tokio::fs::write(&path, &text)
        .await
        .map_err(|e| format!("failed to write tool output spill file: {e}"))?;

    let head = head(&text);
    let offset = head.lines().count() + 1;
    let path = path.display();
    let mut out = vec![ToolResultPart::Text {
        text: format!(
            "{head}\n[Tool output exceeded 100KB ({} bytes) and was saved to {path}. \
             Use read with path=\"{path}\" offset={offset} to continue.]",
            text.len()
        ),
    }];
    out.extend(content.into_iter().filter(|p| matches!(p, ToolResultPart::Image(_))));
    Ok(out)
}

/// Начало текста: целые строки, не длиннее порога.
fn head(text: &str) -> &str {
    let mut end = SPILL_THRESHOLD_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let cut = &text[..end];
    match cut.rfind('\n') {
        Some(i) => &cut[..i],
        None => cut,
    }
}

fn file_for(dir: &Path, call_id: &str) -> PathBuf {
    let name: String = call_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    dir.join(format!("{name}.txt"))
}
