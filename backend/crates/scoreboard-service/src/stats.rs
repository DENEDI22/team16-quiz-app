use uuid::Uuid;

use crate::models::{AnswerOption, QuestionStats, QuestionType};

pub fn build_question_stats(
    question_id: Uuid,
    total: i64,
    counts: &[(i32, i64)],
    correct_answer_id: Option<i32>,
) -> QuestionStats {
    let options = counts
        .iter()
        .map(|(answer_id, count)| AnswerOption {
            answer_id: answer_id.to_string(),
            percentage: percentage(*count, total),
        })
        .collect();

    QuestionStats {
        question_id: question_id.to_string(),
        total_answers: total.max(0) as u32,
        question_type: question_type(counts.len()),
        correct_answer_id: correct_answer_id
            .map(|id| id.to_string())
            .unwrap_or_default(),
        options,
    }
}

fn percentage(count: i64, total: i64) -> f32 {
    if total <= 0 {
        0.0
    } else {
        (count as f32 / total as f32) * 100.0
    }
}

fn question_type(distinct_options: usize) -> QuestionType {
    if distinct_options <= 2 {
        QuestionType::TrueFalse
    } else {
        QuestionType::Multiple
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_percentages_for_each_option() {
        let stats = build_question_stats(Uuid::nil(), 40, &[(1, 10), (2, 30)], Some(2));

        assert_eq!(stats.total_answers, 40);
        assert_eq!(stats.options.len(), 2);
        assert_eq!(stats.options[0].answer_id, "1");
        assert_eq!(stats.options[0].percentage, 25.0);
        assert_eq!(stats.options[1].answer_id, "2");
        assert_eq!(stats.options[1].percentage, 75.0);
    }

    #[test]
    fn percentages_sum_to_one_hundred() {
        let stats = build_question_stats(Uuid::nil(), 8, &[(1, 1), (2, 2), (3, 5)], Some(3));

        let sum: f32 = stats.options.iter().map(|o| o.percentage).sum();
        assert!((sum - 100.0).abs() < f32::EPSILON, "got {sum}");
    }

    #[test]
    fn two_or_fewer_options_is_true_false() {
        let one = build_question_stats(Uuid::nil(), 5, &[(1, 5)], Some(1));
        let two = build_question_stats(Uuid::nil(), 5, &[(1, 2), (2, 3)], Some(1));

        assert_eq!(one.question_type, QuestionType::TrueFalse);
        assert_eq!(two.question_type, QuestionType::TrueFalse);
    }

    #[test]
    fn three_or_more_options_is_multiple() {
        let stats = build_question_stats(Uuid::nil(), 6, &[(1, 1), (2, 2), (3, 3)], Some(3));

        assert_eq!(stats.question_type, QuestionType::Multiple);
    }

    #[test]
    fn correct_answer_id_is_stringified() {
        let stats = build_question_stats(Uuid::nil(), 3, &[(7, 3)], Some(7));
        assert_eq!(stats.correct_answer_id, "7");
    }

    #[test]
    fn missing_correct_answer_yields_empty_string() {
        let stats = build_question_stats(Uuid::nil(), 3, &[(1, 3)], None);
        assert_eq!(stats.correct_answer_id, "");
    }

    #[test]
    fn zero_total_does_not_divide_by_zero() {
        let stats = build_question_stats(Uuid::nil(), 0, &[(1, 0)], None);

        assert_eq!(stats.total_answers, 0);
        assert_eq!(stats.options[0].percentage, 0.0);
        assert!(stats.options[0].percentage.is_finite());
    }

    #[test]
    fn question_id_is_stringified() {
        let id = Uuid::from_u128(0x0123_4567_89ab_cdef_0123_4567_89ab_cdef);
        let stats = build_question_stats(id, 1, &[(1, 1)], Some(1));

        assert_eq!(stats.question_id, id.to_string());
    }
}
