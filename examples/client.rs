use link_conditioner::{Conditioner, ConditionerConfig};

use std::net::UdpSocket;

pub fn main() {
    let socket = UdpSocket::bind(("0.0.0.0", 0)).unwrap();
    let conditioner = Conditioner::new(ConditionerConfig::default(), socket);
}
