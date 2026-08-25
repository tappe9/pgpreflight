use pgpreflight_core::JoinGraph;
use pgpreflight_postgres::parse_and_validate;

fn join_graph(sql: &str) -> JoinGraph {
    parse_and_validate(sql)
        .unwrap_or_else(|error| panic!("statement was rejected: {error}"))
        .facts()
        .join_graph
        .clone()
}

fn edge_pairs(graph: &JoinGraph) -> Vec<(usize, usize)> {
    let mut pairs = graph
        .edges
        .iter()
        .map(|edge| (edge.left.min(edge.right), edge.left.max(edge.right)))
        .collect::<Vec<_>>();
    pairs.sort_unstable();
    pairs.dedup();
    pairs
}

#[test]
fn qualified_where_predicate_connects_unqualified_relation_occurrences() {
    let graph = join_graph(
        "SELECT * FROM accounts AS a CROSS JOIN orders AS o \
         WHERE a.id = o.account_id",
    );

    assert!(!graph.indeterminate);
    assert_eq!(graph.relation_occurrences.len(), 2);
    assert_eq!(graph.relation_occurrences[0].relation.name, "accounts");
    assert_eq!(graph.relation_occurrences[0].alias.as_deref(), Some("a"));
    assert_eq!(graph.relation_occurrences[1].relation.name, "orders");
    assert_eq!(graph.relation_occurrences[1].alias.as_deref(), Some("o"));
    assert_eq!(edge_pairs(&graph), vec![(0, 1)]);
}

#[test]
fn cross_join_and_on_true_do_not_create_edges() {
    for sql in [
        "SELECT * FROM public.accounts AS a CROSS JOIN public.orders AS o",
        "SELECT * FROM public.accounts AS a JOIN public.orders AS o ON TRUE",
    ] {
        let graph = join_graph(sql);

        assert!(!graph.indeterminate, "{sql}");
        assert_eq!(graph.relation_occurrences.len(), 2, "{sql}");
        assert!(graph.edges.is_empty(), "{sql}");
    }
}

#[test]
fn qualified_on_using_and_natural_join_create_edges() {
    for sql in [
        "SELECT * FROM accounts AS a JOIN orders AS o ON a.id = o.account_id",
        "SELECT * FROM accounts AS a JOIN orders AS o USING (account_id)",
        "SELECT * FROM accounts AS a NATURAL JOIN orders AS o",
    ] {
        let graph = join_graph(sql);

        assert!(!graph.indeterminate, "{sql}");
        assert_eq!(graph.relation_occurrences.len(), 2, "{sql}");
        assert_eq!(edge_pairs(&graph), vec![(0, 1)], "{sql}");
    }
}

#[test]
fn qualified_single_relation_filters_do_not_create_edges() {
    let graph = join_graph(
        "SELECT * FROM accounts AS a CROSS JOIN orders AS o \
         WHERE a.active = TRUE AND o.active = TRUE",
    );

    assert!(!graph.indeterminate);
    assert_eq!(graph.relation_occurrences.len(), 2);
    assert!(graph.edges.is_empty());
}

#[test]
fn later_predicates_connect_groups_and_self_join_aliases_remain_distinct() {
    let graph = join_graph(
        "SELECT * FROM accounts AS a \
         CROSS JOIN orders AS o \
         CROSS JOIN line_items AS li \
         WHERE a.id = o.account_id AND o.id = li.order_id",
    );

    assert!(!graph.indeterminate);
    assert_eq!(graph.relation_occurrences.len(), 3);
    assert_eq!(edge_pairs(&graph), vec![(0, 1), (1, 2)]);

    let self_join = join_graph(
        "SELECT * FROM nodes AS parent \
         CROSS JOIN nodes AS child \
         WHERE parent.id = child.parent_id",
    );

    assert!(!self_join.indeterminate);
    assert_eq!(self_join.relation_occurrences.len(), 2);
    assert_eq!(self_join.relation_occurrences[0].relation.name, "nodes");
    assert_eq!(self_join.relation_occurrences[1].relation.name, "nodes");
    assert_eq!(
        self_join.relation_occurrences[0].alias.as_deref(),
        Some("parent")
    );
    assert_eq!(
        self_join.relation_occurrences[1].alias.as_deref(),
        Some("child")
    );
    assert_eq!(edge_pairs(&self_join), vec![(0, 1)]);
}

#[test]
fn later_on_predicates_connect_existing_groups_in_deterministic_edge_order() {
    let graph = join_graph(
        "SELECT * FROM accounts AS a \
         CROSS JOIN orders AS o \
         JOIN line_items AS li \
           ON a.id = li.account_id AND o.id = li.order_id",
    );

    assert!(!graph.indeterminate);
    assert_eq!(graph.relation_occurrences.len(), 3);
    assert_eq!(
        graph
            .edges
            .iter()
            .map(|edge| (edge.left, edge.right))
            .collect::<Vec<_>>(),
        vec![(0, 2), (1, 2)]
    );
}

#[test]
fn unqualified_ambiguous_lateral_correlated_and_or_cases_are_indeterminate() {
    for sql in [
        "SELECT * FROM accounts AS a CROSS JOIN orders AS o WHERE id = o.account_id",
        "SELECT * FROM accounts AS x CROSS JOIN orders AS x WHERE x.id = x.account_id",
        "SELECT * FROM accounts AS a CROSS JOIN LATERAL (SELECT a.id) AS x",
        "SELECT * FROM accounts AS a WHERE EXISTS (\
             SELECT 1 FROM orders AS o WHERE o.account_id = a.id\
         )",
        "SELECT * FROM accounts AS a CROSS JOIN orders AS o \
         WHERE a.id = o.account_id OR a.status = 'active'",
    ] {
        let graph = join_graph(sql);
        assert!(graph.indeterminate, "{sql}");
    }
}

#[test]
fn supported_update_from_and_delete_using_build_join_graphs() {
    for sql in [
        "UPDATE accounts AS a SET status = 'done' \
         FROM orders AS o WHERE a.id = o.account_id",
        "DELETE FROM accounts AS a USING orders AS o \
         WHERE a.id = o.account_id",
    ] {
        let graph = join_graph(sql);

        assert!(!graph.indeterminate, "{sql}");
        assert_eq!(graph.relation_occurrences.len(), 2, "{sql}");
        assert_eq!(
            graph
                .relation_occurrences
                .iter()
                .map(|occurrence| occurrence.alias.as_deref())
                .collect::<Vec<_>>(),
            vec![Some("a"), Some("o")],
            "{sql}"
        );
        assert_eq!(edge_pairs(&graph), vec![(0, 1)], "{sql}");
    }
}

#[test]
fn disconnected_supported_update_from_and_delete_using_remain_edge_free() {
    for sql in [
        "UPDATE accounts AS a SET status = 'done' \
         FROM orders AS o WHERE a.active = TRUE AND o.active = TRUE",
        "DELETE FROM accounts AS a USING orders AS o \
         WHERE a.active = TRUE AND o.active = TRUE",
    ] {
        let graph = join_graph(sql);

        assert!(!graph.indeterminate, "{sql}");
        assert_eq!(graph.relation_occurrences.len(), 2, "{sql}");
        assert!(graph.edges.is_empty(), "{sql}");
    }
}
