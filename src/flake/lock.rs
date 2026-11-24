use super::Input;

#[derive(Debug, Clone, PartialEq)]
pub struct RevChange {
    pub name: String,
    pub old: Option<String>,
    pub new: Option<String>,
}

pub fn rev_changes(before: &[Input], after: &[Input]) -> Vec<RevChange> {
    after
        .iter()
        .filter_map(|input| {
            let old = before
                .iter()
                .find(|b| b.name == input.name)
                .and_then(|b| b.rev.clone());
            (old != input.rev).then(|| RevChange {
                name: input.name.clone(),
                old,
                new: input.rev.clone(),
            })
        })
        .collect()
}

fn short(rev: &Option<String>) -> &str {
    rev.as_deref()
        .map(|r| r.get(..7).unwrap_or(r))
        .unwrap_or("unlocked")
}

pub fn update_summary(changes: &[RevChange]) -> String {
    if changes.is_empty() {
        return "All inputs are already up to date".to_string();
    }
    changes
        .iter()
        .map(|c| format!("{}: {} → {}", c.name, short(&c.old), short(&c.new)))
        .collect::<Vec<_>>()
        .join("\n")
}
