//! Fuzzy scoring algorithm with match position tracking.

/// Result of scoring a candidate against a query.
#[derive(Debug, Clone)]
pub struct ScoredMatch {
    /// Index into the candidate list (set by the caller after scoring).
    pub index: usize,
    /// Score (higher = better match). 0 = no match.
    pub score: i32,
    /// Character indices in the candidate text that matched.
    pub positions: Vec<usize>,
}

/// Score a candidate `text` against `query`.
///
/// Rewards: consecutive matches, word-boundary matches, prefix matches.
/// Penalizes: gaps between matches.
/// Returns `None` if the query doesn't match at all.
pub fn fuzzy_score(text: &str, query: &str) -> Option<ScoredMatch> {
    if query.is_empty() {
        return Some(ScoredMatch { index: 0, score: 0, positions: vec![] });
    }

    let text_chars: Vec<char> = text.chars().collect();
    let query_chars: Vec<char> = query.chars().collect();
    let text_lower: Vec<char> = text.to_lowercase().chars().collect();
    let query_lower: Vec<char> = query.to_lowercase().chars().collect();

    // Greedy forward match to check feasibility and collect positions.
    let mut positions = Vec::with_capacity(query_lower.len());
    let mut ti = 0;
    for &qc in &query_lower {
        let mut found = false;
        while ti < text_lower.len() {
            if text_lower[ti] == qc {
                positions.push(ti);
                ti += 1;
                found = true;
                break;
            }
            ti += 1;
        }
        if !found {
            return None;
        }
    }

    // Score the match.
    let mut score: i32 = 0;

    for (qi, &pos) in positions.iter().enumerate() {
        // Base score per matched character.
        score += 10;

        // Consecutive match bonus.
        if qi > 0 && positions[qi - 1] + 1 == pos {
            score += 16;
        }

        // Word boundary bonus: match at start of word.
        if pos == 0 || !text_chars[pos - 1].is_alphanumeric() {
            score += 8;
        }

        // Exact case match bonus.
        if text_chars[pos] == query_chars[qi] {
            score += 2;
        }

        // Prefix bonus: matching the very start of text.
        if pos == 0 && qi == 0 {
            score += 15;
        }

        // Gap penalty.
        if qi > 0 {
            let gap = pos - positions[qi - 1] - 1;
            score -= gap as i32 * 3;
        }
    }

    // Shorter texts get a small bonus (prefer tight matches).
    score -= (text_chars.len() as i32 - positions.len() as i32) / 4;

    Some(ScoredMatch { index: 0, score: score.max(1), positions })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_prefix_highest() {
        let s1 = fuzzy_score("Ship docs", "ship").unwrap();
        let s2 = fuzzy_score("Worship plan", "ship").unwrap();
        let s3 = fuzzy_score("Flagship", "ship").unwrap();
        assert!(s1.score > s2.score, "prefix {} > mid-word {}", s1.score, s2.score);
        assert!(s1.score > s3.score, "prefix {} > mid-word {}", s1.score, s3.score);
    }

    #[test]
    fn no_match_returns_none() {
        assert!(fuzzy_score("hello", "xyz").is_none());
    }

    #[test]
    fn empty_query_matches_all() {
        let s = fuzzy_score("anything", "").unwrap();
        assert_eq!(s.score, 0);
        assert!(s.positions.is_empty());
    }

    #[test]
    fn consecutive_beats_scattered() {
        let s_consec = fuzzy_score("abcdef", "abc").unwrap();
        let s_scatter = fuzzy_score("a_b_c_def", "abc").unwrap();
        assert!(s_consec.score > s_scatter.score);
    }

    #[test]
    fn case_insensitive() {
        assert!(fuzzy_score("Ship Docs", "sd").is_some());
    }

    #[test]
    fn positions_tracked() {
        let s = fuzzy_score("Ship docs", "sd").unwrap();
        assert_eq!(s.positions.len(), 2);
        assert_eq!(s.positions[0], 0); // 'S'
        assert_eq!(s.positions[1], 5); // 'd'
    }
}
