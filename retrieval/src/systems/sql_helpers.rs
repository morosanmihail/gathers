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

pub fn sql_limit_offset(limit: Option<usize>, skip: Option<usize>) -> String {
    let mut s = String::new();
    match limit {
        Some(l) => s.push_str(&format!(" LIMIT {l}")),
        None => s.push_str(" LIMIT 1"),
    }
    if let Some(sk) = skip {
        s.push_str(&format!(" OFFSET {sk}"));
    }
    s
}
