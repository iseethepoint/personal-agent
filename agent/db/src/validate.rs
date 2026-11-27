//! Проверка запроса `hamlet.db` — как у muse (`hatch-agent/src/tools/db.rs`,
//! тексты ошибок — ядро около `0x18a9ce1`, `0x3134c1`, `0x398a37`,
//! `0x32b1e2`): один SELECT по PostgreSqlDialect, только таблицы каталога и
//! объявленные CTE с приставкой `hatch_cte_`, только функции и приведения из
//! `query_capabilities`, без блокировок, настроек, рекурсии, табличных
//! функций и модификаторов. Исполняется нормализованный текст, и только если
//! он разбирается обратно в то же дерево.

use std::collections::HashSet;
use std::ops::ControlFlow;

use sqlparser::ast::{Expr, ObjectName, ObjectNamePart, Query, Select, SetExpr, Statement, TableFactor, Visit, Visitor};
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

use crate::catalog::Catalog;

/// Предел длины запроса — 16 KiB (`cmp $0x4000` около `0x1f4f1227`).
pub const MAX_SQL_BYTES: usize = 16384;

pub const EMPTY: &str = "SQL statement is empty";
pub const DATA_CHANGING: &str = "data-changing statements are not allowed inside hamlet.db";
pub const ONE_SELECT: &str = "hamlet.db accepts exactly one SELECT statement";
pub const DYNAMIC_FUNCTION: &str = "dynamic function names are not allowed";
pub const COLLATION: &str = "custom collations are not allowed";
pub const CHARSET: &str = "character-set conversion is not allowed";
pub const CLAUSES: &str = "locking, settings, output-format, and pipe clauses are not allowed";
pub const RECURSIVE: &str = "recursive CTEs are not allowed";
pub const CTE_PREFIX: &str = "CTE names must start with `hatch_cte_`";
pub const SELECT_INTO: &str = "SELECT INTO is not allowed";
pub const DYNAMIC_RELATION: &str = "dynamic relation names are not allowed";
pub const TABLE_MODIFIERS: &str = "table functions, lateral relations, sampling, and table modifiers are not allowed";
pub const ROUND_TRIP: &str = "hamlet.db rejected SQL whose normalized form does not round-trip safely";

pub fn too_long() -> String {
    format!("SQL statement exceeds the {MAX_SQL_BYTES}-byte limit")
}
pub fn outside_catalog(r: &str) -> String {
    format!("relation `{r}` is outside the `hamlet.db` table catalog")
}
pub fn unqualified(r: &str) -> String {
    format!("relation `{r}` must be a declared CTE or a schema-qualified table from the `hamlet.db` schema guide")
}
pub fn unsafe_function(f: &str) -> String {
    format!("function `{f}` is outside the `hamlet.db` safe function set")
}
pub fn unsafe_cast(t: &str) -> String {
    format!("cast type `{t}` is outside the `hamlet.db` safe type set")
}
pub fn invalid(e: impl std::fmt::Display) -> String {
    format!("invalid PostgreSQL SELECT: {e}")
}

/// Проверить запрос; вернуть нормализованный текст для исполнения.
pub fn validate(sql: &str, catalog: &Catalog) -> Result<String, String> {
    if sql.len() > MAX_SQL_BYTES {
        return Err(too_long());
    }
    if sql.trim().is_empty() {
        return Err(EMPTY.into());
    }
    let dialect = PostgreSqlDialect {};
    let statements = Parser::parse_sql(&dialect, sql).map_err(invalid)?;
    if statements.iter().any(changes_data) {
        return Err(DATA_CHANGING.into());
    }
    let [stmt] = statements.as_slice() else { return Err(ONE_SELECT.into()) };
    if !matches!(stmt, Statement::Query(_)) {
        return Err(ONE_SELECT.into());
    }
    let mut check = Check { catalog, ctes: HashSet::new() };
    if let ControlFlow::Break(e) = stmt.visit(&mut check) {
        return Err(e);
    }
    // Таблица с `redaction` читается через свой вид.
    let mut stmt = stmt.clone();
    let _ = sqlparser::ast::visit_relations_mut(&mut stmt, |rel| {
        if let [ObjectNamePart::Identifier(s), ObjectNamePart::Identifier(t)] = rel.0.as_slice() {
            if let Some(view) = catalog.view_for(&s.value.to_lowercase(), &t.value.to_lowercase()) {
                let (vs, vt) = view.split_once('.').unwrap_or(("", view));
                rel.0 = vec![ObjectNamePart::Identifier(vs.into()), ObjectNamePart::Identifier(vt.into())];
            }
        }
        ControlFlow::<()>::Continue(())
    });
    let normalized = stmt.to_string();
    match Parser::parse_sql(&dialect, &normalized) {
        Ok(again) if again.len() == 1 && again[0] == stmt => Ok(normalized),
        _ => Err(ROUND_TRIP.into()),
    }
}

