use std::{
    collections::{BTreeMap, BTreeSet},
    ops::ControlFlow,
};

use pgpreflight_core::{JoinEdge, JoinGraph, RelationIdentity, RelationOccurrence};
use sqlparser::ast::{
    BinaryOperator, Delete, Expr, FromTable, JoinConstraint, JoinOperator, ObjectName, Query,
    SetExpr, TableFactor, TableWithJoins, UnaryOperator, Update, UpdateTableFromKind, Visit,
    Visitor,
};

pub(crate) fn select_join_graph(query: &Query) -> JoinGraph {
    if query.with.is_some() || !query.pipe_operators.is_empty() {
        return indeterminate_graph();
    }

    match query.body.as_ref() {
        SetExpr::Select(select) => {
            build_join_graph(select.from.iter().collect(), select.selection.as_ref())
        }
        SetExpr::Query(query) => select_join_graph(query),
        _ => indeterminate_graph(),
    }
}

pub(crate) fn update_join_graph(update: &Update) -> JoinGraph {
    let mut tables = vec![&update.table];

    if let Some(from) = &update.from {
        let from_tables = match from {
            UpdateTableFromKind::BeforeSet(tables) | UpdateTableFromKind::AfterSet(tables) => {
                tables
            }
        };
        tables.extend(from_tables);
    }

    build_join_graph(tables, update.selection.as_ref())
}

pub(crate) fn delete_join_graph(delete: &Delete) -> JoinGraph {
    let from_tables = match &delete.from {
        FromTable::WithFromKeyword(tables) | FromTable::WithoutKeyword(tables) => tables,
    };
    let mut tables = from_tables.iter().collect::<Vec<_>>();

    if let Some(using) = &delete.using {
        tables.extend(using);
    }

    build_join_graph(tables, delete.selection.as_ref())
}

fn indeterminate_graph() -> JoinGraph {
    JoinGraph {
        indeterminate: true,
        ..JoinGraph::default()
    }
}

fn build_join_graph(tables: Vec<&TableWithJoins>, selection: Option<&Expr>) -> JoinGraph {
    let mut builder = JoinGraphBuilder::default();

    for table in &tables {
        builder.register_table_with_joins(table);
    }
    for table in tables {
        builder.process_table_with_joins(table);
    }

    if let Some(selection) = selection {
        builder.process_predicate(selection, None);
    }

    builder.finish()
}

#[derive(Default)]
struct JoinGraphBuilder {
    relation_occurrences: Vec<RelationOccurrence>,
    qualifiers: BTreeMap<Vec<String>, Vec<usize>>,
    factor_scopes: Vec<Vec<usize>>,
    next_scope: usize,
    edges: BTreeSet<(usize, usize)>,
    indeterminate: bool,
}

impl JoinGraphBuilder {
    fn register_table_with_joins(&mut self, table: &TableWithJoins) {
        self.register_factor(&table.relation);
        for join in &table.joins {
            self.register_factor(&join.relation);
        }
    }

    fn register_factor(&mut self, factor: &TableFactor) {
        let scope = match factor {
            TableFactor::Table {
                name,
                alias,
                args,
                with_hints,
                version,
                with_ordinality,
                partitions,
                json_path,
                sample,
                index_hints,
            } if args.is_none()
                && with_hints.is_empty()
                && version.is_none()
                && !*with_ordinality
                && partitions.is_empty()
                && json_path.is_none()
                && sample.is_none()
                && index_hints.is_empty() =>
            {
                match relation_identity(name) {
                    Some(relation) => {
                        let index = self.relation_occurrences.len();
                        let alias = alias.as_ref().map(|alias| alias.name.value.clone());
                        self.register_qualifiers(index, &relation, alias.as_deref());
                        self.relation_occurrences
                            .push(RelationOccurrence { relation, alias });
                        vec![index]
                    }
                    None => {
                        self.indeterminate = true;
                        Vec::new()
                    }
                }
            }
            _ => {
                self.indeterminate = true;
                Vec::new()
            }
        };

        self.factor_scopes.push(scope);
    }

