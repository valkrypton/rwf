//! Implements the `SELECT` query.
use super::{Column, Escape, FromRow, Model, Placeholders, ToColumn, ToSql, ToValue};
use std::marker::PhantomData;

#[derive(Debug, Clone)]
pub struct Insert<T> {
    table_name: String,
    columns: Vec<Column>,
    pub placeholders: Placeholders,
    marker: PhantomData<T>,
    no_conflict: bool,
    unique_by: Vec<Column>,
}

impl<T: Model> Insert<T> {
    pub fn new(model: T) -> Self {
        let columns = T::column_names()
            .into_iter()
            .map(|column| Column::name(column))
            .collect();
        let values = model.values();
        let mut placeholders = Placeholders::new();
        for value in values {
            placeholders.add(&value);
        }

        Self {
            table_name: T::table_name().to_string(),
            placeholders,
            columns,
            marker: PhantomData,
            no_conflict: false,
            unique_by: vec![],
        }
    }

    pub fn from_columns(columns: &[impl ToColumn], values: &[impl ToValue]) -> Self {
        let mut placeholders = Placeholders::new();
        for value in values {
            let value = value.to_value();
            placeholders.add(&value);
        }

        Insert {
            table_name: T::table_name().to_string(),
            columns: columns.iter().map(|c| c.to_column().unqualify()).collect(),
            placeholders,
            marker: PhantomData,
            no_conflict: false,
            unique_by: vec![],
        }
    }

    pub fn no_conflict(mut self) -> Self {
        self.no_conflict = true;
        self
    }

    pub fn unique_by(mut self, columns: &[impl ToColumn]) -> Self {
        self.unique_by = columns.iter().map(|c| c.to_column()).collect();
        self
    }
}

impl<T: FromRow> ToSql for Insert<T> {
    fn to_sql(&self) -> String {
        let columns = self
            .columns
            .iter()
            .map(|c| c.to_sql())
            .collect::<Vec<_>>()
            .join(", ");
        let placeholders = self
            .columns
            .iter()
            .enumerate()
            .map(|(i, _)| format!("${}", i + 1))
            .collect::<Vec<_>>()
            .join(", ");

        let no_conflict = if self.no_conflict {
            "ON CONFLICT DO NOTHING ".to_string()
        } else if !self.unique_by.is_empty() {
            let columns = self
                .unique_by
                .clone()
                .into_iter()
                .map(|c| c.unqualify())
                .collect::<Vec<_>>();
            let on_conflict = columns
                .iter()
                .map(|c| c.to_sql())
                .collect::<Vec<_>>()
                .join(", ");
            let update = columns
                .iter()
                .map(|c| format!("{} = EXCLUDED.{}", c.to_sql(), c.to_sql()))
                .collect::<Vec<_>>()
                .join(", ");
            format!("ON CONFLICT ({}) DO UPDATE SET {} ", on_conflict, update)
        } else {
            "".to_string()
        };

        format!(
            r#"INSERT INTO "{}" ({}) VALUES ({}) {}RETURNING *"#,
            self.table_name.escape(),
            columns,
            placeholders,
            no_conflict,
        )
    }
}
