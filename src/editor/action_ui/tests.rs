use super::*;

#[test]
fn every_action_kind_round_trips_through_its_default() {
    for (kind, _) in ACTION_KINDS {
        assert_eq!(action_name(&default_action(kind)), kind);
    }
}
