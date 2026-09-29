use models::filters::SortOrder;

pub fn sql_placeholders(n: usize) -> String {
    vec!["?"; n].join(",")
}

pub fn sql_pair_placeholders(n: usize) -> String {
    vec!["(?,?)"; n].join(",")
}

pub fn sql_sort_dir(order: &Option<SortOrder>) -> &'static str {
    if matches!(order, Some(SortOrder::Desc)) { "DESC" } else { "ASC" }
}

/// Values past i64::MAX aren't integers to SQLite, so they're capped there
/// (which is "everything" either way).
pub fn sql_limit_offset(limit: Option<usize>, skip: Option<usize>) -> String {
    let cap = |n: usize| n.min(i64::MAX as usize);
    let mut s = String::new();
    match limit {
        Some(l) => s.push_str(&format!(" LIMIT {}", cap(l))),
        None => s.push_str(" LIMIT 1"),
    }
    if let Some(sk) = skip {
        s.push_str(&format!(" OFFSET {}", cap(sk)));
    }
    s
}
