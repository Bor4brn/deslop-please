pub(super) struct Table {
    values: [u32; 8],
}

impl Table {
    pub(super) fn new() -> Table {
        Table { values: [1, 2, 3, 4, 5, 6, 7, 8] }
    }

    pub(super) fn a(&self) -> u32 {
        self.values[0] + 1
    }

    pub(super) fn b(&self) -> u32 {
        self.values[1] + 1
    }

    pub(super) fn c(&self) -> u32 {
        self.values[2] + 1
    }

    pub(super) fn d(&self) -> u32 {
        self.values[3] + 1
    }

    pub(super) fn e(&self) -> u32 {
        self.values[4] + 1
    }

    pub(super) fn f(&self) -> u32 {
        self.values[5] + 1
    }

    pub(super) fn g(&self) -> u32 {
        self.values[6] + 1
    }

    pub(super) fn h(&self) -> u32 {
        self.values[7] + 1
    }
}
