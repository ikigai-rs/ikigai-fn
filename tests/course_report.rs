use std::collections::BTreeMap;

use ikigai_fn::course_report::{course_report, ExerciseAggregate, UnitAggregate};
use serde_json::json;

#[test]
fn instructor_report_only_serializes_reportable_counts_by_name() {
    let report = course_report(BTreeMap::from([
        (
            "first: unit",
            UnitAggregate {
                attempted: 9,
                struggled: 4,
                exercise: BTreeMap::from([(
                    "practice",
                    ExerciseAggregate {
                        attempted: 5,
                        failed: 5,
                        question: BTreeMap::from([("wording?", 0), ("example", 5)]),
                    },
                )]),
            },
        ),
        (
            "next",
            UnitAggregate {
                attempted: 4,
                struggled: 0,
                exercise: BTreeMap::from([(
                    "practice",
                    ExerciseAggregate {
                        attempted: 4,
                        failed: 1,
                        question: BTreeMap::new(),
                    },
                )]),
            },
        ),
    ]));

    assert_eq!(
        json!(report),
        json!({
            "urn:learn:unit:name:first%3A%20unit": {
                "attempted": 9,
                "struggled": "fewer than 5",
                "exercise": {
                    "urn:learn:exercise:name:practice": {
                        "attempted": 5,
                        "failed": 5,
                        "question": {
                            "urn:learn:question:name:wording%3F": {
                                "flagged_ambiguous": "fewer than 5"
                            },
                            "urn:learn:question:name:example": {
                                "flagged_ambiguous": 5
                            }
                        }
                    }
                }
            },
            "urn:learn:unit:name:next": {
                "attempted": "fewer than 5",
                "struggled": "fewer than 5",
                "exercise": {
                    "urn:learn:exercise:name:practice": {
                        "attempted": "fewer than 5",
                        "failed": "fewer than 5",
                        "question": {}
                    }
                }
            }
        })
    );
}
