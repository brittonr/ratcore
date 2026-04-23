//! Fuzzy scoring algorithm with match position tracking.

const EMPTY_SCORE: i32 = 0;
const MINIMUM_MATCH_SCORE: i32 = 1;
const MATCHED_CHARACTER_SCORE: i32 = 10;
const CONSECUTIVE_MATCH_BONUS: i32 = 16;
const WORD_BOUNDARY_BONUS: i32 = 8;
const EXACT_CASE_BONUS: i32 = 2;
const PREFIX_BONUS: i32 = 15;
const GAP_PENALTY_PER_CHARACTER: i32 = 3;
const TIGHT_MATCH_DIVISOR: i32 = 4;
const FIRST_POSITION: usize = 0;
const NEXT_POSITION_OFFSET: usize = 1;

fn i32_from_usize(value: usize) -> Option<i32> {
    i32::try_from(value).ok()
}

/// Input for [`fuzzy_score`].
#[derive(Clone, Copy)]
pub struct FuzzyScoreInput<'a> {
    /// Candidate text to score.
    pub text: &'a str,
    /// Query text to match against the candidate.
    pub query: &'a str,
}

/// Result of scoring a candidate against a query.
#[derive(Debug, Clone)]
pub struct ScoredMatch {
    /// Index into the candidate list, set by the caller after scoring.
    pub index: usize,
    /// Score where higher values represent better matches.
    pub score: i32,
    /// Character indices in the candidate text that matched.
    pub positions: Vec<usize>,
}

/// Scores a candidate `text` against `query`.
///
/// Rewards consecutive matches, word-boundary matches, and prefix matches.
/// Penalizes gaps between matches. Returns `None` when the query does not
/// match at all.
#[must_use]
pub fn fuzzy_score(input: FuzzyScoreInput<'_>) -> Option<ScoredMatch> {
    let FuzzyScoreInput { text, query } = input;
    if query.is_empty() {
        return Some(ScoredMatch {
            index: 0,
            score: EMPTY_SCORE,
            positions: vec![],
        });
    }

    let text_chars: Vec<char> = text.chars().collect();
    let query_chars: Vec<char> = query.chars().collect();
    let text_lower: Vec<char> = text.to_lowercase().chars().collect();
    let query_lower: Vec<char> = query.to_lowercase().chars().collect();

    let mut positions = Vec::with_capacity(query_lower.len());
    let mut text_index = FIRST_POSITION;
    for &query_character in &query_lower {
        let mut is_match_found = false;
        while text_index < text_lower.len() {
            if text_lower[text_index] == query_character {
                positions.push(text_index);
                text_index = text_index.saturating_add(NEXT_POSITION_OFFSET);
                is_match_found = true;
                break;
            }
            text_index = text_index.saturating_add(NEXT_POSITION_OFFSET);
        }
        if !is_match_found {
            return None;
        }
    }

    debug_assert_eq!(positions.len(), query_lower.len());
    debug_assert!(positions.windows(2).all(|window| window[0] < window[1]));

    let mut score = EMPTY_SCORE;
    for (query_index, &position) in positions.iter().enumerate() {
        score += MATCHED_CHARACTER_SCORE;

        if query_index > FIRST_POSITION {
            let previous_query_index = query_index.saturating_sub(NEXT_POSITION_OFFSET);
            let previous_position = positions[previous_query_index];
            if previous_position.saturating_add(NEXT_POSITION_OFFSET) == position {
                score += CONSECUTIVE_MATCH_BONUS;
            }
        }

        if position == FIRST_POSITION
            || !text_chars[position.saturating_sub(NEXT_POSITION_OFFSET)].is_alphanumeric()
        {
            score += WORD_BOUNDARY_BONUS;
        }

        if text_chars[position] == query_chars[query_index] {
            score += EXACT_CASE_BONUS;
        }

        if position == FIRST_POSITION && query_index == FIRST_POSITION {
            score += PREFIX_BONUS;
        }

        if query_index > FIRST_POSITION {
            let previous_query_index = query_index.saturating_sub(NEXT_POSITION_OFFSET);
            let previous_position = positions[previous_query_index];
            let gap = position
                .saturating_sub(previous_position)
                .saturating_sub(NEXT_POSITION_OFFSET);
            score -= i32_from_usize(gap)? * GAP_PENALTY_PER_CHARACTER;
        }
    }

    let unmatched_characters = text_chars.len().saturating_sub(positions.len());
    score -= i32_from_usize(unmatched_characters)? / TIGHT_MATCH_DIVISOR;

    Some(ScoredMatch {
        index: 0,
        score: score.max(MINIMUM_MATCH_SCORE),
        positions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn score(text: &str, query: &str) -> Option<ScoredMatch> {
        fuzzy_score(FuzzyScoreInput { text, query })
    }

    #[test]
    fn exact_prefix_highest() {
        let prefix = score("Ship docs", "ship").unwrap();
        let mid_word = score("Worship plan", "ship").unwrap();
        let suffix = score("Flagship", "ship").unwrap();
        assert!(
            prefix.score > mid_word.score,
            "prefix {} > mid-word {}",
            prefix.score,
            mid_word.score
        );
        assert!(
            prefix.score > suffix.score,
            "prefix {} > mid-word {}",
            prefix.score,
            suffix.score
        );
    }

    #[test]
    fn no_match_returns_none() {
        assert!(score("hello", "xyz").is_none());
    }

    #[test]
    fn empty_query_matches_all() {
        let scored_match = score("anything", "").unwrap();
        assert_eq!(scored_match.score, 0);
        assert!(scored_match.positions.is_empty());
    }

    #[test]
    fn consecutive_beats_scattered() {
        let consecutive = score("abcdef", "abc").unwrap();
        let scattered = score("a_b_c_def", "abc").unwrap();
        assert!(consecutive.score > scattered.score);
    }

    #[test]
    fn case_insensitive() {
        assert!(score("Ship Docs", "sd").is_some());
    }

    #[test]
    fn positions_tracked() {
        let scored_match = score("Ship docs", "sd").unwrap();
        assert_eq!(scored_match.positions.len(), 2);
        assert_eq!(scored_match.positions[0], 0);
        assert_eq!(scored_match.positions[1], 5);
    }
}