fn changes_data(s: &Statement) -> bool {
    match s {
        Statement::Query(q) => body_changes_data(&q.body),
        Statement::Insert(_)
        | Statement::Update(_)
        | Statement::Delete(_)
        | Statement::Merge(_)
        | Statement::Truncate(_)
        | Statement::Copy { .. }
        | Statement::CreateTable(_)
        | Statement::CreateView(_)
        | Statement::CreateIndex(_)
        | Statement::CreateSchema { .. }
        | Statement::CreateFunction(_)
        | Statement::AlterTable(_)
        | Statement::Drop { .. }
        | Statement::Grant(_)
        | Statement::Revoke(_) => true,
        _ => false,
    }
}

fn body_changes_data(b: &SetExpr) -> bool {
    match b {
        SetExpr::Insert(_) | SetExpr::Update(_) | SetExpr::Delete(_) | SetExpr::Merge(_) => true,
        SetExpr::Query(q) => body_changes_data(&q.body),
        SetExpr::SetOperation { left, right, .. } => body_changes_data(left) || body_changes_data(right),
        _ => false,
    }
}

struct Check<'c> {
    catalog: &'c Catalog,
    ctes: HashSet<String>,
}

fn ident(p: &ObjectNamePart) -> Option<String> {
    match p {
        ObjectNamePart::Identifier(i) => Some(i.value.to_lowercase()),
        ObjectNamePart::Function(_) => None,
    }
}

/// Имя типа без размеров: `VARCHAR(10)` → `varchar`.
fn type_name(t: &sqlparser::ast::DataType) -> String {
    let s = t.to_string().to_lowercase();
    match s.find('(') {
        Some(i) if s.ends_with(')') => s[..i].trim_end().to_string(),
        _ => s,
    }
}

impl Check<'_> {
    fn cast(&self, t: &sqlparser::ast::DataType) -> ControlFlow<String> {
        let name = type_name(t);
        if self.catalog.query_capabilities.cast_types.contains(&name) {
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(unsafe_cast(&name))
        }
    }
}

impl Visitor for Check<'_> {
    type Break = String;

    fn pre_visit_query(&mut self, q: &Query) -> ControlFlow<String> {
        if !q.locks.is_empty() || q.for_clause.is_some() || q.settings.is_some() || q.format_clause.is_some() || !q.pipe_operators.is_empty() {
            return ControlFlow::Break(CLAUSES.into());
        }
        if body_changes_data(&q.body) {
            return ControlFlow::Break(DATA_CHANGING.into());
        }
        if let Some(w) = &q.with {
            if w.recursive {
                return ControlFlow::Break(RECURSIVE.into());
            }
            for c in &w.cte_tables {
                let name = c.alias.name.value.to_lowercase();
                if !name.starts_with("hatch_cte_") {
                    return ControlFlow::Break(CTE_PREFIX.into());
                }
                self.ctes.insert(name);
            }
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_select(&mut self, s: &Select) -> ControlFlow<String> {
        if s.into.is_some() {
            return ControlFlow::Break(SELECT_INTO.into());
        }
        if !s.lateral_views.is_empty() {
            return ControlFlow::Break(TABLE_MODIFIERS.into());
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_table_factor(&mut self, t: &TableFactor) -> ControlFlow<String> {
        let plain = match t {
            TableFactor::Table { args, with_hints, version, with_ordinality, partitions, json_path, sample, index_hints, .. } => {
                args.is_none() && with_hints.is_empty() && version.is_none() && !with_ordinality && partitions.is_empty() && json_path.is_none() && sample.is_none() && index_hints.is_empty()
            }
            TableFactor::Derived { lateral, sample, .. } => !lateral && sample.is_none(),
            TableFactor::NestedJoin { .. } => true,
            _ => false,
        };
        if plain {
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(TABLE_MODIFIERS.into())
        }
    }

    fn pre_visit_relation(&mut self, r: &ObjectName) -> ControlFlow<String> {
        let Some(parts) = r.0.iter().map(ident).collect::<Option<Vec<_>>>() else {
            return ControlFlow::Break(DYNAMIC_RELATION.into());
        };
        match parts.as_slice() {
            [one] if self.ctes.contains(one) => ControlFlow::Continue(()),
            [_] => ControlFlow::Break(unqualified(&r.to_string())),
            [schema, table] if self.catalog.has_table(schema, table) => ControlFlow::Continue(()),
            _ => ControlFlow::Break(outside_catalog(&r.to_string())),
        }
    }

    fn pre_visit_expr(&mut self, e: &Expr) -> ControlFlow<String> {
        match e {
            Expr::Function(f) => {
                let Some(parts) = f.name.0.iter().map(ident).collect::<Option<Vec<_>>>() else {
                    return ControlFlow::Break(DYNAMIC_FUNCTION.into());
                };
                match parts.as_slice() {
                    [name] if self.catalog.query_capabilities.functions.iter().any(|x| x == name) => ControlFlow::Continue(()),
                    _ => ControlFlow::Break(unsafe_function(&f.name.to_string())),
                }
            }
            Expr::Cast { data_type, .. } => self.cast(data_type),
            Expr::TypedString(t) => self.cast(&t.data_type),
            Expr::Collate { .. } => ControlFlow::Break(COLLATION.into()),
            Expr::Convert { .. } => ControlFlow::Break(CHARSET.into()),
            Expr::Overlay { .. } => ControlFlow::Break(unsafe_function("overlay")),
            _ => ControlFlow::Continue(()),
        }
    }
}
