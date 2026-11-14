//! Typed recursive-CTE traversals over adjacency-list tables.
//!
//! A [`Hierarchy`] describes a table with an id column and a nullable parent column. Its
//! [`descendants`](Hierarchy::descendants) and [`ancestors`](Hierarchy::ancestors) methods build a
//! `WITH RECURSIVE` query through [`diesel_cte_ext`] that loads `(id, parent_id, depth)` rows:
//!
//! ```ignore
//! use arbor_diesel::sql::Hierarchy;
//! use diesel::sql_types::Text;
//!
//! const CATEGORIES: Hierarchy = Hierarchy::new("category_nodes", "id", "parent_id")
//!     .scoped("domain", "catalog")
//!     .live_when_null("deleted_at");
//!
//! let rows: Vec<(String, Option<String>, i32)> = CATEGORIES
//!     .descendants::<diesel::sqlite::Sqlite, Text, _>("electronics".to_string())
//!     .load(&mut conn)?;
//! ```
//!
//! Safety properties:
//! * Table and column names are `&'static str` constants and are always emitted through the
//!   backend's identifier quoting; values (the start id, discriminator value, and scope key) are
//!   bound parameters.
//! * Every query stops at `max_depth` (64 by default), so cyclic data cannot recurse forever.
//!   A row at depth `max_depth` means the traversal was truncated; see [`reached_depth_limit`].
//! * Fragments opt out of Diesel's type-keyed prepared-statement cache, because the SQL depends on
//!   the runtime [`Hierarchy`], which the Rust type does not capture.

use std::marker::PhantomData;

use diesel::QueryResult;
use diesel::backend::Backend;
use diesel::query_builder::{AstPass, Query, QueryFragment, QueryId};
use diesel::serialize::ToSql;
use diesel::sql_types::{HasSqlType, Integer, Nullable, SingleValue, SqlType, Text};
use diesel_cte_ext::builders::with_recursive;
use diesel_cte_ext::cte::WithRecursive;
use diesel_cte_ext::{RecursiveBackend, RecursiveParts};

/// Name of the recursive CTE and of its columns.
const CTE: &str = "arbor_tree";
const CTE_COLUMNS: &[&str] = &["node_id", "parent_id", "depth"];

/// Default recursion limit.
pub const DEFAULT_MAX_DEPTH: i32 = 64;

/// Restricts a hierarchy to rows whose `column` equals `value`, e.g. one domain of a shared
/// `category_nodes` table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scope {
    /// Discriminator column.
    pub column: &'static str,
    /// Required value, bound as a `Text` parameter.
    pub value: &'static str,
}

/// Scope-key visibility restriction: `(col IS NULL OR col = $scope_key)` when `scope_key` is
/// `Some`, or `col IS NULL` (shared/global rows only) when `scope_key` is `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScopeKey {
    /// Scope-key column.
    pub column: &'static str,
    /// Optional scope key, bound as a `Text` parameter when present.
    pub scope_key: Option<&'static str>,
}

/// An adjacency-list hierarchy stored in one table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hierarchy {
    /// Table name.
    pub table: &'static str,
    /// Primary-key column.
    pub id: &'static str,
    /// Nullable parent column referencing `id`.
    pub parent: &'static str,
    /// Optional discriminator filter applied at every level.
    pub scope: Option<Scope>,
    /// Optional soft-delete column; only rows where it IS NULL are traversed.
    pub live_column: Option<&'static str>,
    /// Optional active flag column; only rows where it IS TRUE are traversed.
    pub active_column: Option<&'static str>,
    /// Optional scope-key visibility filter.
    pub scope_key: Option<ScopeKey>,
    /// Maximum depth (the start row has depth 0).
    pub max_depth: i32,
    /// Maximum breadth per parent node.
    pub max_breadth: Option<i32>,
}

/// Which way a traversal walks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// From a node down to its whole subtree.
    Descendants,
    /// From a node up to its root.
    Ancestors,
}

/// Helper trait for backends supporting recursive `Hierarchy` traversals over key `(ST, K)`.
pub trait TraversalBackend<ST, K>: RecursiveBackend + Sized {
    /// Construct the typed `WITH RECURSIVE` query for `hierarchy` in `direction`.
    fn build_traversal(
        hierarchy: Hierarchy,
        start: K,
        direction: Direction,
    ) -> Traversal<Self, ST, K>;
}

