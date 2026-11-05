//! Profiles: named bundles of configuration. In Phase 3 they are stored and managed only; the
//! launcher that will use them arrives later.
//!
//! Rules (enforced in Rust and again by the schema):
//! - Names are unique, ignoring ASCII upper/lower case. Names are trimmed.
//! - At most one profile is the default. Making one the default atomically un-defaults the old one.
//! - The default cannot be un-set directly (make another profile the default instead), and it
//!   cannot be deleted while other profiles exist. That way, once a default exists, the app never
//!   ends up with "no default but several profiles" by accident.
//! - `configuration_json` must be a JSON object of at most 64 KiB.

use rusqlite::{Connection, Row, params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::errors::{Result, StorageError, unique_to_conflict};
use super::validation;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Profile {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub configuration_json: String,
    pub is_default: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProfileInput {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub configuration_json: String,
    #[serde(default)]
    pub is_default: bool,
}

const COLUMNS: &str =
    "id, name, description, configuration_json, is_default, created_at, updated_at";

fn from_row(r: &Row<'_>) -> rusqlite::Result<Profile> {
    Ok(Profile {
        id: r.get(0)?,
        name: r.get(1)?,
        description: r.get(2)?,
        configuration_json: r.get(3)?,
        is_default: r.get::<_, i64>(4)? == 1,
        created_at: r.get(5)?,
        updated_at: r.get(6)?,
    })
}

struct Clean {
    name: String,
    description: String,
    configuration_json: String,
    is_default: bool,
}

fn clean(input: &ProfileInput) -> Result<Clean> {
    Ok(Clean {
        name: validation::name(&input.name, "profile")?,
        description: validation::description(&input.description)?,
        configuration_json: validation::json_object(
            &input.configuration_json,
            "profile configuration",
            validation::CONFIG_JSON_MAX,
        )?,
        is_default: input.is_default,
    })
}

fn name_taken(name: &str) -> String {
    format!("a profile named \"{name}\" already exists")
}

/// Defaults first, then alphabetical. Bounded: more than 1,000 profiles is not a supported use.
pub fn list(conn: &Connection) -> Result<Vec<Profile>> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM profiles ORDER BY is_default DESC, name COLLATE NOCASE LIMIT 1000"
    ))?;
    let rows = stmt.query_map([], from_row)?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

pub fn get(conn: &Connection, id: i64) -> Result<Profile> {
    Ok(conn
        .prepare_cached(&format!("SELECT {COLUMNS} FROM profiles WHERE id = ?1"))?
        .query_row([id], from_row)?)
}

pub fn create(conn: &mut Connection, input: &ProfileInput, now_ms: i64) -> Result<Profile> {
    let c = clean(input)?;
    let tx = conn.transaction()?;
    if c.is_default {
        tx.execute(
            "UPDATE profiles SET is_default = 0, updated_at = ?1 WHERE is_default = 1",
            [now_ms],
        )?;
    }
    tx.execute(
        "INSERT INTO profiles (name, description, configuration_json, is_default, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        params![c.name, c.description, c.configuration_json, c.is_default, now_ms],
    )
    .map_err(|e| unique_to_conflict(e, name_taken(&c.name)))?;
    let id = tx.last_insert_rowid();
    let created = get(&tx, id)?;
    tx.commit()?;
    Ok(created)
}

pub fn update(
    conn: &mut Connection,
    id: i64,
    input: &ProfileInput,
    now_ms: i64,
) -> Result<Profile> {
    let c = clean(input)?;
    let tx = conn.transaction()?;
    let existing = get(&tx, id)?;
    if existing.is_default && !c.is_default {
        return Err(StorageError::Conflict(
            "this is the default profile; make another profile the default instead of clearing this one"
                .into(),
        ));
    }
    if c.is_default && !existing.is_default {
        tx.execute(
            "UPDATE profiles SET is_default = 0, updated_at = ?1 WHERE is_default = 1",
            [now_ms],
        )?;
    }
    tx.execute(
        "UPDATE profiles
            SET name = ?1, description = ?2, configuration_json = ?3, is_default = ?4, updated_at = ?5
          WHERE id = ?6",
        params![c.name, c.description, c.configuration_json, c.is_default, now_ms, id],
    )
    .map_err(|e| unique_to_conflict(e, name_taken(&c.name)))?;
    let updated = get(&tx, id)?;
    tx.commit()?;
    Ok(updated)
}

