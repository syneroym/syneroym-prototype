use anyhow::Result;
use rusqlite::{params_from_iter, Connection};
use serde_json::Value;

pub struct DbOperations;

impl DbOperations {
    pub fn execute(conn: &Connection, sql: &str, params: Vec<String>) -> Result<Vec<Vec<Vec<u8>>>> {
        let mut stmt = conn.prepare(sql)?;

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(|p| p as &dyn rusqlite::ToSql).collect();

        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            let column_count = row.as_ref().column_count();
            let mut row_data = Vec::new();

            for i in 0..column_count {
                let value: Result<Vec<u8>, _> = row.get(i);
                if let Ok(bytes) = value {
                    row_data.push(bytes);
                } else {
                    // Try to get as string and convert
                    let str_value: Result<String, _> = row.get(i);
                    if let Ok(s) = str_value {
                        row_data.push(s.into_bytes());
                    } else {
                        // Try as i64
                        let int_value: Result<i64, _> = row.get(i);
                        if let Ok(i) = int_value {
                            row_data.push(i.to_string().into_bytes());
                        } else {
                            row_data.push(Vec::new());
                        }
                    }
                }
            }

            Ok(row_data)
        })?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }

        Ok(result)
    }

    pub fn insert(conn: &Connection, table: &str, data: Vec<u8>) -> Result<u64> {
        let value: Value = serde_json::from_slice(&data)?;

        if let Value::Object(map) = value {
            let columns: Vec<String> = map.keys().cloned().collect();
            let placeholders: Vec<String> = (0..columns.len()).map(|_| "?".to_string()).collect();

            let sql = format!(
                "INSERT INTO {} ({}) VALUES ({})",
                table,
                columns.join(", "),
                placeholders.join(", ")
            );

            let values: Vec<String> = columns
                .iter()
                .filter_map(|col| {
                    map.get(col).and_then(|v| match v {
                        Value::String(s) => Some(s.clone()),
                        Value::Number(n) => Some(n.to_string()),
                        Value::Bool(b) => Some(b.to_string()),
                        _ => None,
                    })
                })
                .collect();

            let params_refs: Vec<&dyn rusqlite::ToSql> =
                values.iter().map(|v| v as &dyn rusqlite::ToSql).collect();

            conn.execute(&sql, params_refs.as_slice())?;
            Ok(conn.last_insert_rowid() as u64)
        } else {
            Err(anyhow::anyhow!("Expected object for insert"))
        }
    }

    pub fn update(conn: &Connection, table: &str, id: u64, data: Vec<u8>) -> Result<bool> {
        let value: Value = serde_json::from_slice(&data)?;

        if let Value::Object(map) = value {
            let set_clauses: Vec<String> = map.keys().map(|k| format!("{} = ?", k)).collect();

            let sql = format!(
                "UPDATE {} SET {} WHERE id = ?",
                table,
                set_clauses.join(", ")
            );

            let mut values: Vec<String> = map
                .values()
                .filter_map(|v| match v {
                    Value::String(s) => Some(s.clone()),
                    Value::Number(n) => Some(n.to_string()),
                    Value::Bool(b) => Some(b.to_string()),
                    _ => None,
                })
                .collect();
            values.push(id.to_string());

            let params_refs: Vec<&dyn rusqlite::ToSql> =
                values.iter().map(|v| v as &dyn rusqlite::ToSql).collect();

            let affected = conn.execute(&sql, params_refs.as_slice())?;
            Ok(affected > 0)
        } else {
            Err(anyhow::anyhow!("Expected object for update"))
        }
    }

    pub fn delete(conn: &Connection, table: &str, id: u64) -> Result<bool> {
        let sql = format!("DELETE FROM {} WHERE id = ?", table);
        let affected = conn.execute(&sql, [id])?;
        Ok(affected > 0)
    }
}