impl<DB, ST, K> TraversalBackend<ST, K> for DB
where
    DB: RecursiveBackend,
    Seed<ST, K>: QueryFragment<DB>,
    Step<ST>: QueryFragment<DB>,
    Body<ST>: QueryFragment<DB>,
{
    fn build_traversal(
        hierarchy: Hierarchy,
        start: K,
        direction: Direction,
    ) -> Traversal<Self, ST, K> {
        with_recursive::<Self, (), _, _, _, _>(
            CTE,
            CTE_COLUMNS,
            RecursiveParts::new(
                Seed {
                    hierarchy,
                    start,
                    _marker: PhantomData,
                },
                Step {
                    hierarchy,
                    direction,
                    _marker: PhantomData,
                },
                Body {
                    _marker: PhantomData,
                },
            ),
        )
    }
}

impl Hierarchy {
    /// A hierarchy over `table` with the given id and parent columns.
    pub const fn new(table: &'static str, id: &'static str, parent: &'static str) -> Self {
        Self {
            table,
            id,
            parent,
            scope: None,
            live_column: None,
            active_column: None,
            scope_key: None,
            max_depth: DEFAULT_MAX_DEPTH,
            max_breadth: None,
        }
    }

    /// Only traverse rows whose `column` equals `value`.
    pub const fn scoped(mut self, column: &'static str, value: &'static str) -> Self {
        self.scope = Some(Scope { column, value });
        self
    }

    /// Only traverse rows where `column` IS NULL (soft-deleted rows are skipped).
    pub const fn live_when_null(mut self, column: &'static str) -> Self {
        self.live_column = Some(column);
        self
    }

    /// Only traverse rows where `column` IS TRUE.
    pub const fn active_when_true(mut self, column: &'static str) -> Self {
        self.active_column = Some(column);
        self
    }

    /// Traverse global/shared rows (`column IS NULL`) and rows matching `scope_key`.
    pub const fn scope_key_or_global(
        mut self,
        column: &'static str,
        scope_key: Option<&'static str>,
    ) -> Self {
        self.scope_key = Some(ScopeKey { column, scope_key });
        self
    }

    /// Override the recursion limit.
    pub const fn max_depth(mut self, max_depth: i32) -> Self {
        self.max_depth = max_depth;
        self
    }

    /// Override the breadth degree limit per node.
    pub const fn max_breadth(mut self, max_breadth: i32) -> Self {
        self.max_breadth = Some(max_breadth);
        self
    }

    /// `start` and its whole subtree as `(id, parent_id, depth)` rows ordered by depth.
    pub fn descendants<DB: TraversalBackend<ST, K>, ST, K>(
        &self,
        start: K,
    ) -> Traversal<DB, ST, K> {
        self.traverse(start, Direction::Descendants)
    }

    /// `start` and every ancestor up to its root as `(id, parent_id, depth)` rows ordered by depth.
    pub fn ancestors<DB: TraversalBackend<ST, K>, ST, K>(&self, start: K) -> Traversal<DB, ST, K> {
        self.traverse(start, Direction::Ancestors)
    }

    /// Traverse in the given direction.
    pub fn traverse<DB: TraversalBackend<ST, K>, ST, K>(
        &self,
        start: K,
        direction: Direction,
    ) -> Traversal<DB, ST, K> {
        DB::build_traversal(*self, start, direction)
    }
}

/// The query returned by [`Hierarchy::descendants`] and [`Hierarchy::ancestors`]; load it as
/// `Vec<(K, Option<K>, i32)>`.
pub type Traversal<DB, ST, K> = WithRecursive<DB, (), Seed<ST, K>, Step<ST>, Body<ST>>;

/// `true` if any row reached the hierarchy's `max_depth`, meaning the traversal may be truncated
/// (a very deep tree, or a cycle in the stored data).
pub fn reached_depth_limit<K>(hierarchy: &Hierarchy, rows: &[(K, Option<K>, i32)]) -> bool {
    rows.iter()
        .any(|(_, _, depth)| *depth >= hierarchy.max_depth)
}

/// `true` if making `new_parent` the parent of `node` would create a cycle, given the
/// rows of `hierarchy.ancestors(new_parent)` (which include `new_parent` itself).
pub fn creates_cycle<K: PartialEq>(
    node: &K,
    ancestors_of_new_parent: &[(K, Option<K>, i32)],
) -> bool {
    ancestors_of_new_parent.iter().any(|(id, _, _)| id == node)
}

fn push_filters<'b, DB>(
    hierarchy: &'b Hierarchy,
    alias: &str,
    out: &mut AstPass<'_, 'b, DB>,
) -> QueryResult<()>
where
    DB: Backend + HasSqlType<Text>,
    str: ToSql<Text, DB>,
{
    push_scope_filter(&hierarchy.scope, alias, out)?;
    push_flag_filters(hierarchy.live_column, hierarchy.active_column, alias, out)?;
    push_scope_key_filter(&hierarchy.scope_key, alias, out)
}

