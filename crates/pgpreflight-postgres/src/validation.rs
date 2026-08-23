use std::{fmt, ops::ControlFlow};

use pgpreflight_core::{JoinGraph, RelationRef, StatementFacts, StatementKind};
use sqlparser::ast::{
    Delete, FromTable, ObjectName, Query, SetExpr, Statement, TableFactor, Update, Visit, Visitor,
};

use crate::CheckError;

pub struct ValidatedStatement {
    // Consumed by the PostgreSQL planning adapter in the next implementation slice.
    #[allow(dead_code)]
    statement: Statement,
    facts: StatementFacts,
}

impl fmt::Debug for ValidatedStatement {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ValidatedStatement")
            .field("facts", &self.facts)
            .finish_non_exhaustive()
    }
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
        Statement::Query(query) => select_facts(query)?,
        Statement::Update(update) => update_facts(update)?,
        Statement::Delete(delete) => delete_facts(delete)?,
        Statement::Explain { .. } => return Err(CheckError::UnsafeConstruct { kind: "EXPLAIN" }),
        _ => return Err(CheckError::UnsupportedStatement),
    };

    Ok(ValidatedStatement { statement, facts })
}

fn select_facts(query: &Query) -> Result<StatementFacts, CheckError> {
    ensure_select_query_body(&query.body)?;

    Ok(StatementFacts {
        kind: StatementKind::Select,
        target_relation: None,
        has_where: query_body_has_where(&query.body),
        has_returning: false,
        join_graph: JoinGraph::default(),
    })
}

fn update_facts(update: &Update) -> Result<StatementFacts, CheckError> {
    Ok(StatementFacts {
        kind: StatementKind::Update,
        target_relation: relation_from_factor(&update.table.relation)?,
        has_where: update.selection.is_some(),
        has_returning: update.returning.is_some(),
        join_graph: JoinGraph::default(),
    })
}

fn delete_facts(delete: &Delete) -> Result<StatementFacts, CheckError> {
    let tables = match &delete.from {
        FromTable::WithFromKeyword(tables) | FromTable::WithoutKeyword(tables) => tables,
    };

    Ok(StatementFacts {
        kind: StatementKind::Delete,
        target_relation: tables
            .first()
            .map(|table| relation_from_factor(&table.relation))
            .transpose()?
            .flatten(),
        has_where: delete.selection.is_some(),
        has_returning: delete.returning.is_some(),
        join_graph: JoinGraph::default(),
    })
}

fn relation_from_factor(factor: &TableFactor) -> Result<Option<RelationRef>, CheckError> {
    match factor {
        TableFactor::Table { name, .. } => relation_from_name(name),
        _ => Ok(None),
    }
}

fn relation_from_name(name: &ObjectName) -> Result<Option<RelationRef>, CheckError> {
    let identifiers = name
        .0
        .iter()
        .map(|part| part.as_ident())
        .collect::<Option<Vec<_>>>()
        .ok_or(CheckError::UnsupportedStatement)?;

    match identifiers.as_slice() {
        [_relation] => Ok(None),
        [schema, relation] => Ok(Some(RelationRef::new(
            schema.value.clone(),
            relation.value.clone(),
        ))),
        _ => Err(CheckError::UnsupportedStatement),
    }
}

fn ensure_select_query_body(expr: &SetExpr) -> Result<(), CheckError> {
    match expr {
        SetExpr::Select(_) => Ok(()),
        SetExpr::Query(query) => ensure_select_query_body(&query.body),
        SetExpr::SetOperation { left, right, .. } => {
            ensure_select_query_body(left)?;
            ensure_select_query_body(right)
        }
        _ => Err(CheckError::UnsupportedStatement),
    }
}

fn query_body_has_where(expr: &SetExpr) -> bool {
    match expr {
        SetExpr::Select(select) => select.selection.is_some(),
        SetExpr::Query(query) => query_body_has_where(&query.body),
        SetExpr::SetOperation { left, right, .. } => {
            query_body_has_where(left) || query_body_has_where(right)
        }
        _ => false,
    }
}

fn set_expr_contains_select_into(expr: &SetExpr) -> bool {
    match expr {
        SetExpr::Select(select) => select.into.is_some(),
        SetExpr::Query(query) => set_expr_contains_select_into(&query.body),
        SetExpr::SetOperation { left, right, .. } => {
            set_expr_contains_select_into(left) || set_expr_contains_select_into(right)
        }
        _ => false,
    }
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
            if set_expr_contains_select_into(&query.body) {
                return ControlFlow::Break("SELECT INTO");
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
