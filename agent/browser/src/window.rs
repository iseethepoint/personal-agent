//! Окно браузера: вкладки, виды страницы и действия на них — форма muse
//! (`browser.observe` / `browser.act` / `browser.tabs`,
//! `prompts/browser/tools/*.md`).
//!
//! Из ядра muse (`strings-all.txt`): ссылки на элементы `@eN` (L33551,
//! L75021), вкладки `tN`, виды `vN`, снимки `iN` (L39961, `observe.md`);
//! заголовок вида «## Tab », «, view » (L16063), «Title:»; пояснения
//! «This view shows the first … of the page's … of text.» (L40717), «This
//! view shows the requested fields of one element.», «The page has no
//! usable text, so read the screenshot.» (L49640); состояния шагов «not
//! started», «timed out», «applied», «outcome unknown» и факты «the page
//! did not navigate» / «the page updated in place» / «the page navigated» /
//! «the field does not show the text that was typed» / «the page
//! reformatted the text» / «it already had that value»; итоговые фразы
//! «The action applied. Read the new view before you continue.» и др.
//! (L49657); «## Tabs in this window», «This window has no open tabs.»,
//! «It loaded the requested URL.» (L49711-49713).
//!
//! Своё (в ядре muse вид собирает отдельный «browser broker», его разметки
//! в разборе нет): строки вида — дерево доступности Chromium
//! (`Accessibility.getFullAXTree`) в виде `- роль "имя" @eN [состояния]`;
//! ссылки получают элементы с ролями управления, ссылка держится за
//! элементом, пока он есть на странице; предел текста вида `VIEW_MAX_CHARS`;
//! снимок — JPEG видимой части; склейка фраз результата; признак «страница
//! ушла» — смена `performance.timeOrigin`. Старые виды в истории разговора
//! не вычищаются: у нас нет хода, который правил бы прошлые результаты.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use agent_model::{Image, ToolResultPart};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::cdp::{Cdp, Event};
use crate::chromium::Chromium;

/// Предел текста вида. Своё (число у muse не найдено).
pub const VIEW_MAX_CHARS: usize = 30_000;

/// Роли, которые получают ссылку `@eN`.
const CONTROLS: [&str; 18] = [
    "button", "link", "textbox", "searchbox", "checkbox", "radio", "combobox", "listbox", "option", "menuitem", "menuitemcheckbox", "menuitemradio", "tab", "switch", "slider",
    "spinbutton", "treeitem", "ListBoxOption",
];
/// Узлы без своей строки: их дети поднимаются на уровень выше.
const TRANSPARENT: [&str; 8] = ["generic", "none", "InlineTextBox", "LineBreak", "MenuListPopup", "RootWebArea", "paragraph", "LabelText"];

struct Tab {
    id: String,
    target: String,
    session: String,
    /// Ссылки последнего вида: `eN` → `backendNodeId`.
    refs: HashMap<String, i64>,
    /// `backendNodeId` → `eN`, чтобы ссылка держалась за элементом.
    stable: HashMap<i64, String>,
    has_view: bool,
}

struct State {
    tabs: Vec<Tab>,
    selected: Option<String>,
    next_tab: u32,
    next_view: u32,
    next_ref: u32,
}

/// Браузер одного профиля: один процесс Chromium, в нём окна. Все окна
/// профиля делят его cookies и входы («Every window shares the default
/// profile», `browser.windows` muse); Chromium не даёт двум процессам один
/// профиль, поэтому процесс на профиль один.
pub struct Browser {
    chromium: Mutex<Option<Chromium>>,
    cdp: Arc<Cdp>,
    /// Вкладки, открытые страницей (`window.open`, `target=_blank`); свой
    /// замок — задача событий не ждёт идущего действия. Признак `openerId`
    /// не годится: у ссылки `target=_blank` его нет (noopener), поэтому
    /// новую вкладку забирает окно, чьё действие идёт (своё).
    popups: std::sync::Mutex<Vec<String>>,
    /// Вкладки, которые уже чьи-то (окна или стартовая пустая).
    known: std::sync::Mutex<std::collections::HashSet<String>>,
}

impl Browser {
    pub async fn launch(bin: &Path, profile_dir: &Path, headless: bool) -> Result<Arc<Browser>, String> {
        let chromium = Chromium::launch(bin, profile_dir, headless).await?;
        let cdp = chromium.cdp.clone();
        cdp.call(None, "Target.setDiscoverTargets", json!({"discover": true})).await?;
        let targets = cdp.call(None, "Target.getTargets", json!({})).await?;
        let known = targets["targetInfos"].as_array().into_iter().flatten().map(|t| s(t, "targetId")).collect();
        let b = Arc::new(Browser { chromium: Mutex::new(Some(chromium)), cdp: cdp.clone(), popups: std::sync::Mutex::new(Vec::new()), known: std::sync::Mutex::new(known) });
        let weak = Arc::downgrade(&b);
        let mut events = cdp.events();
        tokio::spawn(async move {
            while let Ok(Event { method, params, .. }) = events.recv().await {
                let Some(b) = weak.upgrade() else { break };
                let info = &params["targetInfo"];
                if method == "Target.targetCreated" && s(info, "type") == "page" {
                    b.popups.lock().unwrap().push(s(info, "targetId"));
                }
            }
        });
        Ok(b)
    }