fn push_scope_filter<'b, DB>(
    scope: &'b Option<Scope>,
    alias: &str,
    out: &mut AstPass<'_, 'b, DB>,
) -> QueryResult<()>
where
    DB: Backend + HasSqlType<Text>,
    str: ToSql<Text, DB>,
{
    if let Some(s) = scope {
        out.push_sql(" AND ");
        out.push_sql(alias);
        out.push_identifier(s.column)?;
        out.push_sql(" = ");
        out.push_bind_param::<Text, str>(s.value)?;
    }
    Ok(())
}

fn push_flag_filters<'b, DB>(
    live_column: Option<&'static str>,
    active_column: Option<&'static str>,
    alias: &str,
    out: &mut AstPass<'_, 'b, DB>,
) -> QueryResult<()>
where
    DB: Backend,
{
    if let Some(column) = live_column {
        out.push_sql(" AND ");
        out.push_sql(alias);
        out.push_identifier(column)?;
        out.push_sql(" IS NULL");
    }
    if let Some(column) = active_column {
        out.push_sql(" AND ");
        out.push_sql(alias);
        out.push_identifier(column)?;
        out.push_sql(" IS TRUE");
    }
    Ok(())
}

fn push_scope_key_filter<'b, DB>(
    scope_key: &'b Option<ScopeKey>,
    alias: &str,
    out: &mut AstPass<'_, 'b, DB>,
) -> QueryResult<()>
where
    DB: Backend + HasSqlType<Text>,
    str: ToSql<Text, DB>,
{
    let Some(key_filter) = scope_key else {
        return Ok(());
    };
    out.push_sql(" AND ");
    match key_filter.scope_key {
        Some(key) => {
            out.push_sql("(");
            out.push_sql(alias);
            out.push_identifier(key_filter.column)?;
            out.push_sql(" IS NULL OR ");
            out.push_sql(alias);
            out.push_identifier(key_filter.column)?;
            out.push_sql(" = ");
            out.push_bind_param::<Text, str>(key)?;
            out.push_sql(")");
        }
        None => {
            out.push_sql(alias);
            out.push_identifier(key_filter.column)?;
            out.push_sql(" IS NULL");
        }
    }
    Ok(())
}

/// Seed: `SELECT id, parent, 0 FROM table WHERE id = $start [filters]`.
#[derive(Debug, Clone)]
pub struct Seed<ST, K> {
    hierarchy: Hierarchy,
    start: K,
    _marker: PhantomData<ST>,
}

impl<ST, K> QueryId for Seed<ST, K> {
    type QueryId = ();
    const HAS_STATIC_QUERY_ID: bool = false;
}

impl<DB, ST, K> QueryFragment<DB> for Seed<ST, K>
where
    DB: Backend + HasSqlType<ST> + HasSqlType<Text>,
    ST: SqlType,
    K: ToSql<ST, DB>,
    str: ToSql<Text, DB>,
{
    fn walk_ast<'b>(&'b self, mut out: AstPass<'_, 'b, DB>) -> QueryResult<()> {
        out.unsafe_to_cache_prepared();
        let h = &self.hierarchy;
        out.push_sql("SELECT ");
        out.push_identifier(h.id)?;
        out.push_sql(", ");
        out.push_identifier(h.parent)?;
        out.push_sql(", 0 FROM ");
        out.push_identifier(h.table)?;
        out.push_sql(" WHERE ");
        out.push_identifier(h.id)?;
        out.push_sql(" = ");
        out.push_bind_param::<ST, K>(&self.start)?;
        push_filters(h, "", &mut out)
    }
}

/// Recursive step joining the table to the CTE in the chosen direction, bounded by `max_depth`.
#[derive(Debug, Clone)]
pub struct Step<ST> {
    hierarchy: Hierarchy,
    direction: Direction,
    _marker: PhantomData<ST>,
}

impl<ST> QueryId for Step<ST> {
    type QueryId = ();
    const HAS_STATIC_QUERY_ID: bool = false;
}

impl<DB, ST> QueryFragment<DB> for Step<ST>
where
    DB: Backend + HasSqlType<Text> + HasSqlType<Integer>,
    str: ToSql<Text, DB>,
    i32: ToSql<Integer, DB>,
{
    fn walk_ast<'b>(&'b self, mut out: AstPass<'_, 'b, DB>) -> QueryResult<()> {
        out.unsafe_to_cache_prepared();
        let h = &self.hierarchy;
        push_step_select(h, &mut out)?;
        push_step_from(h, &mut out)?;
        push_step_join(h, self.direction, &mut out)?;
        push_step_where(h, &mut out)
    }
}

