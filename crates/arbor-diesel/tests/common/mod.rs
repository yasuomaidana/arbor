//! Shared traversal tests, instantiated once per backend in separate test binaries: SQLite and
//! libSQL resolve to the same `sqlite3_*` symbols, and libSQL must configure SQLite before any
//! plain `diesel::sqlite` connection opens, so the two must not share a process.
#![allow(dead_code)]

pub use arbor_diesel::sql::{Hierarchy, creates_cycle, reached_depth_limit};

pub const SCHEMA: &str = "
    CREATE TABLE category_nodes (
        id TEXT PRIMARY KEY,
        parent_id TEXT REFERENCES category_nodes(id) ON DELETE RESTRICT,
        domain TEXT NOT NULL,
        scope_key TEXT,
        is_active BOOLEAN NOT NULL DEFAULT 1,
        deleted_at TEXT
    );
    INSERT INTO category_nodes (id, parent_id, domain, scope_key, is_active, deleted_at) VALUES
        ('electronics', NULL, 'catalog', NULL, 1, NULL),
        ('computers', 'electronics', 'catalog', NULL, 1, NULL),
        ('all_in_one', 'computers', 'catalog', NULL, 1, NULL),
        ('custom_build', 'computers', 'catalog', 'scope_a', 1, NULL),
        ('modular_rig', 'computers', 'catalog', 'scope_b', 1, NULL),
        ('inactive_model', 'computers', 'catalog', NULL, 0, NULL),
        ('retired_model', 'computers', 'catalog', NULL, 1, '2026-10-01'),
        ('engineering', NULL, 'org', NULL, 1, NULL),
        ('platform', 'engineering', 'org', NULL, 1, NULL);
    -- a table named with a reserved word, to exercise identifier quoting
    CREATE TABLE \"order\" (\"key\" TEXT PRIMARY KEY, \"up\" TEXT);
    INSERT INTO \"order\" VALUES ('a', NULL), ('b', 'a');
    -- cyclic data: x -> y -> x
    CREATE TABLE loops (id TEXT PRIMARY KEY, parent_id TEXT);
    INSERT INTO loops VALUES ('x', 'y'), ('y', 'x');
";

pub const CATEGORIES: Hierarchy = Hierarchy::new("category_nodes", "id", "parent_id");
pub const CATALOG: Hierarchy = CATEGORIES
    .scoped("domain", "catalog")
    .live_when_null("deleted_at");
pub const ACTIVE_CATALOG: Hierarchy = CATALOG.active_when_true("is_active");
pub const SCOPE_A_CATALOG: Hierarchy = CATALOG.scope_key_or_global("scope_key", Some("scope_a"));
pub const GLOBAL_CATALOG: Hierarchy = CATALOG.scope_key_or_global("scope_key", None);
pub const BREADTH_CATALOG: Hierarchy = CATALOG.max_breadth(1);
pub const ORDER: Hierarchy = Hierarchy::new("order", "key", "up");
pub const LOOPS: Hierarchy = Hierarchy::new("loops", "id", "parent_id").max_depth(5);

pub type Row = (String, Option<String>, i32);

pub fn ids(rows: &[Row]) -> Vec<&str> {
    rows.iter().map(|(id, _, _)| id.as_str()).collect()
}

