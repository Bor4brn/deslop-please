mod values;

use values::Values;

pub struct Back {
    values: Values,
}

impl Back {
    pub fn new() -> Back {
        Back { values: Values::new() }
    }

    pub fn f01(&self) -> u32 {
        self.values.get(0) + 1
    }

    pub fn f02(&self) -> u32 {
        self.values.get(1) + 1
    }

    pub fn f03(&self) -> u32 {
        self.values.get(2) + 1
    }

    pub fn f04(&self) -> u32 {
        self.values.get(3) + 1
    }

    pub fn f05(&self) -> u32 {
        self.values.get(4) + 1
    }

    pub fn f06(&self) -> u32 {
        self.values.get(5) + 1
    }

    pub fn f07(&self) -> u32 {
        self.values.get(6) + 1
    }

    pub fn f08(&self) -> u32 {
        self.values.get(7) + 1
    }

    pub fn f09(&self) -> u32 {
        self.values.get(8) + 1
    }

    pub fn f10(&self) -> u32 {
        self.values.get(9) + 1
    }

    pub fn f11(&self) -> u32 {
        self.values.get(10) + 1
    }

    pub fn f12(&self) -> u32 {
        self.values.get(11) + 1
    }

    pub fn f13(&self) -> u32 {
        self.values.get(12) + 1
    }

    pub fn f14(&self) -> u32 {
        self.values.get(13) + 1
    }

    pub fn f15(&self) -> u32 {
        self.values.get(14) + 1
    }

    pub fn f16(&self) -> u32 {
        self.values.get(15) + 1
    }

    pub fn f17(&self) -> u32 {
        self.values.get(16) + 1
    }

    pub fn f18(&self) -> u32 {
        self.values.get(17) + 1
    }

    pub fn f19(&self) -> u32 {
        self.values.get(18) + 1
    }

    pub fn f20(&self) -> u32 {
        self.values.get(19) + 1
    }

    pub fn f21(&self) -> u32 {
        self.values.get(20) + 1
    }

    pub fn f22(&self) -> u32 {
        self.values.get(21) + 1
    }

    pub fn f23(&self) -> u32 {
        self.values.get(22) + 1
    }

    pub fn f24(&self) -> u32 {
        self.values.get(23) + 1
    }

    pub fn f25(&self) -> u32 {
        self.values.get(24) + 1
    }
}
