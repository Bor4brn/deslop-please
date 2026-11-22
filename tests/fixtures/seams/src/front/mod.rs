mod part;

use crate::back::Back;

pub struct Front {
    back: Back,
    part: part::Part,
}

impl Front {
    pub fn new() -> Front {
        Front { back: Back::new(), part: part::Part }
    }

    pub fn f01(&self) -> u32 {
        self.back.f01()
    }

    pub fn f02(&self) -> u32 {
        self.back.f02()
    }

    pub fn f03(&self) -> u32 {
        self.back.f03()
    }

    pub fn f04(&self) -> u32 {
        self.back.f04()
    }

    pub fn f05(&self) -> u32 {
        self.back.f05()
    }

    pub fn f06(&self) -> u32 {
        self.back.f06()
    }

    pub fn f07(&self) -> u32 {
        self.back.f07()
    }

    pub fn f08(&self) -> u32 {
        self.back.f08()
    }

    pub fn f09(&self) -> u32 {
        self.back.f09()
    }

    pub fn f10(&self) -> u32 {
        self.back.f10()
    }

    pub fn f11(&self) -> u32 {
        self.back.f11()
    }

    pub fn f12(&self) -> u32 {
        self.back.f12()
    }

    pub fn f13(&self) -> u32 {
        self.back.f13()
    }

    pub fn f14(&self) -> u32 {
        self.back.f14()
    }

    pub fn f15(&self) -> u32 {
        self.back.f15()
    }

    pub fn f16(&self) -> u32 {
        self.back.f16()
    }

    pub fn f17(&self) -> u32 {
        self.back.f17()
    }

    pub fn f18(&self) -> u32 {
        self.back.f18()
    }

    pub fn f19(&self) -> u32 {
        self.back.f19()
    }

    pub fn f20(&self) -> u32 {
        self.back.f20()
    }

    pub fn f21(&self) -> u32 {
        self.back.f21()
    }

    pub fn f22(&self) -> u32 {
        self.back.f22()
    }

    pub fn f23(&self) -> u32 {
        self.back.f23()
    }

    pub fn f24(&self) -> u32 {
        self.back.f24()
    }

    pub fn f25(&self) -> u32 {
        self.f25_from_back()
    }

    fn f25_from_back(&self) -> u32 {
        self.back.f25()
    }
}
