use super::Header;
use super::U256;
use super::RewardsCalculator;
use super::POWEquihashEngineParams;
use super::DifficultyCalc;

#[test]
fn test_calculate_rewards_number1() {
    let params = POWEquihashEngineParams {
        rampup_upper_bound: U256::from(259200),
        rampup_lower_bound: U256::zero(),
        rampup_start_value: U256::from(748994641621655092u64),
        rampup_end_value: U256::from(1497989283243310185u64),
        lower_block_reward: U256::from(748994641621655092u64),
        upper_block_reward: U256::from(1497989283243310185u64),
        difficulty_bound_divisor: U256::zero(),
        block_time_lower_bound: 0u64,
        block_time_upper_bound: 0u64,
        minimum_difficulty: U256::zero(),
    };
    let calculator = RewardsCalculator::new(&params, None, U256::from(0));
    let mut header = Header::default();
    header.set_number(1);
    assert_eq!(
        calculator.calculate_reward(&header),
        U256::from(748997531261476163u64)
    );
}

#[test]
fn test_calculate_rewards_number10000() {
    let params = POWEquihashEngineParams {
        rampup_upper_bound: U256::from(259200),
        rampup_lower_bound: U256::zero(),
        rampup_start_value: U256::from(748994641621655092u64),
        rampup_end_value: U256::from(1497989283243310185u64),
        lower_block_reward: U256::from(748994641621655092u64),
        upper_block_reward: U256::from(1497989283243310185u64),
        difficulty_bound_divisor: U256::zero(),
        block_time_lower_bound: 0u64,
        block_time_upper_bound: 0u64,
        minimum_difficulty: U256::zero(),
    };
    let calculator = RewardsCalculator::new(&params, None, U256::from(0));
    let mut header = Header::default();
    header.set_number(10000);
    assert_eq!(
        calculator.calculate_reward(&header),
        U256::from(777891039832365092u64)
    );
}

#[test]
fn test_calculate_rewards_number259200() {
    let params = POWEquihashEngineParams {
        rampup_upper_bound: U256::from(259200),
        rampup_lower_bound: U256::zero(),
        rampup_start_value: U256::from(748994641621655092u64),
        rampup_end_value: U256::from(1497989283243310185u64),
        lower_block_reward: U256::from(748994641621655092u64),
        upper_block_reward: U256::from(1497989283243310185u64),
        difficulty_bound_divisor: U256::zero(),
        block_time_lower_bound: 0u64,
        block_time_upper_bound: 0u64,
        minimum_difficulty: U256::zero(),
    };
    let calculator = RewardsCalculator::new(&params, None, U256::from(0));
    let mut header = Header::default();
    header.set_number(259200);
    assert_eq!(
        calculator.calculate_reward(&header),
        U256::from(1497989283243258292u64)
    );
}

#[test]
fn test_calculate_rewards_number300000() {
    let params = POWEquihashEngineParams {
        rampup_upper_bound: U256::from(259200),
        rampup_lower_bound: U256::zero(),
        rampup_start_value: U256::from(748994641621655092u64),
        rampup_end_value: U256::from(1497989283243310185u64),
        lower_block_reward: U256::from(748994641621655092u64),
        upper_block_reward: U256::from(1497989283243310185u64),
        difficulty_bound_divisor: U256::zero(),
        block_time_lower_bound: 0u64,
        block_time_upper_bound: 0u64,
        minimum_difficulty: U256::zero(),
    };
    let calculator = RewardsCalculator::new(&params, None, U256::from(0));
    let mut header = Header::default();
    header.set_number(300000);
    assert_eq!(
        calculator.calculate_reward(&header),
        U256::from(1497989283243310185u64)
    );
}

