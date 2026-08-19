use std::ops::ControlFlow;

use pgpreflight_core::{JoinGraph, RelationRef, StatementFacts, StatementKind};
use sqlparser::ast::{
    Delete, FromTable, ObjectName, Query, SetExpr, Statement, TableFactor, Update, Visit, Visitor,
};

use crate::CheckError;

#[derive(Debug)]
pub struct ValidatedStatement {
    // Consumed by the PostgreSQL planning adapter in the next implementation slice.
    #[allow(dead_code)]
    statement: Statement,
    facts: StatementFacts,
}

impl ValidatedStatement {
    pub fn facts(&self) -> &StatementFacts {
        &self.facts
    }

    // Kept crate-private so sqlparser AST types never become part of the public API.
    #[allow(dead_code)]
    pub(crate) fn statement(&self) -> &Statement {
        &self.statement
    }
}

pub(crate) fn validate_statement(statement: Statement) -> Result<ValidatedStatement, CheckError> {
    ensure_no_locking_queries(&statement)?;

    let facts = match &statement {
        Statement::Query(query) => validate_query(query)?,
        Statement::Update(update) => update_facts(update),
        Statement::Delete(delete) => delete_facts(delete),
        Statement::Explain { .. } => return Err(CheckError::UnsafeConstruct { kind: "EXPLAIN" }),
        _ => return Err(CheckError::UnsupportedStatement),
    };

    Ok(ValidatedStatement { statement, facts })
}

fn validate_query(query: &Query) -> Result<StatementFacts, CheckError> {
    if contains_modifying_set_expr(&query.body) {
        return Err(CheckError::UnsafeConstruct {
            kind: "data-modifying query body",
        });
    }

    if let Some(with) = &query.with {
        for cte in &with.cte_tables {
            if contains_modifying_set_expr(&cte.query.body) {
                return Err(CheckError::UnsafeConstruct {
                    kind: "data-modifying CTE",
                });
            }
        }
    }

    Ok(StatementFacts {
        kind: StatementKind::Select,
        target_relation: None,
        has_where: false,
        has_returning: false,
        join_graph: JoinGraph::default(),
    })
}

fn update_facts(update: &Update) -> StatementFacts {
    StatementFacts {
        kind: StatementKind::Update,
        target_relation: relation_from_factor(&update.table.relation),
        has_where: update.selection.is_some(),
        has_returning: update.returning.is_some(),
        join_graph: JoinGraph::default(),
    }
}

fn delete_facts(delete: &Delete) -> StatementFacts {
    let tables = match &delete.from {
        FromTable::WithFromKeyword(tables) | FromTable::WithoutKeyword(tables) => tables,
    };

    StatementFacts {
        kind: StatementKind::Delete,
        target_relation: tables
            .first()
            .and_then(|table| relation_from_factor(&table.relation)),
        has_where: delete.selection.is_some(),
        has_returning: delete.returning.is_some(),
        join_graph: JoinGraph::default(),
    }
}

fn relation_from_factor(factor: &TableFactor) -> Option<RelationRef> {
    match factor {
        TableFactor::Table { name, .. } => Some(relation_from_name(name)),
        _ => None,
    }
}

fn relation_from_name(name: &ObjectName) -> RelationRef {
    let rendered = name.to_string();
    let mut parts = rendered.rsplitn(2, '.');
    let relation = parts.next().unwrap_or(&rendered).trim_matches('"');
    let schema = parts.next().unwrap_or("public").trim_matches('"');
    RelationRef::new(schema, relation)
}

fn contains_modifying_set_expr(expr: &SetExpr) -> bool {
    match expr {
        SetExpr::Insert(_) | SetExpr::Update(_) => true,
        SetExpr::Query(query) => contains_modifying_set_expr(&query.body),
        SetExpr::SetOperation { left, right, .. } => {
            contains_modifying_set_expr(left) || contains_modifying_set_expr(right)
        }
        _ => false,
    }
}

fn ensure_no_locking_queries(statement: &Statement) -> Result<(), CheckError> {
    #[derive(Default)]
    struct LockVisitor;

    impl Visitor for LockVisitor {
        type Break = ();

        fn pre_visit_query(&mut self, query: &Query) -> ControlFlow<Self::Break> {
            if query.locks.is_empty() {
                ControlFlow::Continue(())
            } else {
                ControlFlow::Break(())
            }
        }
    }

    let mut visitor = LockVisitor;
    match statement.visit(&mut visitor) {
        ControlFlow::Continue(()) => Ok(()),
        ControlFlow::Break(()) => Err(CheckError::UnsafeConstruct {
            kind: "locking clause",
        }),
    }
}