    /// Закрыть процесс браузера.
    pub async fn close(&self) {
        if let Some(c) = self.chromium.lock().await.take() {
            c.close().await;
        }
    }
}

pub struct Window {
    browser: Arc<Browser>,
    /// Окно само запустило браузер и закрывает его вместе с собой.
    owns_browser: bool,
    cdp: Arc<Cdp>,
    state: Mutex<State>,
}

/// Вид вкладки: текст и снимок.
pub struct View {
    pub text: String,
    pub screenshot: Option<(String, Image)>,
}

impl View {
    pub fn parts(self) -> Vec<ToolResultPart> {
        let mut v = vec![ToolResultPart::Text { text: self.text }];
        if let Some((id, img)) = self.screenshot {
            v.push(ToolResultPart::Text { text: format!("Screenshot {id} of this view:") });
            v.push(ToolResultPart::Image(img));
        }
        v
    }
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn ax_value(v: &Value, k: &str) -> Option<Value> {
    v.get(k).and_then(|x| x.get("value")).cloned()
}

fn quote(t: &str) -> String {
    let one: String = t.split_whitespace().collect::<Vec<_>>().join(" ");
    format!("\"{}\"", one.replace('"', "\\\""))
}

/// Код отказа шага.
#[derive(Debug)]
enum Step {
    Applied(Vec<String>),
    Refused(String),
    TimedOut(String),
    Unknown(String),
}

impl Window {
    /// Открыть окно со своим Chromium и профилем `profile_dir`; браузер
    /// закрывается вместе с окном.
    pub async fn open(bin: &Path, profile_dir: &Path, headless: bool) -> Result<Arc<Window>, String> {
        let browser = Browser::launch(bin, profile_dir, headless).await?;
        let targets = browser.cdp.call(None, "Target.getTargets", json!({})).await?;
        let first = targets["targetInfos"].as_array().into_iter().flatten().find(|t| s(t, "type") == "page").map(|t| s(t, "targetId"));
        Self::with_target(browser, true, first).await
    }

    /// Новое окно в уже запущенном браузере профиля.
    pub async fn new_in(browser: Arc<Browser>) -> Result<Arc<Window>, String> {
        Self::with_target(browser, false, None).await
    }

    async fn with_target(browser: Arc<Browser>, owns_browser: bool, target: Option<String>) -> Result<Arc<Window>, String> {
        let cdp = browser.cdp.clone();
        let target = match target {
            Some(t) => t,
            None => s(&cdp.call(None, "Target.createTarget", json!({"url": "about:blank", "newWindow": true})).await?, "targetId"),
        };
        browser.popups.lock().unwrap().retain(|p| *p != target);
        let w = Arc::new(Window { browser, owns_browser, cdp, state: Mutex::new(State { tabs: vec![], selected: None, next_tab: 1, next_view: 1, next_ref: 1 }) });
        let mut st = w.state.lock().await;
        let id = w.attach(&mut st, &target).await?;
        st.selected = Some(id);
        drop(st);
        Ok(w)
    }

    async fn attach(&self, st: &mut State, target: &str) -> Result<String, String> {
        let r = self.cdp.call(None, "Target.attachToTarget", json!({"targetId": target, "flatten": true})).await?;
        let session = s(&r, "sessionId");
        self.browser.known.lock().unwrap().insert(target.to_string());
        for m in ["Page.enable", "DOM.enable", "Accessibility.enable"] {
            self.cdp.call(Some(&session), m, json!({})).await?;
        }
        let id = format!("t{}", st.next_tab);
        st.next_tab += 1;
        st.tabs.push(Tab { id: id.clone(), target: target.into(), session, refs: HashMap::new(), stable: HashMap::new(), has_view: false });
        Ok(id)
    }

    /// Закрыть окно: его вкладки, а свой браузер — целиком.
    pub async fn close(&self) {
        if self.owns_browser {
            self.browser.close().await;
            return;
        }
        let targets: Vec<String> = self.state.lock().await.tabs.drain(..).map(|t| t.target).collect();
        for t in targets {
            let _ = self.cdp.call(None, "Target.closeTarget", json!({"targetId": t})).await;
        }
    }

    /// Список вкладок окна для `browser.windows`.
    pub async fn tab_lines(&self) -> Vec<String> {
        let st = self.state.lock().await;
        let mut out = vec![];
        for t in &st.tabs {
            let info = self.eval(&t.session, "[document.title, location.href]").await.unwrap_or(Value::Null);
            out.push(format!("- {}: Title: {} URL: {}", t.id, info[0].as_str().unwrap_or_default(), info[1].as_str().unwrap_or_default()));
        }
        out
    }

    pub async fn tab_ids(&self) -> Vec<String> {
        self.state.lock().await.tabs.iter().map(|t| t.id.clone()).collect()
    }

    fn session_of(st: &State, tab: &str) -> Result<String, String> {
        st.tabs.iter().find(|t| t.id == tab).map(|t| t.session.clone()).ok_or_else(|| format!("{tab} is not open in this window. Report it in browser.report."))
    }

    async fn eval(&self, session: &str, expr: &str) -> Result<Value, String> {
        let r = self.cdp.call(Some(session), "Runtime.evaluate", json!({"expression": expr, "returnByValue": true, "awaitPromise": true})).await?;
        if let Some(e) = r.get("exceptionDetails") {
            return Err(format!("page script failed: {}", e["exception"]["description"].as_str().unwrap_or("error")));
        }
        Ok(r["result"]["value"].clone())
    }

