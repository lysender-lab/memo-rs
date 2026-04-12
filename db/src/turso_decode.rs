use snafu::ResultExt;
use turso::{Row, Rows, Value};

use crate::error::{DbRowSnafu, DbValueSnafu};
use crate::{Error, Result};

pub trait FromTursoRow: Sized {
    fn from_row(row: &Row) -> Result<Self>;
}

pub struct RowCountDto {
    pub total_count: i64,
}

impl FromTursoRow for RowCountDto {
    fn from_row(row: &Row) -> Result<Self> {
        Ok(Self {
            total_count: row_integer(row, 0)?,
        })
    }
}

pub fn collect_row<T: FromTursoRow>(row: turso::Result<Row>) -> Result<Option<T>> {
    match row {
        Ok(t_row) => Ok(Some(T::from_row(&t_row)?)),
        Err(turso::Error::QueryReturnedNoRows) => Ok(None),
        Err(err) => Err(Error::DbRow { source: err }),
    }
}

pub async fn collect_rows<T: FromTursoRow>(rows: &mut Rows) -> Result<Vec<T>> {
    let mut items = Vec::new();

    while let Some(row) = rows.next().await.context(DbRowSnafu)? {
        items.push(T::from_row(&row)?);
    }

    Ok(items)
}

pub fn collect_count(row: turso::Result<Row>) -> Result<i64> {
    let count_row: Option<RowCountDto> = collect_row(row)?;
    match count_row {
        Some(dto) => Ok(dto.total_count),
        None => Ok(0),
    }
}

pub fn row_text(row: &Row, idx: usize) -> Result<String> {
    let value = row.get_value(idx).context(DbValueSnafu)?;
    value
        .as_text()
        .cloned()
        .ok_or_else(|| format!("Expected text value at column index {idx}").into())
}

pub fn row_integer(row: &Row, idx: usize) -> Result<i64> {
    let value = row.get_value(idx).context(DbValueSnafu)?;
    value
        .as_integer()
        .copied()
        .ok_or_else(|| format!("Expected integer value at column index {idx}").into())
}

pub fn opt_row_text(row: &Row, idx: usize) -> Result<Option<String>> {
    let value = row.get_value(idx).context(DbValueSnafu)?;

    match value {
        Value::Null => Ok(None),
        _ => value
            .as_text()
            .cloned()
            .map(Some)
            .ok_or_else(|| format!("Expected nullable text value at column index {idx}").into()),
    }
}

#[allow(dead_code)]
pub fn opt_row_integer(row: &Row, idx: usize) -> Result<Option<i64>> {
    let value = row.get_value(idx).context(DbValueSnafu)?;

    match value {
        Value::Null => Ok(None),
        _ => {
            value.as_integer().copied().map(Some).ok_or_else(|| {
                format!("Expected nullable integer value at column index {idx}").into()
            })
        }
    }
}