#[test]
fn test_calculate_difficulty() {
    let params = POWEquihashEngineParams {
        rampup_upper_bound: U256::zero(),
        rampup_lower_bound: U256::zero(),
        rampup_start_value: U256::zero(),
        rampup_end_value: U256::zero(),
        lower_block_reward: U256::zero(),
        upper_block_reward: U256::zero(),
        difficulty_bound_divisor: U256::from(2048),
        block_time_lower_bound: 5u64,
        block_time_upper_bound: 15u64,
        minimum_difficulty: U256::from(16),
    };
    let calculator = DifficultyCalc::new(&params);
    let mut header = Header::default();
    header.set_number(3);
    let mut parent_header = Header::default();
    parent_header.set_timestamp(1524538000u64);
    parent_header.set_difficulty(U256::from(1));
    parent_header.set_number(2);
    let mut grant_parent_header = Header::default();
    grant_parent_header.set_timestamp(1524528000u64);
    grant_parent_header.set_number(1);
    let difficulty =
        calculator.calculate_difficulty(&header, &parent_header, Some(&grant_parent_header));
    assert_eq!(difficulty, U256::from(16));
}

#[test]
fn test_calculate_difficulty2() {
    let params = POWEquihashEngineParams {
        rampup_upper_bound: U256::zero(),
        rampup_lower_bound: U256::zero(),
        rampup_start_value: U256::zero(),
        rampup_end_value: U256::zero(),
        lower_block_reward: U256::zero(),
        upper_block_reward: U256::zero(),
        difficulty_bound_divisor: U256::from(2048),
        block_time_lower_bound: 5u64,
        block_time_upper_bound: 15u64,
        minimum_difficulty: U256::from(16),
    };
    let calculator = DifficultyCalc::new(&params);
    let mut header = Header::default();
    header.set_number(3);
    let mut parent_header = Header::default();
    parent_header.set_timestamp(1524528005u64);
    parent_header.set_number(2);
    parent_header.set_difficulty(U256::from(2000));
    let mut grant_parent_header = Header::default();
    grant_parent_header.set_timestamp(1524528000u64);
    grant_parent_header.set_number(1);
    let difficulty =
        calculator.calculate_difficulty(&header, &parent_header, Some(&grant_parent_header));
    assert_eq!(difficulty, U256::from(2001));
}

#[test]
fn test_calculate_difficulty3() {
    let params = POWEquihashEngineParams {
        rampup_upper_bound: U256::zero(),
        rampup_lower_bound: U256::zero(),
        rampup_start_value: U256::zero(),
        rampup_end_value: U256::zero(),
        lower_block_reward: U256::zero(),
        upper_block_reward: U256::zero(),
        difficulty_bound_divisor: U256::from(2048),
        block_time_lower_bound: 5u64,
        block_time_upper_bound: 15u64,
        minimum_difficulty: U256::from(16),
    };
    let calculator = DifficultyCalc::new(&params);
    let mut header = Header::default();
    header.set_number(3);
    let mut parent_header = Header::default();
    parent_header.set_timestamp(1524528010u64);
    parent_header.set_difficulty(U256::from(3000));
    parent_header.set_number(2);
    let mut grant_parent_header = Header::default();
    grant_parent_header.set_timestamp(1524528000u64);
    grant_parent_header.set_number(1);
    let difficulty =
        calculator.calculate_difficulty(&header, &parent_header, Some(&grant_parent_header));
    assert_eq!(difficulty, U256::from(3000));
}

#[test]
fn test_calculate_difficulty4() {
    let params = POWEquihashEngineParams {
        rampup_upper_bound: U256::zero(),
        rampup_lower_bound: U256::zero(),
        rampup_start_value: U256::zero(),
        rampup_end_value: U256::zero(),
        lower_block_reward: U256::zero(),
        upper_block_reward: U256::zero(),
        difficulty_bound_divisor: U256::from(2048),
        block_time_lower_bound: 5u64,
        block_time_upper_bound: 15u64,
        minimum_difficulty: U256::from(16),
    };
    let calculator = DifficultyCalc::new(&params);
    let mut header = Header::default();
    header.set_number(3);
    let mut parent_header = Header::default();
    parent_header.set_timestamp(1524528020u64);
    parent_header.set_difficulty(U256::from(3000));
    parent_header.set_number(2);
    let mut grant_parent_header = Header::default();
    grant_parent_header.set_timestamp(1524528000u64);
    grant_parent_header.set_number(1);
    let difficulty =
        calculator.calculate_difficulty(&header, &parent_header, Some(&grant_parent_header));
    assert_eq!(difficulty, U256::from(2999));
}
