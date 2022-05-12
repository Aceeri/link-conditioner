use link_conditioner::{ConditionerConfig, UdpConditioner};

pub fn main() {
    let conditioner =
        UdpConditioner::bind_conditioned(ConditionerConfig::default(), ("0.0.0.0", 0)).unwrap();
}