    fn register_qualifiers(
        &mut self,
        index: usize,
        relation: &RelationIdentity,
        alias: Option<&str>,
    ) {
        if let Some(alias) = alias {
            self.qualifiers
                .entry(vec![alias.to_owned()])
                .or_default()
                .push(index);
            return;
        }

        self.qualifiers
            .entry(vec![relation.name.clone()])
            .or_default()
            .push(index);

        if let Some(schema) = &relation.schema {
            self.qualifiers
                .entry(vec![schema.clone(), relation.name.clone()])
                .or_default()
                .push(index);
        }
    }

    fn process_table_with_joins(&mut self, table: &TableWithJoins) {
        let mut left_scope = self.take_factor_scope();

        for join in &table.joins {
            let right_scope = self.take_factor_scope();
            self.process_join_operator(&join.join_operator, &left_scope, &right_scope);
            left_scope.extend(right_scope);
        }
    }

    fn take_factor_scope(&mut self) -> Vec<usize> {
        let scope = self.factor_scopes.get(self.next_scope).cloned();
        self.next_scope += 1;

        match scope {
            Some(scope) => scope,
            None => {
                self.indeterminate = true;
                Vec::new()
            }
        }
    }

    fn process_join_operator(
        &mut self,
        operator: &JoinOperator,
        left_scope: &[usize],
        right_scope: &[usize],
    ) {
        match operator {
            JoinOperator::Join(constraint)
            | JoinOperator::Inner(constraint)
            | JoinOperator::Left(constraint)
            | JoinOperator::LeftOuter(constraint)
            | JoinOperator::Right(constraint)
            | JoinOperator::RightOuter(constraint)
            | JoinOperator::FullOuter(constraint) => {
                self.process_join_constraint(constraint, left_scope, right_scope);
            }
            JoinOperator::CrossJoin(constraint) => {
                if !matches!(constraint, JoinConstraint::None) {
                    self.indeterminate = true;
                }
            }
            JoinOperator::Semi(_)
            | JoinOperator::LeftSemi(_)
            | JoinOperator::RightSemi(_)
            | JoinOperator::Anti(_)
            | JoinOperator::LeftAnti(_)
            | JoinOperator::RightAnti(_)
            | JoinOperator::CrossApply
            | JoinOperator::OuterApply
            | JoinOperator::AsOf { .. }
            | JoinOperator::StraightJoin(_)
            | JoinOperator::ArrayJoin
            | JoinOperator::LeftArrayJoin
            | JoinOperator::InnerArrayJoin => {
                self.indeterminate = true;
            }
        }
    }

    fn process_join_constraint(
        &mut self,
        constraint: &JoinConstraint,
        left_scope: &[usize],
        right_scope: &[usize],
    ) {
        match constraint {
            JoinConstraint::On(predicate) => {
                let allowed = left_scope
                    .iter()
                    .chain(right_scope)
                    .copied()
                    .collect::<BTreeSet<_>>();
                self.process_predicate(predicate, Some(&allowed));
            }
            JoinConstraint::Using(columns) => {
                if columns.is_empty() {
                    self.indeterminate = true;
                } else {
                    self.connect_join_operands(left_scope, right_scope);
                }
            }
            JoinConstraint::Natural => self.connect_join_operands(left_scope, right_scope),
            JoinConstraint::None => {}
        }
    }

    fn connect_join_operands(&mut self, left_scope: &[usize], right_scope: &[usize]) {
        if left_scope.is_empty()
            || right_scope.is_empty()
            || !self.scope_is_connected(left_scope)
            || !self.scope_is_connected(right_scope)
        {
            self.indeterminate = true;
            return;
        }

        self.add_edge(left_scope[0], right_scope[0]);
    }

    fn scope_is_connected(&self, scope: &[usize]) -> bool {
        if scope.len() <= 1 {
            return !scope.is_empty();
        }

        let allowed = scope.iter().copied().collect::<BTreeSet<_>>();
        let mut visited = BTreeSet::new();
        let mut stack = vec![scope[0]];

        while let Some(current) = stack.pop() {
            if !visited.insert(current) {
                continue;
            }

            for &(left, right) in &self.edges {
                let neighbor = if left == current {
                    Some(right)
                } else if right == current {
                    Some(left)
                } else {
                    None
                };
                let Some(neighbor) = neighbor else {
                    continue;
                };

                if allowed.contains(&neighbor) && !visited.contains(&neighbor) {
                    stack.push(neighbor);
                }
            }
        }

        visited == allowed
    }

