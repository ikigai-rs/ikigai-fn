//! Instructor counts from already-aggregated course data.
//!
//! Inputs count distinct learners within each partition, not events. This
//! projection takes no learner rows and returns only reportable figures.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::{aggregate::Count, html::url_segment};

/// Aggregate input for one unit, with exercises keyed by name.
#[derive(Default)]
pub struct UnitAggregate<'a> {
    pub attempted: usize,
    pub struggled: usize,
    pub exercise: BTreeMap<&'a str, ExerciseAggregate<'a>>,
}

/// Aggregate input for one exercise, with questions keyed by name.
/// Each question's count is the number of learners who flagged it as ambiguous.
#[derive(Default)]
pub struct ExerciseAggregate<'a> {
    pub attempted: usize,
    pub failed: usize,
    pub question: BTreeMap<&'a str, usize>,
}

/// Reportable counts for one unit.
#[derive(Debug, Serialize)]
pub struct UnitCount {
    pub attempted: Count,
    pub struggled: Count,
    pub exercise: BTreeMap<String, ExerciseCount>,
}

/// Reportable counts for one exercise.
#[derive(Debug, Serialize)]
pub struct ExerciseCount {
    pub attempted: Count,
    pub failed: Count,
    pub question: BTreeMap<String, QuestionCount>,
}

/// Reportable counts for one question.
#[derive(Debug, Serialize)]
pub struct QuestionCount {
    pub flagged_ambiguous: Count,
}

/// Project one course's named units into instructor-facing counts. Child
/// names are scoped by their containing unit or exercise.
pub fn course_report(unit: BTreeMap<&str, UnitAggregate<'_>>) -> BTreeMap<String, UnitCount> {
    unit.into_iter()
        .map(|(name, unit)| {
            let exercise = unit
                .exercise
                .into_iter()
                .map(|(name, exercise)| {
                    let question = exercise
                        .question
                        .into_iter()
                        .map(|(name, count)| {
                            (
                                key("question", name),
                                QuestionCount {
                                    flagged_ambiguous: Count::new(count),
                                },
                            )
                        })
                        .collect();
                    (
                        key("exercise", name),
                        ExerciseCount {
                            attempted: Count::new(exercise.attempted),
                            failed: Count::new(exercise.failed),
                            question,
                        },
                    )
                })
                .collect();
            (
                key("unit", name),
                UnitCount {
                    attempted: Count::new(unit.attempted),
                    struggled: Count::new(unit.struggled),
                    exercise,
                },
            )
        })
        .collect()
}

fn key(kind: &str, name: &str) -> String {
    format!("urn:learn:{kind}:name:{}", url_segment(name))
}