    async fn on_node(&self, session: &str, node: i64, func: &str, args: Vec<Value>) -> Result<Value, String> {
        let o = self.cdp.call(Some(session), "DOM.resolveNode", json!({"backendNodeId": node})).await?;
        let oid = s(&o["object"], "objectId");
        let args: Vec<Value> = args.into_iter().map(|v| json!({"value": v})).collect();
        let r = self
            .cdp
            .call(Some(session), "Runtime.callFunctionOn", json!({"objectId": oid, "functionDeclaration": func, "arguments": args, "returnByValue": true, "awaitPromise": true}))
            .await?;
        Ok(r["result"]["value"].clone())
    }

    /// Дождаться загрузки документа. Срок — своё.
    async fn settle(&self, session: &str) {
        tokio::time::sleep(Duration::from_millis(150)).await;
        for _ in 0..100 {
            if let Ok(v) = self.eval(session, "document.readyState").await {
                if v == "complete" {
                    return;
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    async fn marker(&self, session: &str) -> (f64, String) {
        let v = self.eval(session, "[performance.timeOrigin, location.href, (document.body ? document.body.innerText : '')]").await.unwrap_or(Value::Null);
        let text = v[2].as_str().unwrap_or_default();
        (v[0].as_f64().unwrap_or(0.0), format!("{}|{}|{}", v[1].as_str().unwrap_or_default(), text.len(), text.chars().take(4000).collect::<String>()))
    }

    async fn screenshot(&self, session: &str, st: &mut State, view_no: u32) -> Option<(String, Image)> {
        let r = self.cdp.call(Some(session), "Page.captureScreenshot", json!({"format": "jpeg", "quality": 70})).await.ok()?;
        let _ = st;
        Some((format!("i{view_no}"), Image { media_type: "image/jpeg".into(), data_base64: s(&r, "data") }))
    }

    /// Вид вкладки: `auto`, `text` или `element` с `fields`; `query` —
    /// только совпавшие строки.
    pub async fn observe(&self, tab: &str, view: &str, query: Option<&str>, element: Option<(&str, &[String])>) -> Result<View, String> {
        let mut st = self.state.lock().await;
        self.view_locked(&mut st, tab, view, query, element).await
    }

    async fn view_locked(&self, st: &mut State, tab: &str, view: &str, query: Option<&str>, element: Option<(&str, &[String])>) -> Result<View, String> {
        let session = Self::session_of(st, tab)?;
        if view == "element" {
            let (r, fields) = element.ok_or("the element view needs a target from the latest view")?;
            let node = Self::resolve(st, tab, r)?;
            let v = self
                .on_node(
                    &session,
                    node,
                    "function(){const r=this.getAttribute&&this.getAttribute('role');return {text:(this.innerText||this.textContent||'').trim(),value:('value' in this)?(this.type==='password'?'[REDACTED SENSITIVE VALUE]':String(this.value)):null,href:this.href||null,image_url:this.currentSrc||this.src||null,role:r||this.tagName.toLowerCase(),name:this.getAttribute&&(this.getAttribute('aria-label')||this.getAttribute('name')||this.getAttribute('title'))||null,state:{checked:this.checked===undefined?null:this.checked,disabled:!!this.disabled,selected:this.selected===undefined?null:this.selected}}}",
                    vec![],
                )
                .await?;
            let n = st.next_view;
            st.next_view += 1;
            let mut lines = vec![format!("## Tab {tab}, view v{n}"), "This view shows the requested fields of one element.".into()];
            for f in fields {
                lines.push(format!("{f}: {}", match &v[f.as_str()] {
                    Value::String(x) => x.clone(),
                    Value::Null => "(none)".into(),
                    other => other.to_string(),
                }));
            }
            let shot = self.screenshot(&session, st, n).await;
            return Ok(View { text: lines.join("\n"), screenshot: shot });
        }
        let info = self.eval(&session, "[document.title, location.href]").await?;
        let (title, url) = (info[0].as_str().unwrap_or_default().to_string(), info[1].as_str().unwrap_or_default().to_string());
        let body = if view == "text" {
            self.eval(&session, "document.body ? document.body.innerText : ''").await?.as_str().unwrap_or_default().to_string()
        } else {
            self.ax_tree(st, tab, &session).await?
        };
        let n = st.next_view;
        st.next_view += 1;
        if let Some(t) = st.tabs.iter_mut().find(|t| t.id == tab) {
            t.has_view = true;
        }
        let mut body = match query {
            Some(q) => {
                let q = q.to_lowercase();
                body.lines().filter(|l| l.to_lowercase().contains(&q)).collect::<Vec<_>>().join("\n")
            }
            None => body,
        };
        let mut notes = vec![];
        let total = body.chars().count();
        if total > VIEW_MAX_CHARS {
            body = body.chars().take(VIEW_MAX_CHARS).collect();
            notes.push(format!("This view shows the first {VIEW_MAX_CHARS} of the page's {total} characters of text. This view does not show the whole page."));
        }
        if body.trim().is_empty() {
            notes.push("The page has no usable text, so read the screenshot.".into());
        }
        let mut text = format!("## Tab {tab}, view v{n}\nTitle: {title}\nURL: {url}");
        for x in notes {
            text.push('\n');
            text.push_str(&x);
        }
        text.push_str("\n\n");
        text.push_str(&body);
        let shot = self.screenshot(&session, st, n).await;
        Ok(View { text, screenshot: shot })
    }

    async fn ax_tree(&self, st: &mut State, tab: &str, session: &str) -> Result<String, String> {
        let r = self.cdp.call(Some(session), "Accessibility.getFullAXTree", json!({})).await?;
        let nodes = r["nodes"].as_array().cloned().unwrap_or_default();
        let by_id: HashMap<String, &Value> = nodes.iter().map(|n| (s(n, "nodeId"), n)).collect();
        let root = nodes.iter().find(|n| n.get("parentId").is_none()).map(|n| s(n, "nodeId"));
        let mut out = Vec::new();
        let mut refs = HashMap::new();
        let next_ref = &mut st.next_ref;
        let t = st.tabs.iter_mut().find(|t| t.id == tab).ok_or("tab gone")?;
        // Ссылка держится за элементом, пока жив документ.
        let mut stable = std::mem::take(&mut t.stable);
        if let Some(root) = root {
            let mut stack: Vec<(String, usize, String)> = vec![(root, 0, String::new())];
            while let Some((id, depth, parent_name)) = stack.pop() {
                let Some(n) = by_id.get(&id) else { continue };
                let role = ax_value(n, "role").and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
                let name = ax_value(n, "name").and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
                let ignored = n.get("ignored").and_then(Value::as_bool).unwrap_or(false);
                let mut child_depth = depth;
                let mut child_parent = parent_name.clone();
                if !ignored && !TRANSPARENT.contains(&role.as_str()) {
                    if role == "StaticText" {
                        let one = name.split_whitespace().collect::<Vec<_>>().join(" ");
                        if !one.is_empty() && !parent_name.contains(&one) {
                            out.push(format!("{}{}", "  ".repeat(depth), one));
                        }
                    } else {
                        let mut line = format!("{}- {role}", "  ".repeat(depth));
                        if !name.trim().is_empty() {
                            line.push(' ');
                            line.push_str(&quote(&name));
                        }
                        if CONTROLS.contains(&role.as_str()) {
                            if let Some(b) = n.get("backendDOMNodeId").and_then(Value::as_i64) {
                                let r = stable.entry(b).or_insert_with(|| {
                                    let r = format!("e{next_ref}");
                                    *next_ref += 1;
                                    r
                                });
                                line.push_str(&format!(" @{r}"));
                                refs.insert(r.clone(), b);
                            }
                        }
                        if let Some(v) = ax_value(n, "value").and_then(|v| v.as_str().map(String::from)) {
                            if !v.is_empty() && v != name {
                                line.push_str(&format!(" value={}", quote(&v)));
                            }
                        }
                        let mut states = vec![];
                        for p in n["properties"].as_array().into_iter().flatten() {
                            let pn = s(p, "name");
                            let pv = &p["value"]["value"];
                            match pn.as_str() {
                                "checked" | "pressed" if pv == "true" || pv == true => states.push(pn.clone()),
                                "disabled" | "expanded" | "selected" | "focused" | "required" | "invalid" if pv == true || pv == "true" => states.push(pn.clone()),
                                "level" => states.push(format!("level={pv}")),
                                _ => {}
                            }
                        }
                        if !states.is_empty() {
                            line.push_str(&format!(" [{}]", states.join(", ")));
                        }
                        out.push(line);
                        child_depth = depth + 1;
                        // Текст внутри элемента повторяет его имя или значение.
                        let value = ax_value(n, "value").and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
                        child_parent = format!("{name} {value}").split_whitespace().collect::<Vec<_>>().join(" ");
                    }
                }
                let kids: Vec<String> = n["childIds"].as_array().into_iter().flatten().filter_map(|c| c.as_str().map(String::from)).collect();
                for k in kids.into_iter().rev() {
                    stack.push((k, child_depth, child_parent.clone()));
                }
            }
        }
        // Узлы, которых больше нет, теряют ссылку.
        stable.retain(|b, _| refs.values().any(|x| x == b));
        t.stable = stable;
        t.refs = refs;
        Ok(out.join("\n"))
    }

    fn resolve(st: &State, tab: &str, r: &str) -> Result<i64, String> {
        let t = st.tabs.iter().find(|t| t.id == tab).ok_or_else(|| format!("{tab} is not open in this window. Report it in browser.report."))?;
        if !t.has_view {
            return Err(format!("There is no current view of tab {tab}. Observe it, then make this call again."));
        }
        let key = r.trim_start_matches('@');
        t.refs.get(key).copied().ok_or_else(|| format!("@{key} no longer names a control by its reference in the latest view, such as @e12. Observe the tab to find it."))
    }

    async fn center(&self, session: &str, node: i64) -> Result<(f64, f64), Step> {
        let _ = self.on_node(session, node, "function(){this.scrollIntoView({block:'center',inline:'center'})}", vec![]).await;
        let q = self.cdp.call(Some(session), "DOM.getContentQuads", json!({"backendNodeId": node})).await.map_err(|_| Step::Refused("not_visible".into()))?;
        let quad = q["quads"].as_array().and_then(|a| a.first()).and_then(Value::as_array).cloned().ok_or(Step::Refused("not_visible".into()))?;
        let xs: Vec<f64> = quad.iter().step_by(2).filter_map(Value::as_f64).collect();
        let ys: Vec<f64> = quad.iter().skip(1).step_by(2).filter_map(Value::as_f64).collect();
        if xs.len() < 4 || ys.len() < 4 {
            return Err(Step::Refused("not_visible".into()));
        }
        let (x, y) = (xs.iter().sum::<f64>() / 4.0, ys.iter().sum::<f64>() / 4.0);
        let w = xs.iter().cloned().fold(f64::MIN, f64::max) - xs.iter().cloned().fold(f64::MAX, f64::min);
        let h = ys.iter().cloned().fold(f64::MIN, f64::max) - ys.iter().cloned().fold(f64::MAX, f64::min);
        if w < 1.0 || h < 1.0 {
            return Err(Step::Refused("not_visible".into()));
        }
        Ok((x, y))
    }

    async fn pointer_target(&self, session: &str, node: i64) -> Result<(f64, f64), Step> {
        let disabled = self.on_node(session, node, "function(){return !!(this.disabled||this.getAttribute&&this.getAttribute('aria-disabled')==='true')}", vec![]).await.unwrap_or(Value::Bool(false));
        if disabled == true {
            return Err(Step::Refused("disabled".into()));
        }
        let (x, y) = self.center(session, node).await?;
        let hit = self
            .on_node(session, node, "function(x,y){const h=document.elementFromPoint(x,y);if(!h)return 'none';return (h===this||this.contains(h)||h.contains(this)||(this.labels&&[...this.labels].some(l=>l.contains(h))))?'ok':'covered'}", vec![json!(x), json!(y)])
            .await
            .unwrap_or(Value::Null);
        match hit.as_str() {
            Some("covered") => Err(Step::Refused("covered".into())),
            Some("none") => Err(Step::Refused("not_visible".into())),
            _ => Ok((x, y)),
        }
    }

    async fn mouse(&self, session: &str, kind: &str, x: f64, y: f64, button: &str, count: i64) -> Result<(), String> {
        self.cdp.call(Some(session), "Input.dispatchMouseEvent", json!({"type": kind, "x": x, "y": y, "button": button, "clickCount": count})).await.map(|_| ())
    }

    async fn click_at(&self, session: &str, x: f64, y: f64, button: &str, count: i64) -> Result<(), String> {
        self.mouse(session, "mouseMoved", x, y, "none", 0).await?;
        for c in 1..=count {
            self.mouse(session, "mousePressed", x, y, button, c).await?;
            self.mouse(session, "mouseReleased", x, y, button, c).await?;
        }
        Ok(())
    }

    async fn key(&self, session: &str, key: &str, modifiers: i64) -> Result<(), String> {
        let (code, vk, text): (String, i64, Option<String>) = match key {
            "Enter" => ("Enter".into(), 13, Some("\r".into())),
            "Tab" => ("Tab".into(), 9, None),
            "Escape" => ("Escape".into(), 27, None),
            "Backspace" => ("Backspace".into(), 8, None),
            "Delete" => ("Delete".into(), 46, None),
            "Space" => ("Space".into(), 32, Some(" ".into())),
            "ArrowUp" => ("ArrowUp".into(), 38, None),
            "ArrowDown" => ("ArrowDown".into(), 40, None),
            "ArrowLeft" => ("ArrowLeft".into(), 37, None),
            "ArrowRight" => ("ArrowRight".into(), 39, None),
            "Home" => ("Home".into(), 36, None),
            "End" => ("End".into(), 35, None),
            "PageUp" => ("PageUp".into(), 33, None),
            "PageDown" => ("PageDown".into(), 34, None),
            c if c.chars().count() == 1 => {
                let ch = c.chars().next().unwrap();
                let up = ch.to_ascii_uppercase();
                let code = if up.is_ascii_alphabetic() { format!("Key{up}") } else if ch.is_ascii_digit() { format!("Digit{ch}") } else { String::new() };
                (code, up as i64, if modifiers & 0b0110 == 0 { Some(c.to_string()) } else { None })
            }
            other => return Err(format!("unsupported key `{other}`")),
        };
        let key_name = if key == "Space" { " " } else { key };
        let mut down = json!({"type": if text.is_some() { "keyDown" } else { "rawKeyDown" }, "key": key_name, "code": code, "windowsVirtualKeyCode": vk, "modifiers": modifiers});
        if let Some(t) = &text {
            down["text"] = json!(t);
        }
        self.cdp.call(Some(session), "Input.dispatchKeyEvent", down).await?;
        self.cdp.call(Some(session), "Input.dispatchKeyEvent", json!({"type": "keyUp", "key": key_name, "code": code, "windowsVirtualKeyCode": vk, "modifiers": modifiers})).await?;
        Ok(())
    }

    async fn step(&self, st: &State, tab: &str, session: &str, a: &Value) -> Step {
        let kind = s(a, "kind");
        let node = |st: &State| -> Result<Option<i64>, Step> {
            match a.get("ref").and_then(Value::as_str) {
                Some(r) => Self::resolve(st, tab, r).map(Some).map_err(Step::Refused),
                None => Ok(None),
            }
        };
        let need_ref = |n: Option<i64>| n.ok_or(Step::Refused("needs_reference".into()));
        let run = async {
            match kind.as_str() {
                "navigate" => {
                    let url = s(a, "url");
                    let r = self.cdp.call(Some(session), "Page.navigate", json!({"url": url})).await.map_err(Step::Unknown)?;
                    if let Some(e) = r.get("errorText").and_then(Value::as_str) {
                        return Ok(vec![format!("the page did not load: {e}")]);
                    }
                    Ok(vec![])
                }
                "back" | "forward" => {
                    let h = self.cdp.call(Some(session), "Page.getNavigationHistory", json!({})).await.map_err(Step::Unknown)?;
                    let i = h["currentIndex"].as_i64().unwrap_or(0) + if kind == "back" { -1 } else { 1 };
                    let entry = h["entries"].as_array().and_then(|e| e.get(i.max(0) as usize).filter(|_| i >= 0)).cloned().ok_or(Step::Refused("no page to go to".into()))?;
                    self.cdp.call(Some(session), "Page.navigateToHistoryEntry", json!({"entryId": entry["id"]})).await.map_err(Step::Unknown)?;
                    Ok(vec![])
                }
                "reload" => {
                    self.cdp.call(Some(session), "Page.reload", json!({})).await.map_err(Step::Unknown)?;
                    Ok(vec![])
                }
                "click" | "hover" => {
                    let n = need_ref(node(st)?)?;
                    let (x, y) = self.pointer_target(session, n).await?;
                    if kind == "hover" {
                        self.mouse(session, "mouseMoved", x, y, "none", 0).await.map_err(Step::Unknown)?;
                    } else {
                        let button = a.get("button").and_then(Value::as_str).unwrap_or("left");
                        let count = a.get("count").and_then(Value::as_i64).unwrap_or(1);
                        self.click_at(session, x, y, button, count).await.map_err(Step::Unknown)?;
                    }
                    Ok(vec![])
                }
                "fill" => {
                    let n = need_ref(node(st)?)?;
                    let text = s(a, "text");
                    let append = s(a, "mode") == "append";
                    let disabled = self.on_node(session, n, "function(){return !!(this.disabled||this.readOnly)}", vec![]).await.unwrap_or(Value::Bool(false));
                    if disabled == true {
                        return Err(Step::Refused("disabled".into()));
                    }
                    self.center(session, n).await?;
                    self.on_node(
                        session,
                        n,
                        "function(append){this.focus();if(append){if(this.setSelectionRange&&'value' in this){const l=this.value.length;try{this.setSelectionRange(l,l)}catch(e){}}}else{if(this.select){this.select()}else{document.execCommand('selectAll')}}}",
                        vec![json!(append)],
                    )
                    .await
                    .map_err(Step::Unknown)?;
                    if !append && text.is_empty() {
                        self.key(session, "Delete", 0).await.map_err(Step::Unknown)?;
                    }
                    if s(a, "entry") == "typing" {
                        for ch in text.chars() {
                            let c = ch.to_string();
                            self.cdp.call(Some(session), "Input.dispatchKeyEvent", json!({"type": "keyDown", "key": c, "text": c})).await.map_err(Step::Unknown)?;
                            self.cdp.call(Some(session), "Input.dispatchKeyEvent", json!({"type": "keyUp", "key": c})).await.map_err(Step::Unknown)?;
                        }
                    } else if !text.is_empty() {
                        self.cdp.call(Some(session), "Input.insertText", json!({"text": text})).await.map_err(Step::Unknown)?;
                    }
                    let shown = self.on_node(session, n, "function(){return 'value' in this?String(this.value):(this.innerText||'')}", vec![]).await.unwrap_or(Value::Null);
                    let shown = shown.as_str().unwrap_or_default();
                    let mut facts = vec![];
                    if !shown.contains(&text) {
                        let squash = |x: &str| x.chars().filter(|c| c.is_alphanumeric()).collect::<String>();
                        facts.push(if squash(shown).contains(&squash(&text)) && !text.is_empty() { "the page reformatted the text".to_string() } else { "the field does not show the text that was typed".to_string() });
                    }
                    Ok(facts)
                }
                "set_checked" => {
                    let n = need_ref(node(st)?)?;
                    let want = a.get("checked").and_then(Value::as_bool).unwrap_or(true);
                    let cur = self.on_node(session, n, "function(){return this.checked===undefined?(this.getAttribute('aria-checked')==='true'):this.checked}", vec![]).await.map_err(Step::Unknown)?;
                    if cur == want {
                        return Ok(vec!["it already had that value".into()]);
                    }
                    let (x, y) = self.pointer_target(session, n).await?;
                    self.click_at(session, x, y, "left", 1).await.map_err(Step::Unknown)?;
                    Ok(vec![])
                }
                "select" => {
                    let n = need_ref(node(st)?)?;
                    let v = self
                        .on_node(
                            session,
                            n,
                            "function(v){if(this.disabled)return 'disabled';if(!this.options)return 'not_select';const o=[...this.options].find(o=>o.value===v)||[...this.options].find(o=>o.text.trim()===v);if(!o)return 'missing';if(this.value===o.value)return 'same';this.value=o.value;this.dispatchEvent(new Event('input',{bubbles:true}));this.dispatchEvent(new Event('change',{bubbles:true}));return 'ok'}",
                            vec![json!(s(a, "value"))],
                        )
                        .await
                        .map_err(Step::Unknown)?;
                    match v.as_str() {
                        Some("ok") => Ok(vec![]),
                        Some("same") => Ok(vec!["it already had that value".into()]),
                        Some("disabled") => Err(Step::Refused("disabled".into())),
                        Some("missing") => Err(Step::Refused("the list has no such option".into())),
                        _ => Err(Step::Refused("needs_reference".into())),
                    }
                }
                "press" => {
                    let key = s(a, "key");
                    let mods: Vec<String> = a.get("modifiers").and_then(Value::as_array).into_iter().flatten().filter_map(|m| m.as_str().map(String::from)).collect();
                    let bits = mods.iter().map(|m| match m.as_str() {
                        "Alt" => 1,
                        "Control" => 2,
                        "Meta" => 4,
                        "Shift" => 8,
                        _ => 0,
                    }).fold(0, |a, b| a | b);
                    let up = key.to_uppercase();
                    if (bits & 0b0110 != 0 && ["C", "V", "X"].contains(&up.as_str())) || (bits & 8 != 0 && key == "Delete") {
                        return Err(Step::Refused("that key is unavailable".into()));
                    }
                    if let Some(n) = node(st)? {
                        self.on_node(session, n, "function(){this.focus()}", vec![]).await.map_err(Step::Unknown)?;
                    }
                    self.key(session, &key, bits).await.map_err(Step::Refused)?;
                    Ok(vec![])
                }
                "scroll" => {
                    let (x, y) = match node(st)? {
                        Some(n) => self.center(session, n).await?,
                        None => {
                            let v = self.eval(session, "[innerWidth/2, innerHeight/2]").await.map_err(Step::Unknown)?;
                            (v[0].as_f64().unwrap_or(400.0), v[1].as_f64().unwrap_or(300.0))
                        }
                    };
                    let h = self.eval(session, "innerHeight").await.ok().and_then(|v| v.as_f64()).unwrap_or(800.0);
                    let pages = a.get("pages").and_then(Value::as_f64).unwrap_or(1.0);
                    let d = h * pages;
                    let (dx, dy) = match s(a, "direction").as_str() {
                        "up" => (0.0, -d),
                        "left" => (-d, 0.0),
                        "right" => (d, 0.0),
                        _ => (0.0, d),
                    };
                    self.cdp.call(Some(session), "Input.dispatchMouseEvent", json!({"type": "mouseWheel", "x": x, "y": y, "deltaX": dx, "deltaY": dy})).await.map_err(Step::Unknown)?;
                    Ok(vec![])
                }
                "wait" => {
                    let visible = s(a, "condition") != "hidden";
                    let timeout = a.get("timeout_ms").and_then(Value::as_u64).unwrap_or(5000).min(60_000);
                    let target = node(st)?;
                    let text = a.get("text").and_then(Value::as_str).map(String::from);
                    let deadline = tokio::time::Instant::now() + Duration::from_millis(timeout);
                    loop {
                        let shown = match (target, &text) {
                            (Some(n), _) => self.on_node(session, n, "function(){const r=this.getBoundingClientRect();return this.isConnected&&r.width>0&&r.height>0}", vec![]).await.map(|v| v == true).unwrap_or(false),
                            (None, Some(t)) => self.eval(session, "document.body ? document.body.innerText : ''").await.map(|v| v.as_str().unwrap_or_default().contains(t.as_str())).unwrap_or(false),
                            (None, None) => return Err(Step::Refused("a wait needs ref or text".into())),
                        };
                        if shown == visible {
                            return Ok(vec![]);
                        }
                        if tokio::time::Instant::now() >= deadline {
                            return Err(Step::TimedOut("condition_not_met".into()));
                        }
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
                other => Err(Step::Refused(format!("unsupported action `{other}`"))),
            }
        };
        match run.await {
            Ok(facts) => Step::Applied(facts),
            Err(e) => e,
        }
    }

    /// Новые вкладки, открытые страницей, — в окно.
    async fn adopt_popups(&self, st: &mut State) -> Vec<String> {
        tokio::time::sleep(Duration::from_millis(200)).await;
        let pending: Vec<String> = std::mem::take(&mut *self.browser.popups.lock().unwrap());
        let mut ids = vec![];
        for target in pending {
            if self.browser.known.lock().unwrap().contains(&target) {
                continue;
            }
            if let Ok(id) = self.attach(st, &target).await {
                ids.push(id);
            }
        }
        ids
    }

    /// Последовательность действий на вкладке; результат — отчёт по шагам
    /// и новый вид.
    pub async fn act(&self, tab: &str, actions: &[Value]) -> Result<Vec<ToolResultPart>, String> {
        let mut st = self.state.lock().await;
        let session = Self::session_of(&st, tab)?;
        self.browser.popups.lock().unwrap().clear();
        let mut lines = vec![];
        let mut stopped = false;
        let mut unknown = false;
        let mut opened = vec![];
        for (i, a) in actions.iter().enumerate() {
            let label = match a.get("ref").and_then(Value::as_str) {
                Some(r) => format!("{} @{}", s(a, "kind"), r.trim_start_matches('@')),
                None => s(a, "kind"),
            };
            if stopped {
                lines.push(format!("Action {} ({label}): not started.", i + 1));
                continue;
            }
            let before = self.marker(&session).await;
            let step = self.step(&st, tab, &session, a).await;
            self.settle(&session).await;
            let after = self.marker(&session).await;
            let navigated = before.0 != after.0;
            let changed = navigated || before.1 != after.1;
            let page = if navigated { "the page navigated" } else if changed { "the page updated in place" } else { "the page did not navigate" };
            let new_tabs = self.adopt_popups(&mut st).await;
            match step {
                Step::Applied(mut facts) => {
                    facts.insert(0, page.into());
                    for t in &new_tabs {
                        facts.push(format!("it opened tab {t}"));
                    }
                    lines.push(format!("Action {} ({label}): applied; {}.", i + 1, facts.join("; ")));
                    // Ссылки следующих шагов взяты с прежнего вида.
                    if navigated || !new_tabs.is_empty() {
                        stopped = actions[i + 1..].iter().any(|n| n.get("ref").is_some());
                    }
                }
                Step::Refused(why) => {
                    lines.push(format!("Action {} ({label}): refused: {why}.", i + 1));
                    stopped = true;
                }
                Step::TimedOut(why) => {
                    lines.push(format!("Action {} ({label}): timed out: {why}.", i + 1));
                    stopped = true;
                }
                Step::Unknown(why) => {
                    lines.push(format!("Action {} ({label}): sent, outcome unknown: {why}.", i + 1));
                    stopped = true;
                    unknown = true;
                }
            }
            opened.extend(new_tabs);
        }
        let summary = if unknown {
            "The outcome of these actions is unknown. Read the tab to see what happened. Do not repeat them."
        } else if stopped {
            "Act on the tab's current view below."
        } else {
            "The action applied. Read the new view before you continue."
        };
        let mut parts = vec![ToolResultPart::Text { text: format!("{}\n{summary}", lines.join("\n")) }];
        match self.view_locked(&mut st, tab, "auto", None, None).await {
            Ok(v) => parts.extend(v.parts()),
            Err(_) => parts.push(ToolResultPart::Text { text: "The tab's new view did not come back. Observe the tab before you continue.".into() }),
        }
        for t in opened {
            if let Ok(v) = self.view_locked(&mut st, &t, "auto", None, None).await {
                parts.extend(v.parts());
            }
        }
        Ok(parts)
    }

    async fn tab_list(&self, st: &State) -> String {
        if st.tabs.is_empty() {
            return "## Tabs in this window\nThis window has no open tabs.".into();
        }
        let mut out = vec!["## Tabs in this window".to_string()];
        for t in &st.tabs {
            let info = self.eval(&t.session, "[document.title, location.href]").await.unwrap_or(Value::Null);
            let sel = if st.selected.as_deref() == Some(&t.id) { " (selected)" } else { "" };
            out.push(format!("- {}{sel}: Title: {} URL: {}", t.id, info[0].as_str().unwrap_or_default(), info[1].as_str().unwrap_or_default()));
        }
        out.join("\n")
    }

    /// `browser.tabs`: list, open, select, close.
    pub async fn tabs(&self, kind: &str, url: Option<&str>, tab: Option<&str>, r: Option<&str>) -> Result<Vec<ToolResultPart>, String> {
        let mut st = self.state.lock().await;
        match kind {
            "list" => Ok(vec![ToolResultPart::Text { text: self.tab_list(&st).await }]),
            "open" => {
                let url = match (url, tab, r) {
                    (Some(u), _, _) => u.to_string(),
                    (None, Some(t), Some(r)) => {
                        let node = Self::resolve(&st, t, r)?;
                        let session = Self::session_of(&st, t)?;
                        let href = self.on_node(&session, node, "function(){return this.href||''}", vec![]).await?;
                        href.as_str().filter(|h| !h.is_empty()).map(String::from).ok_or_else(|| format!("@{} is not a link.", r.trim_start_matches('@')))?
                    }
                    _ => "about:blank".into(),
                };
                let target = s(&self.cdp.call(None, "Target.createTarget", json!({"url": url})).await?, "targetId");
                let id = self.attach(&mut st, &target).await?;
                self.browser.popups.lock().unwrap().retain(|p| *p != target);
                st.selected = Some(id.clone());
                let session = Self::session_of(&st, &id)?;
                self.settle(&session).await;
                let mut parts = vec![ToolResultPart::Text { text: format!("Opened tab {id}. It loaded the requested URL.") }];
                parts.extend(self.view_locked(&mut st, &id, "auto", None, None).await?.parts());
                Ok(parts)
            }
            "select" => {
                let t = tab.ok_or("select needs tab")?;
                let target = st.tabs.iter().find(|x| x.id == t).map(|x| x.target.clone()).ok_or_else(|| format!("{t} is not open in this window. Report it in browser.report."))?;
                self.cdp.call(None, "Target.activateTarget", json!({"targetId": target})).await?;
                st.selected = Some(t.into());
                let mut parts = vec![];
                parts.extend(self.view_locked(&mut st, t, "auto", None, None).await?.parts());
                Ok(parts)
            }
            "close" => {
                let t = tab.ok_or("close needs tab")?;
                let i = st.tabs.iter().position(|x| x.id == t).ok_or_else(|| format!("{t} is not open in this window. Report it in browser.report."))?;
                let target = st.tabs[i].target.clone();
                self.cdp.call(None, "Target.closeTarget", json!({"targetId": target})).await?;
                st.tabs.remove(i);
                if st.selected.as_deref() == Some(t) {
                    st.selected = st.tabs.last().map(|x| x.id.clone());
                }
                Ok(vec![ToolResultPart::Text { text: format!("Closed tab {t}.\n\n{}", self.tab_list(&st).await) }])
            }
            other => Err(format!("unknown tabs kind `{other}`")),
        }
    }
}
