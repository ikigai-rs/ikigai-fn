use ikigai_fn::aggregate::{Count, Ratio, COUNT_FLOOR};
use serde_json::{json, Value};

#[test]
fn aggregate_output_matches_the_shared_vector() {
    let vector: Value = serde_json::from_str(include_str!("vectors/count_vector.json")).unwrap();
    assert_eq!(json!(COUNT_FLOOR), vector["floor"]);
    for case in vector["count"].as_array().unwrap() {
        let count = Count::new(case["count"].as_u64().unwrap() as usize);
        assert_eq!(json!(count), case["json"], "{case}");
        assert_eq!(json!(count.to_string()), case["text"], "{case}");
        if case["json"].is_string() {
            assert_eq!(format!("{count:?}"), format!("{:?}", Count::new(0)));
        }
    }
    for case in vector["ratio"].as_array().unwrap() {
        let ratio = Ratio::new(
            case["part"].as_u64().unwrap() as usize,
            case["group"].as_u64().unwrap() as usize,
        );
        assert_eq!(json!(ratio), case["ratio"], "{case}");
        let percent = ratio.map(Ratio::percent);
        assert_eq!(json!(percent), case["percent"], "{case}");
        assert_eq!(
            json!(ratio.map(|value| value.to_string())),
            case["ratio_text"],
            "{case}"
        );
        assert_eq!(
            json!(percent.map(|value| value.to_string())),
            case["percent_text"],
            "{case}"
        );
    }
}
