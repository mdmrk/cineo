//! A read-only health report for `cineo doctor`. It never creates, migrates
//! or repairs the database.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};

use crate::migrate::{SCHEMA_VERSION, user_version};

/// What [`diagnose`] found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub path: PathBuf,
    pub exists: bool,
    pub schema_version: Option<i64>,
    /// Row counts; `None` if the table is missing or unreadable.
    pub addons: Option<i64>,
    pub library_items: Option<i64>,
    /// SQLite's `integrity_check` result: `["ok"]` when healthy.
    pub integrity: Vec<String>,
    /// Why the database could not be inspected, if it could not.
    pub error: Option<String>,
}

impl Report {
    /// A missing database is healthy (it is created on first use).
    pub fn is_healthy(&self) -> bool {
        !self.exists
            || (self.error.is_none()
                && self.integrity == ["ok"]
                && self.schema_version.is_some_and(|v| v <= SCHEMA_VERSION))
    }
}

/// Inspects the database at `path` without modifying it.
pub fn diagnose(path: &Path) -> Report {
    let mut report = Report {
        path: path.to_path_buf(),
        exists: path.exists(),
        schema_version: None,
        addons: None,
        library_items: None,
        integrity: Vec::new(),
        error: None,
    };
    if !report.exists {
        return report;
    }
    if let Err(err) = inspect(path, &mut report) {
        report.error = Some(err.to_string());
    }
    report
}

fn inspect(path: &Path, report: &mut Report) -> rusqlite::Result<()> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    report.schema_version = Some(user_version(&conn)?);
    let count = |table: &str| -> Option<i64> {
        // Table names are constants below, never input.
        conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .ok()
    };
    report.addons = count("addons");
    report.library_items = count("library_items");
    let mut stmt = conn.prepare("PRAGMA integrity_check")?;
    report.integrity = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(())
}