    fn process_predicate(&mut self, predicate: &Expr, allowed: Option<&BTreeSet<usize>>) {
        match predicate {
            Expr::BinaryOp {
                left,
                op: BinaryOperator::And,
                right,
            } => {
                self.process_predicate(left, allowed);
                self.process_predicate(right, allowed);
            }
            Expr::BinaryOp {
                op: BinaryOperator::Or,
                ..
            }
            | Expr::UnaryOp {
                op: UnaryOperator::Not,
                ..
            } => {
                self.indeterminate = true;
            }
            Expr::Nested(predicate) => self.process_predicate(predicate, allowed),
            _ => self.process_predicate_atom(predicate, allowed),
        }
    }

    fn process_predicate_atom(&mut self, predicate: &Expr, allowed: Option<&BTreeSet<usize>>) {
        let mut visitor = PredicateOwnerVisitor {
            qualifiers: &self.qualifiers,
            allowed,
            owners: BTreeSet::new(),
            indeterminate: false,
        };
        let _ = predicate.visit(&mut visitor);

        if visitor.indeterminate {
            self.indeterminate = true;
            return;
        }

        let owners = visitor.owners.into_iter().collect::<Vec<_>>();
        for pair in owners.windows(2) {
            self.add_edge(pair[0], pair[1]);
        }
    }

    fn add_edge(&mut self, left: usize, right: usize) {
        if left == right {
            return;
        }

        self.edges.insert((left.min(right), left.max(right)));
    }

    fn finish(mut self) -> JoinGraph {
        if self.next_scope != self.factor_scopes.len() {
            self.indeterminate = true;
        }

        JoinGraph {
            relation_occurrences: self.relation_occurrences,
            edges: self
                .edges
                .into_iter()
                .map(|(left, right)| JoinEdge { left, right })
                .collect(),
            indeterminate: self.indeterminate,
        }
    }
}

struct PredicateOwnerVisitor<'a> {
    qualifiers: &'a BTreeMap<Vec<String>, Vec<usize>>,
    allowed: Option<&'a BTreeSet<usize>>,
    owners: BTreeSet<usize>,
    indeterminate: bool,
}

impl Visitor for PredicateOwnerVisitor<'_> {
    type Break = ();

    fn pre_visit_query(&mut self, _query: &Query) -> ControlFlow<Self::Break> {
        self.indeterminate = true;
        ControlFlow::Break(())
    }

    fn pre_visit_expr(&mut self, expr: &Expr) -> ControlFlow<Self::Break> {
        match expr {
            Expr::Identifier(_)
            | Expr::QualifiedWildcard(..)
            | Expr::BinaryOp {
                op: BinaryOperator::And | BinaryOperator::Or,
                ..
            } => {
                self.indeterminate = true;
                ControlFlow::Break(())
            }
            Expr::CompoundIdentifier(parts) => {
                let qualifier = parts
                    .get(..parts.len().saturating_sub(1))
                    .filter(|parts| !parts.is_empty())
                    .map(|parts| {
                        parts
                            .iter()
                            .map(|identifier| identifier.value.clone())
                            .collect::<Vec<_>>()
                    });

                let Some(qualifier) = qualifier else {
                    self.indeterminate = true;
                    return ControlFlow::Break(());
                };
                let Some(owners) = self.qualifiers.get(&qualifier) else {
                    self.indeterminate = true;
                    return ControlFlow::Break(());
                };
                let [owner] = owners.as_slice() else {
                    self.indeterminate = true;
                    return ControlFlow::Break(());
                };

                if self.allowed.is_some_and(|allowed| !allowed.contains(owner)) {
                    self.indeterminate = true;
                    return ControlFlow::Break(());
                }

                self.owners.insert(*owner);
                ControlFlow::Continue(())
            }
            _ => ControlFlow::Continue(()),
        }
    }
}

fn relation_identity(name: &ObjectName) -> Option<RelationIdentity> {
    let identifiers = name
        .0
        .iter()
        .map(|part| part.as_ident())
        .collect::<Option<Vec<_>>>()?;

    match identifiers.as_slice() {
        [relation] => Some(RelationIdentity::unqualified(relation.value.clone())),
        [schema, relation] => Some(RelationIdentity::qualified(
            schema.value.clone(),
            relation.value.clone(),
        )),
        _ => None,
    }
}
