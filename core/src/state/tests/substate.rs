use state::substate::Substate;
use log_entry::LogEntry;

#[test]
fn created() {
    let sub_state = Substate::new();
    assert_eq!(sub_state.suicides.len(), 0);
}

#[test]
fn accrue() {
    let mut sub_state = Substate::new();
    sub_state.contracts_created.push(1u64.into());
    sub_state.logs.push(LogEntry {
        address: 1u64.into(),
        topics: vec![],
        data: vec![],
    });
    sub_state.sstore_clears_count = 5.into();
    sub_state.suicides.insert(10u64.into());

    let mut sub_state_2 = Substate::new();
    sub_state_2.contracts_created.push(2u64.into());
    sub_state_2.logs.push(LogEntry {
        address: 1u64.into(),
        topics: vec![],
        data: vec![],
    });
    sub_state_2.sstore_clears_count = 7.into();

    sub_state.accrue(sub_state_2);
    assert_eq!(sub_state.contracts_created.len(), 2);
    assert_eq!(sub_state.sstore_clears_count, 12.into());
    assert_eq!(sub_state.suicides.len(), 1);
}
