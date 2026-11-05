//! Automation profiles: stored configuration only. Phase 3 does not run, schedule or evaluate
//! anything; `enabled` is just a saved flag for the automation engine that comes later.
//!
//! Names are unique ignoring ASCII case; `configuration_json` must be a JSON object (64 KiB max).

use rusqlite::{Connection, Row, params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::errors::{Result, unique_to_conflict};
use super::validation;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AutomationProfile {
    pub id: i64,
    pub name: String,
    pub configuration_json: String,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AutomationProfileInput {
    pub name: String,
    pub configuration_json: String,
    #[serde(default)]
    pub enabled: bool,
}

const COLUMNS: &str = "id, name, configuration_json, enabled, created_at, updated_at";

fn from_row(r: &Row<'_>) -> rusqlite::Result<AutomationProfile> {
    Ok(AutomationProfile {
        id: r.get(0)?,
        name: r.get(1)?,
        configuration_json: r.get(2)?,
        enabled: r.get::<_, i64>(3)? == 1,
        created_at: r.get(4)?,
        updated_at: r.get(5)?,
    })
}

fn clean(input: &AutomationProfileInput) -> Result<(String, String)> {
    Ok((
        validation::name(&input.name, "automation profile")?,
        validation::json_object(
            &input.configuration_json,
            "automation configuration",
            validation::CONFIG_JSON_MAX,
        )?,
    ))
}

fn name_taken(name: &str) -> String {
    format!("an automation profile named \"{name}\" already exists")
}

pub fn list(conn: &Connection) -> Result<Vec<AutomationProfile>> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM automation_profiles ORDER BY name COLLATE NOCASE LIMIT 1000"
    ))?;
    let rows = stmt.query_map([], from_row)?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

pub fn get(conn: &Connection, id: i64) -> Result<AutomationProfile> {
    Ok(conn
        .prepare_cached(&format!(
            "SELECT {COLUMNS} FROM automation_profiles WHERE id = ?1"
        ))?
        .query_row([id], from_row)?)
}

pub fn create(
    conn: &Connection,
    input: &AutomationProfileInput,
    now_ms: i64,
) -> Result<AutomationProfile> {
    let (name, json) = clean(input)?;
    conn.execute(
        "INSERT INTO automation_profiles (name, configuration_json, enabled, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?4)",
        params![name, json, input.enabled, now_ms],
    )
    .map_err(|e| unique_to_conflict(e, name_taken(&name)))?;
    get(conn, conn.last_insert_rowid())
}

pub fn update(
    conn: &Connection,
    id: i64,
    input: &AutomationProfileInput,
    now_ms: i64,
) -> Result<AutomationProfile> {
    let (name, json) = clean(input)?;
    let changed = conn
        .execute(
            "UPDATE automation_profiles
                SET name = ?1, configuration_json = ?2, enabled = ?3, updated_at = ?4
              WHERE id = ?5",
            params![name, json, input.enabled, now_ms, id],
        )
        .map_err(|e| unique_to_conflict(e, name_taken(&name)))?;
    if changed == 0 {
        return Err(super::errors::StorageError::NotFound);
    }
    get(conn, id)
}

pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    if conn.execute("DELETE FROM automation_profiles WHERE id = ?1", [id])? == 0 {
        return Err(super::errors::StorageError::NotFound);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::errors::StorageError;
    use crate::storage::test_support::fresh;

    fn input(name: &str) -> AutomationProfileInput {
        AutomationProfileInput {
            name: name.into(),
            configuration_json: r#"{"retries":3}"#.into(),
            enabled: false,
        }
    }

    #[test]
    fn crud_round_trip() {
        let (_t, db) = fresh();
        db.with(|c| {
            assert!(list(c)?.is_empty());
            let a = create(c, &input(" Rejoin "), 10)?;
            assert_eq!(
                (a.name.as_str(), a.enabled, a.created_at),
                ("Rejoin", false, 10)
            );
            let upd = update(
                c,
                a.id,
                &AutomationProfileInput {
                    enabled: true,
                    configuration_json: r#"{"retries":5}"#.into(),
                    ..input("Rejoin")
                },
                20,
            )?;
            assert_eq!(
                (upd.enabled, upd.configuration_json.as_str()),
                (true, r#"{"retries":5}"#)
            );
            assert_eq!((upd.created_at, upd.updated_at), (10, 20));
            assert_eq!(list(c)?, vec![upd.clone()]);
            delete(c, a.id)?;
            assert!(list(c)?.is_empty());
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn duplicate_names_and_bad_input_are_rejected() {
        let (_t, db) = fresh();
        db.with(|c| {
            let a = create(c, &input("Same"), 1)?;
            assert!(matches!(
                create(c, &input("SAME"), 2),
                Err(StorageError::Conflict(_))
            ));
            let b = create(c, &input("Other"), 3)?;
            assert!(matches!(
                update(c, b.id, &input("same"), 4),
                Err(StorageError::Conflict(_))
            ));
            for bad in [
                AutomationProfileInput {
                    name: "".into(),
                    ..input("x")
                },
                AutomationProfileInput {
                    configuration_json: "[]".into(),
                    ..input("ok")
                },
                AutomationProfileInput {
                    configuration_json: "{broken".into(),
                    ..input("ok")
                },
            ] {
                assert!(
                    matches!(create(c, &bad, 5), Err(StorageError::Invalid(_))),
                    "{bad:?}"
                );
            }
            assert_eq!(list(c)?.len(), 2);
            assert_eq!(get(c, a.id)?.name, "Same");
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn missing_ids_are_not_found() {
        let (_t, db) = fresh();
        db.with(|c| {
            assert!(matches!(get(c, 1), Err(StorageError::NotFound)));
            assert!(matches!(
                update(c, 1, &input("x"), 1),
                Err(StorageError::NotFound)
            ));
            assert!(matches!(delete(c, 1), Err(StorageError::NotFound)));
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn schema_guards_enabled_flag_and_json() {
        let (_t, db) = fresh();
        db.with(|c| {
            let sql = "INSERT INTO automation_profiles (name, configuration_json, enabled, created_at, updated_at)
                       VALUES ('x', ?1, ?2, 1, 1)";
            assert!(c.execute(sql, params!["{}", 2]).is_err());
            assert!(c.execute(sql, params!["oops", 0]).is_err());
            assert!(c.execute(sql, params!["{}", 1]).is_ok());
            Ok(())
        })
        .unwrap();
    }
}
