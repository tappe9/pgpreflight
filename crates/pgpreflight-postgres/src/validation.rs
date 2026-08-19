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
    ensure_no_unsafe_queries(&statement)?;

    let facts = match &statement {
        Statement::Query(_) => select_facts(),
        Statement::Update(update) => update_facts(update),
        Statement::Delete(delete) => delete_facts(delete),
        Statement::Explain { .. } => return Err(CheckError::UnsafeConstruct { kind: "EXPLAIN" }),
        _ => return Err(CheckError::UnsupportedStatement),
    };

    Ok(ValidatedStatement { statement, facts })
}

fn select_facts() -> StatementFacts {
    StatementFacts {
        kind: StatementKind::Select,
        target_relation: None,
        has_where: false,
        has_returning: false,
        join_graph: JoinGraph::default(),
    }
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

fn query_contains_modification(query: &Query) -> bool {
    contains_modifying_set_expr(&query.body)
        || query.with.as_ref().is_some_and(|with| {
            with.cte_tables
                .iter()
                .any(|cte| query_contains_modification(&cte.query))
        })
}

fn contains_modifying_set_expr(expr: &SetExpr) -> bool {
    match expr {
        SetExpr::Insert(_) | SetExpr::Update(_) | SetExpr::Delete(_) | SetExpr::Merge(_) => true,
        SetExpr::Query(query) => query_contains_modification(query),
        SetExpr::SetOperation { left, right, .. } => {
            contains_modifying_set_expr(left) || contains_modifying_set_expr(right)
        }
        _ => false,
    }
}

fn ensure_no_unsafe_queries(statement: &Statement) -> Result<(), CheckError> {
    #[derive(Default)]
    struct SafetyVisitor;

    impl Visitor for SafetyVisitor {
        type Break = &'static str;

        fn pre_visit_query(&mut self, query: &Query) -> ControlFlow<Self::Break> {
            if !query.locks.is_empty() {
                return ControlFlow::Break("locking clause");
            }
            if query_contains_modification(query) {
                return ControlFlow::Break("data-modifying query");
            }
            ControlFlow::Continue(())
        }
    }

    let mut visitor = SafetyVisitor;
    match statement.visit(&mut visitor) {
        ControlFlow::Continue(()) => Ok(()),
        ControlFlow::Break(kind) => Err(CheckError::UnsafeConstruct { kind }),
    }
}