fn push_step_select<'b, DB: Backend>(
    h: &'b Hierarchy,
    out: &mut AstPass<'_, 'b, DB>,
) -> QueryResult<()> {
    out.push_sql("SELECT t.");
    out.push_identifier(h.id)?;
    out.push_sql(", t.");
    out.push_identifier(h.parent)?;
    out.push_sql(", c.");
    out.push_identifier("depth")?;
    out.push_sql(" + 1 FROM ");
    Ok(())
}

fn push_step_from<'b, DB: Backend>(
    h: &'b Hierarchy,
    out: &mut AstPass<'_, 'b, DB>,
) -> QueryResult<()> {
    if h.max_breadth.is_some() {
        out.push_sql("(SELECT *, ROW_NUMBER() OVER (PARTITION BY ");
        out.push_identifier(h.parent)?;
        out.push_sql(" ORDER BY ");
        out.push_identifier(h.id)?;
        out.push_sql(") AS arbor_rn FROM ");
        out.push_identifier(h.table)?;
        out.push_sql(") t INNER JOIN ");
    } else {
        out.push_identifier(h.table)?;
        out.push_sql(" t INNER JOIN ");
    }
    Ok(())
}

fn push_step_join<'b, DB: Backend>(
    h: &'b Hierarchy,
    direction: Direction,
    out: &mut AstPass<'_, 'b, DB>,
) -> QueryResult<()> {
    out.push_identifier(CTE)?;
    out.push_sql(" c ON ");
    match direction {
        Direction::Descendants => {
            out.push_sql("t.");
            out.push_identifier(h.parent)?;
            out.push_sql(" = c.");
            out.push_identifier("node_id")
        }
        Direction::Ancestors => {
            out.push_sql("t.");
            out.push_identifier(h.id)?;
            out.push_sql(" = c.");
            out.push_identifier("parent_id")
        }
    }
}

fn push_step_where<'b, DB>(h: &'b Hierarchy, out: &mut AstPass<'_, 'b, DB>) -> QueryResult<()>
where
    DB: Backend + HasSqlType<Text> + HasSqlType<Integer>,
    str: ToSql<Text, DB>,
    i32: ToSql<Integer, DB>,
{
    out.push_sql(" WHERE c.");
    out.push_identifier("depth")?;
    out.push_sql(" < ");
    out.push_bind_param::<Integer, i32>(&h.max_depth)?;
    if let Some(breadth) = &h.max_breadth {
        out.push_sql(" AND t.arbor_rn <= ");
        out.push_bind_param::<Integer, i32>(breadth)?;
    }
    push_filters(h, "t.", out)
}

/// Body: `SELECT node_id, parent_id, depth FROM arbor_tree ORDER BY depth`.
#[derive(Debug, Clone)]
pub struct Body<ST> {
    _marker: PhantomData<ST>,
}

impl<ST> QueryId for Body<ST> {
    type QueryId = ();
    const HAS_STATIC_QUERY_ID: bool = false;
}

impl<ST> Query for Body<ST>
where
    ST: SqlType + SingleValue,
{
    type SqlType = (ST, Nullable<ST>, Integer);
}

impl<DB, ST> QueryFragment<DB> for Body<ST>
where
    DB: Backend,
{
    fn walk_ast<'b>(&'b self, mut out: AstPass<'_, 'b, DB>) -> QueryResult<()> {
        out.push_sql("SELECT ");
        out.push_identifier("node_id")?;
        out.push_sql(", ");
        out.push_identifier("parent_id")?;
        out.push_sql(", ");
        out.push_identifier("depth")?;
        out.push_sql(" FROM ");
        out.push_identifier(CTE)?;
        out.push_sql(" ORDER BY ");
        out.push_identifier("depth")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn runtime_hierarchy_builders_populate_all_fields() {
        let h = Hierarchy::new("category_nodes", "id", "parent_id")
            .scoped("domain", "catalog")
            .live_when_null("deleted_at")
            .active_when_true("is_active")
            .scope_key_or_global("scope_key", Some("scope_a"))
            .max_depth(16)
            .max_breadth(4);

        assert_eq!(h.table, "category_nodes");
        assert_eq!(
            h.scope,
            Some(Scope {
                column: "domain",
                value: "catalog"
            })
        );
        assert_eq!(h.live_column, Some("deleted_at"));
        assert_eq!(h.active_column, Some("is_active"));
        assert_eq!(
            h.scope_key,
            Some(ScopeKey {
                column: "scope_key",
                scope_key: Some("scope_a")
            })
        );
        assert_eq!(h.max_depth, 16);
        assert_eq!(h.max_breadth, Some(4));
    }
}
