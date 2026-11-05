//! Timing measurements for `docs/PERFORMANCE.md`. They are `#[ignore]`d so normal test runs stay
//! fast and deterministic. Reproduce with:
//!
//! `cargo test --release storage::perf_tests -- --ignored --nocapture --test-threads=1`
//!
//! The numbers depend on the machine and file system; the document says which ones were used.

use std::time::{Duration, Instant};

use super::activity_logs::{self, ActivityLogQuery, MAX_ROWS, NewActivityLog, Severity};
use super::database::{DB_FILE_NAME, Database};
use super::test_support::TempDir;

fn entry(i: i64) -> NewActivityLog {
    NewActivityLog {
        event_type: if i % 7 == 0 {
            "database.backup_created"
        } else {
            "app.started"
        }
        .into(),
        severity: match i % 20 {
            0 => Severity::Error,
            1..=3 => Severity::Warning,
            _ => Severity::Info,
        },
        message: format!(
            "Synthetic measurement entry number {i} with a message of realistic length."
        ),
        metadata_json: None,
    }
}

fn avg(total: Duration, n: u32) -> String {
    let us = total.as_secs_f64() * 1e6 / f64::from(n);
    if us >= 1000.0 {
        format!("{:.2} ms", us / 1000.0)
    } else {
        format!("{us:.0} µs")
    }
}

fn time<R>(f: impl FnOnce() -> R) -> (R, Duration) {
    let t = Instant::now();
    let r = f();
    (r, t.elapsed())
}

#[test]
#[ignore = "measurement, run on demand"]
fn report() {
    let dir = TempDir::new();
    let path = dir.path().join(DB_FILE_NAME);

    let ((db, _), cold) = time(|| Database::open(&path, 1).unwrap());
    println!(
        "open + migrate (new file):                 {:.1} ms",
        cold.as_secs_f64() * 1e3
    );

    // Single-row commits, the way the app writes one event at a time (synchronous = FULL).
    let ((), t) = time(|| {
        for i in 0..500 {
            db.with(|c| activity_logs::create(c, &entry(i), i).map(|_| ()))
                .unwrap();
        }
    });
    println!(
        "create_activity_log, 500 single commits:   {} each",
        avg(t, 500)
    );

    // Fill to the cap with one transaction (not how the app writes; it only builds the data set).
    db.with(|c| {
        let tx = c.transaction()?;
        {
            let mut s = tx.prepare(
                "INSERT INTO activity_logs (event_type, severity, message, created_at) VALUES (?1,?2,?3,?4)",
            )?;
            for i in 500..MAX_ROWS {
                let e = entry(i);
                s.execute(rusqlite::params![e.event_type, e.severity.as_str(), e.message, i])?;
            }
        }
        tx.commit()?;
        Ok(())
    })
    .unwrap();
    db.checkpoint().unwrap();
    println!(
        "rows at the cap:                           {}",
        db.with(|c| activity_logs::count(c)).unwrap()
    );
    println!(
        "main file size at the cap:                 {} KB",
        std::fs::metadata(&path).unwrap().len() / 1024
    );

    let ((), t) = time(|| {
        for i in 0..200 {
            db.with(|c| activity_logs::create(c, &entry(i), 20_000 + i).map(|_| ()))
                .unwrap();
        }
    });
    println!(
        "create_activity_log at the cap (+trim):    {} each",
        avg(t, 200)
    );
    println!(
        "rows after 200 more (still bounded):       {}",
        db.with(|c| activity_logs::count(c)).unwrap()
    );

    let q = ActivityLogQuery {
        limit: Some(50),
        ..Default::default()
    };
    let ((), t) = time(|| {
        for _ in 0..500 {
            db.with(|c| activity_logs::list(c, &q).map(|_| ())).unwrap();
        }
    });
    println!(
        "list newest 50 of 10,000:                  {} each",
        avg(t, 500)
    );

    let qs = ActivityLogQuery {
        limit: Some(50),
        severity: Some(Severity::Error),
        ..Default::default()
    };
    let ((), t) = time(|| {
        for _ in 0..500 {
            db.with(|c| activity_logs::list(c, &qs).map(|_| ()))
                .unwrap();
        }
    });
    println!(
        "list newest 50 errors (filtered):          {} each",
        avg(t, 500)
    );

    let deep = ActivityLogQuery {
        limit: Some(50),
        before_id: Some(120),
        ..Default::default()
    };
    let ((), t) = time(|| {
        for _ in 0..500 {
            db.with(|c| activity_logs::list(c, &deep).map(|_| ()))
                .unwrap();
        }
    });
    println!(
        "list 50 from the oldest end (deep page):   {} each",
        avg(t, 500)
    );

    let out = TempDir::new();
    let dest = out.path().join("b.db");
    let (info, t) = time(|| db.backup_to(&dest, false).unwrap());
    println!(
        "backup (VACUUM INTO + validate), {} KB:   {:.1} ms",
        info.size_bytes / 1024,
        t.as_secs_f64() * 1e3
    );
    let (_, t) = time(|| db.restore_from(&dest, 5).unwrap());
    println!(
        "restore (copy + validate + safety + load): {:.1} ms",
        t.as_secs_f64() * 1e3
    );

    drop(db);
    let ((_db, _), t) = time(|| Database::open(&path, 9).unwrap());
    println!(
        "reopen with 10,000 rows (incl. quick_check): {:.1} ms",
        t.as_secs_f64() * 1e3
    );
}
