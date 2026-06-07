use crate::state::domain::SearchResult;

pub fn sort_search_results(results: &mut Vec<SearchResult>, query: &str) {
    let query_lower = query.to_lowercase();
    results.sort_by(|a, b| {
        let a_name_lower = a.name.to_lowercase();
        let b_name_lower = b.name.to_lowercase();

        let a_exact = a_name_lower == query_lower;
        let b_exact = b_name_lower == query_lower;
        if a_exact != b_exact {
            return if a_exact {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }

        let a_starts = a_name_lower.starts_with(&query_lower);
        let b_starts = b_name_lower.starts_with(&query_lower);
        if a_starts != b_starts {
            return if a_starts {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }

        let a_contains = a_name_lower.contains(&query_lower);
        let b_contains = b_name_lower.contains(&query_lower);
        if a_contains != b_contains {
            return if a_contains {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }

        let a_desc_contains = a.description.to_lowercase().contains(&query_lower);
        let b_desc_contains = b.description.to_lowercase().contains(&query_lower);
        if a_desc_contains != b_desc_contains {
            return if a_desc_contains {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }

        a_name_lower.cmp(&b_name_lower)
    });
}