pub fn delete(conn: &mut Connection, id: i64) -> Result<()> {
    let tx = conn.transaction()?;
    let existing = get(&tx, id)?;
    if existing.is_default {
        let others: i64 =
            tx.query_row("SELECT COUNT(*) FROM profiles WHERE id <> ?1", [id], |r| {
                r.get(0)
            })?;
        if others > 0 {
            return Err(StorageError::Conflict(
                "the default profile cannot be deleted while other profiles exist; make another profile the default first"
                    .into(),
            ));
        }
    }
    tx.execute("DELETE FROM profiles WHERE id = ?1", [id])?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test_support::fresh;

    fn input(name: &str) -> ProfileInput {
        ProfileInput {
            name: name.into(),
            description: String::new(),
            configuration_json: r#"{"fps":60}"#.into(),
            is_default: false,
        }
    }

    #[test]
    fn create_list_get_update_delete() {
        let (_t, db) = fresh();
        db.with(|c| {
            assert!(list(c)?.is_empty());
            let a = create(
                c,
                &ProfileInput {
                    description: "  main  ".into(),
                    ..input("  Alpha ")
                },
                100,
            )?;
            assert_eq!((a.name.as_str(), a.description.as_str()), ("Alpha", "main"));
            assert_eq!(
                (a.created_at, a.updated_at, a.is_default),
                (100, 100, false)
            );
            let b = create(c, &input("beta"), 200)?;
            assert_eq!(
                list(c)?.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
                ["Alpha", "beta"]
            );
            assert_eq!(get(c, b.id)?, b);

            let upd = update(
                c,
                a.id,
                &ProfileInput {
                    configuration_json: r#"{"fps":144}"#.into(),
                    ..input("Alpha 2")
                },
                300,
            )?;
            assert_eq!(
                (upd.name.as_str(), upd.configuration_json.as_str()),
                ("Alpha 2", r#"{"fps":144}"#)
            );
            assert_eq!((upd.created_at, upd.updated_at), (100, 300));

            delete(c, a.id)?;
            assert!(matches!(get(c, a.id), Err(StorageError::NotFound)));
            assert_eq!(list(c)?.len(), 1);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn names_are_unique_ignoring_case() {
        let (_t, db) = fresh();
        db.with(|c| {
            create(c, &input("Main"), 1)?;
            for dup in ["Main", "main", "  MAIN  "] {
                let err = create(c, &input(dup), 2).unwrap_err();
                assert!(
                    matches!(err, StorageError::Conflict(ref m) if m.contains("already exists")),
                    "{dup}: {err}"
                );
            }
            let other = create(c, &input("Other"), 3)?;
            let err = update(c, other.id, &input("mAiN"), 4).unwrap_err();
            assert!(matches!(err, StorageError::Conflict(_)));
            assert_eq!(
                get(c, other.id)?.name,
                "Other",
                "failed rename changes nothing"
            );
            // Renaming a profile to its own name (different case) is allowed.
            assert_eq!(update(c, other.id, &input("OTHER"), 5)?.name, "OTHER");
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn invalid_input_is_rejected_and_nothing_is_stored() {
        let (_t, db) = fresh();
        db.with(|c| {
            let bad = [
                ProfileInput {
                    name: "  ".into(),
                    ..input("x")
                },
                ProfileInput {
                    name: "x".repeat(65),
                    ..input("x")
                },
                ProfileInput {
                    name: "tab\there".into(),
                    ..input("x")
                },
                ProfileInput {
                    description: "d".repeat(501),
                    ..input("ok")
                },
                ProfileInput {
                    configuration_json: "not json".into(),
                    ..input("ok")
                },
                ProfileInput {
                    configuration_json: "[1,2]".into(),
                    ..input("ok")
                },
                ProfileInput {
                    configuration_json: "null".into(),
                    ..input("ok")
                },
                ProfileInput {
                    configuration_json: format!("{{\"k\":\"{}\"}}", "x".repeat(70_000)),
                    ..input("ok")
                },
            ];
            for b in &bad {
                assert!(
                    matches!(create(c, b, 1), Err(StorageError::Invalid(_))),
                    "{b:?}"
                );
            }
            assert!(list(c)?.is_empty());
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn schema_rejects_bad_rows_even_if_rust_were_bypassed() {
        let (_t, db) = fresh();
        db.with(|c| {
            let sql = "INSERT INTO profiles (name, description, configuration_json, is_default, created_at, updated_at)
                       VALUES (?1, '', ?2, ?3, 1, 1)";
            assert!(c.execute(sql, params!["", "{}", 0]).is_err(), "empty name");
            assert!(c.execute(sql, params!["x", "nope", 0]).is_err(), "invalid json");
            assert!(c.execute(sql, params!["x", "{}", 2]).is_err(), "is_default must be 0/1");
            c.execute(sql, params!["a", "{}", 1])?;
            assert!(c.execute(sql, params!["b", "{}", 1]).is_err(), "second default refused by the index");
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn only_one_default_exists_at_any_time() {
        let (_t, db) = fresh();
        db.with(|c| {
            let a = create(
                c,
                &ProfileInput {
                    is_default: true,
                    ..input("A")
                },
                1,
            )?;
            let b = create(
                c,
                &ProfileInput {
                    is_default: true,
                    ..input("B")
                },
                2,
            )?;
            let defaults = |c: &Connection| -> Result<Vec<String>> {
                Ok(list(c)?
                    .into_iter()
                    .filter(|p| p.is_default)
                    .map(|p| p.name)
                    .collect())
            };
            assert_eq!(defaults(c)?, ["B"]);
            assert!(!get(c, a.id)?.is_default);

            update(
                c,
                a.id,
                &ProfileInput {
                    is_default: true,
                    ..input("A")
                },
                3,
            )?;
            assert_eq!(defaults(c)?, ["A"]);
            assert!(!get(c, b.id)?.is_default);
            assert_eq!(list(c)?[0].name, "A", "the default is listed first");
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn default_profile_is_protected_from_unsetting_and_deleting() {
        let (_t, db) = fresh();
        db.with(|c| {
            let a = create(
                c,
                &ProfileInput {
                    is_default: true,
                    ..input("A")
                },
                1,
            )?;
            let b = create(c, &input("B"), 2)?;
            assert!(matches!(
                update(c, a.id, &input("A"), 3),
                Err(StorageError::Conflict(_))
            ));
            assert!(get(c, a.id)?.is_default);
            assert!(matches!(delete(c, a.id), Err(StorageError::Conflict(_))));
            // Hand over the default, then deleting the old one is fine.
            update(
                c,
                b.id,
                &ProfileInput {
                    is_default: true,
                    ..input("B")
                },
                4,
            )?;
            delete(c, a.id)?;
            // The only profile may be deleted even though it is the default.
            delete(c, b.id)?;
            assert!(list(c)?.is_empty());
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn missing_ids_are_not_found() {
        let (_t, db) = fresh();
        db.with(|c| {
            assert!(matches!(get(c, 42), Err(StorageError::NotFound)));
            assert!(matches!(
                update(c, 42, &input("x"), 1),
                Err(StorageError::NotFound)
            ));
            assert!(matches!(delete(c, 42), Err(StorageError::NotFound)));
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn a_failed_multi_step_change_rolls_back_completely() {
        let (_t, db) = fresh();
        db.with(|c| {
            let a = create(
                c,
                &ProfileInput {
                    is_default: true,
                    ..input("A")
                },
                1,
            )?;
            create(c, &input("Taken"), 2)?;
            // Creating a *default* profile whose name is taken: the un-defaulting of A happens
            // first inside the transaction, then the insert fails. A must still be the default.
            let err = create(
                c,
                &ProfileInput {
                    is_default: true,
                    ..input("taken")
                },
                3,
            )
            .unwrap_err();
            assert!(matches!(err, StorageError::Conflict(_)));
            assert!(
                get(c, a.id)?.is_default,
                "rollback restored the previous default"
            );
            assert_eq!(get(c, a.id)?.updated_at, 1, "and its timestamp");
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn data_survives_reopening() {
        let t = crate::storage::test_support::TempDir::new();
        let path = t.path().join("roalt.db");
        {
            let (db, _) = crate::storage::Database::open(&path, 1).unwrap();
            db.with(|c| {
                create(
                    c,
                    &ProfileInput {
                        is_default: true,
                        ..input("Kept")
                    },
                    5,
                )
                .map(|_| ())
            })
            .unwrap();
        }
        let (db, _) = crate::storage::Database::open(&path, 2).unwrap();
        let all = db.with(|c| list(c)).unwrap();
        assert_eq!(
            (all.len(), all[0].name.as_str(), all[0].is_default),
            (1, "Kept", true)
        );
    }
}
