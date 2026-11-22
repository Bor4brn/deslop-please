mod sum;

use crate::front::Front;

pub fn run() -> u32 {
    let front = Front::new();
    sum::sum(&[
        front.f01(),
        front.f02(),
        front.f03(),
        front.f04(),
        front.f05(),
        front.f06(),
        front.f07(),
        front.f08(),
        front.f09(),
        front.f10(),
        front.f11(),
        front.f12(),
        front.f13(),
        front.f14(),
        front.f15(),
        front.f16(),
        front.f17(),
        front.f18(),
        front.f19(),
        front.f20(),
        front.f21(),
        front.f22(),
        front.f23(),
        front.f24(),
        front.f25(),
    ])
}
