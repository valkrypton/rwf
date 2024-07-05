use super::{Column, ToSql, ToValue, Value};

/// The WHERE clause of a SQL query.
#[derive(Debug, Default)]
pub struct WhereClause {
    filter: Filter,
}

#[derive(Debug, Clone)]
enum Comparison {
    /// x = 1
    Equal((Column, Value)),
    /// x IN (1, 2, 3)
    In((Column, Value)),
    /// X NOT IN (1, 2, 3)
    NotIn((Column, Value)),
    /// x <> 1
    NotEqual((Column, Value)),
    /// (x = 1 AND y = 2)
    Filter(Filter),
}

impl ToSql for Comparison {
    fn to_sql(&self) -> String {
        use Comparison::*;

        match self {
            Equal((a, b)) => format!(r#"{} = {}"#, a.to_sql(), b.to_sql()),
            In((column, value)) => format!(r#"{} = ANY({})"#, column.to_sql(), value.to_sql()),
            NotIn((column, value)) => format!(r#"{} <> ANY({})"#, column.to_sql(), value.to_sql()),
            NotEqual((column, value)) => format!(r#"{} <> {}"#, column.to_sql(), value.to_sql()),
            Filter(filter) => format!("({})", filter.to_sql()),
        }
    }
}

impl WhereClause {
    /// Add predicates to the WHERE clause using OR operator.
    pub fn or(&mut self, filter: Filter) {
        self.filter = self.filter.or(filter);
    }

    /// Add predicates to the WHERE clause using AND operator.
    pub fn and(&mut self, filter: Filter) {
        self.filter = self.filter.and(filter);
    }

    /// Add a single predicate to the WHERE clause, using the AND operator.
    pub fn add(&mut self, column: Column, value: impl ToValue) {
        self.filter.add(column, value);
    }

    /// Append all predicates of the filter into the current WHERE clause, e.g.
    /// (x = 1) "concat" (y = 2 AND z = 3) becomes (x = 1 AND y = 2 AND z = 3).
    pub fn concat(&mut self, filter: Filter) {
        self.filter = self.filter.concat(filter);
    }

    /// Remove all predicates.
    pub fn clear(&mut self) {
        self.filter.clauses.clear();
    }

    /// Clone the current filter.
    pub fn filter(&self) -> Filter {
        self.filter.clone()
    }
}

impl ToSql for WhereClause {
    fn to_sql(&self) -> String {
        if self.filter.is_empty() {
            "".to_string()
        } else {
            format!(" WHERE {}", self.filter.to_sql())
        }
    }
}

/// Type of connecting operation between two filters.
#[derive(Debug, Clone, Default, PartialEq, Copy)]
pub enum JoinOp {
    /// AND
    #[default]
    And,
    /// OR
    Or,
}

impl ToSql for JoinOp {
    fn to_sql(&self) -> String {
        use JoinOp::*;

        match self {
            And => "AND",
            Or => "OR",
        }
        .to_string()
    }
}

/// A filter to be applied using the WHERE clause.
///
/// A filter is composed of multiple predicates joined by an operator,
/// e.g. AND.
///
/// # Example
///
/// ```sql
/// WHERE x = 1 AND b = 2
/// ```
///
#[derive(Debug, Clone, Default)]
pub struct Filter {
    clauses: Vec<Comparison>,
    op: JoinOp,
}

impl Filter {
    /// Merge a filter using the OR operator, e.g.
    /// (x = 1) OR (y = 2 AND z = 3).
    pub fn or(&self, filter: Filter) -> Self {
        self.join(JoinOp::Or, filter)
    }

    /// Merge a filter using the AND operator, e.g.
    /// (x = 1) AND (y = 2 AND z = 3).
    pub fn and(&self, filter: Filter) -> Self {
        self.join(JoinOp::And, filter)
    }

    pub fn is_empty(&self) -> bool {
        self.clauses.is_empty()
    }

    /// Add a predicate to the filter, using the AND operator.
    pub fn add(&mut self, column: Column, value: impl ToValue) {
        let value = value.to_value();
        match value {
            Value::Record(value) => {
                self.clauses.push(Comparison::In((column, *value)));
            }
            value => {
                self.clauses.push(Comparison::Equal((column, value)));
            }
        }
    }

    /// Add a negated predicate to the filter, using the AND operator.
    pub fn add_not(&mut self, column: Column, value: impl ToValue) {
        let value = value.to_value();
        match value {
            Value::Record(value) => {
                self.clauses.push(Comparison::NotIn((column, *value)));
            }
            value => {
                self.clauses.push(Comparison::NotEqual((column, value)));
            }
        }
    }

    /// Append all predicates of the filter into the current filter.
    pub fn concat(&self, filter: Filter) -> Self {
        assert_eq!(self.op, filter.op);
        let mut clauses = self.clauses.clone();
        clauses.extend(filter.clauses);
        Filter {
            clauses,
            op: self.op,
        }
    }

    // pub fn rewrite_placeholders(mut self, starting_id: i32) -> Self {
    //     use Comparison::*;

    //     let clauses = self.clauses.into_iter().map(|clause| match clause {

    //     })
    // }

    fn join(&self, op: JoinOp, filter: Filter) -> Self {
        if self.is_empty() {
            filter
        } else {
            Filter {
                clauses: vec![Comparison::Filter(self.clone()), Comparison::Filter(filter)],
                op,
            }
        }
    }
}

impl ToSql for Filter {
    fn to_sql(&self) -> String {
        self.clauses
            .iter()
            .map(|s| format!("{}", s.to_sql()))
            .collect::<Vec<_>>()
            .join(&format!(" {} ", &self.op.to_sql()))
    }
}

#[cfg(test)]
mod test {
    use super::super::{Column, Value};
    use super::*;

    #[test]
    fn test_filter() {
        let filter = Filter {
            clauses: vec![
                Comparison::Equal((
                    Column::new("table_name", "column_a"),
                    Value::String("value".into()),
                )),
                Comparison::NotEqual((Column::new("table_name", "column_b"), Value::Integer(42))),
                Comparison::Filter(Filter {
                    clauses: vec![
                        Comparison::NotIn((
                            Column::new("table_x", "column_y"),
                            Value::List(vec![Value::Integer(56), Value::Integer(67)]),
                        )),
                        Comparison::Equal((
                            Column::new("table_y", "column_x"),
                            Value::String("hello".into()),
                        )),
                    ],
                    op: JoinOp::Or,
                }),
            ],
            op: JoinOp::And,
        };

        let sql = filter.to_sql();
        assert_eq!(
            sql,
            r#""table_name"."column_a" = 'value' AND "table_name"."column_b" <> 42 AND ("table_x"."column_y" <> ANY({56, 67}) OR "table_y"."column_x" = 'hello')"#
        );
    }

    #[test]
    fn test_join() {
        let a = Filter {
            clauses: vec![
                Comparison::Equal((Column::new("table", "column_a"), Value::Integer(5))),
                Comparison::NotEqual((Column::new("table", "column_a"), Value::Integer(125))),
            ],
            op: JoinOp::Or,
        };

        let b = Filter {
            clauses: vec![
                Comparison::Equal((Column::new("table", "column_b"), Value::Integer(42))),
                Comparison::NotEqual((Column::new("table", "column_b"), Value::Integer(56))),
            ],
            op: JoinOp::And,
        };

        let or = a.clone().or(b.clone());
        let and = a.and(b);

        assert_eq!(
            and.to_sql(),
            r#"("table"."column_a" = 5 OR "table"."column_a" <> 125) AND ("table"."column_b" = 42 AND "table"."column_b" <> 56)"#
        );
        assert_eq!(
            or.to_sql(),
            r#"("table"."column_a" = 5 OR "table"."column_a" <> 125) OR ("table"."column_b" = 42 AND "table"."column_b" <> 56)"#
        );
    }
}