#[macro_export]
macro_rules! traversal_tests {
    ($module:ident, $backend:ty, $connect:expr) => {
        mod $module {
            use super::common::*;
            use diesel::RunQueryDsl;
            use diesel::connection::SimpleConnection;
            use diesel::sql_types::Text;
            use rstest::rstest;

            fn conn() -> impl diesel::connection::LoadConnection<Backend = $backend> {
                let mut conn = $connect;
                conn.batch_execute(SCHEMA).expect("schema");
                conn
            }

            fn load_descendants(hierarchy: &Hierarchy, start: &str) -> Vec<Row> {
                let mut conn = conn();
                hierarchy
                    .descendants::<$backend, Text, _>(start.to_string())
                    .load(&mut conn)
                    .expect("descendants")
            }

            fn load_ancestors(hierarchy: &Hierarchy, start: &str) -> Vec<Row> {
                let mut conn = conn();
                hierarchy
                    .ancestors::<$backend, Text, _>(start.to_string())
                    .load(&mut conn)
                    .expect("ancestors")
            }

            #[rstest]
            fn descendants_walk_the_whole_subtree_by_depth() {
                let rows = load_descendants(&CATEGORIES, "electronics");
                assert_eq!(
                    ids(&rows),
                    vec![
                        "electronics",
                        "computers",
                        "all_in_one",
                        "custom_build",
                        "inactive_model",
                        "modular_rig",
                        "retired_model"
                    ]
                );
                assert_eq!(rows[2], ("all_in_one".into(), Some("computers".into()), 2));
            }

            #[rstest]
            fn ancestors_walk_up_to_the_root() {
                let rows = load_ancestors(&CATEGORIES, "all_in_one");
                assert_eq!(ids(&rows), vec!["all_in_one", "computers", "electronics"]);
            }

            #[rstest]
            fn scope_and_soft_delete_filters_apply_at_every_level() {
                let rows = load_descendants(&CATALOG, "electronics");
                assert_eq!(
                    ids(&rows),
                    vec![
                        "electronics",
                        "computers",
                        "all_in_one",
                        "custom_build",
                        "inactive_model",
                        "modular_rig"
                    ]
                );

                let other_domain = load_descendants(&CATALOG, "engineering");
                assert!(
                    other_domain.is_empty(),
                    "an org-domain root is outside the catalog scope"
                );
            }

            #[rstest]
            #[case::active_only(
                ACTIVE_CATALOG,
                vec!["electronics", "computers", "all_in_one", "custom_build", "modular_rig"]
            )]
            #[case::scope_a_or_global(
                SCOPE_A_CATALOG,
                vec!["electronics", "computers", "all_in_one", "custom_build", "inactive_model"]
            )]
            #[case::global_only(
                GLOBAL_CATALOG,
                vec!["electronics", "computers", "all_in_one", "inactive_model"]
            )]
            #[case::breadth_one(
                BREADTH_CATALOG,
                vec!["electronics", "computers", "all_in_one"]
            )]
            fn filtered_descendant_traversals(
                #[case] hierarchy: Hierarchy,
                #[case] expected: Vec<&str>,
            ) {
                let rows = load_descendants(&hierarchy, "electronics");
                assert_eq!(ids(&rows), expected);
            }

            #[rstest]
            fn reserved_word_identifiers_are_quoted() {
                let rows = load_descendants(&ORDER, "a");
                assert_eq!(ids(&rows), vec!["a", "b"]);
            }

            #[rstest]
            fn cyclic_data_stops_at_max_depth() {
                let rows = load_descendants(&LOOPS, "x");
                assert_eq!(rows.len(), 6, "depths 0..=5");
                assert!(reached_depth_limit(&LOOPS, &rows));
                assert!(!reached_depth_limit(
                    &CATEGORIES,
                    &[("electronics".to_string(), None, 0)]
                ));
            }

            #[rstest]
            fn different_tables_never_share_a_cached_statement() {
                let mut conn = conn();
                let categories: Vec<Row> = CATEGORIES
                    .descendants::<$backend, Text, _>("electronics".to_string())
                    .load(&mut conn)
                    .expect("categories");
                let order: Vec<Row> = ORDER
                    .descendants::<$backend, Text, _>("a".to_string())
                    .load(&mut conn)
                    .expect("order");
                assert_eq!(categories.len(), 7);
                assert_eq!(ids(&order), vec!["a", "b"]);
            }

            #[rstest]
            fn cycle_check_from_ancestors() {
                let ancestors_of_all_in_one = load_ancestors(&CATEGORIES, "all_in_one");
                // Moving `electronics` under `all_in_one` would make electronics its own ancestor.
                assert!(creates_cycle(
                    &"electronics".to_string(),
                    &ancestors_of_all_in_one
                ));
                assert!(!creates_cycle(
                    &"engineering".to_string(),
                    &ancestors_of_all_in_one
                ));
            }
        }
    };
}
