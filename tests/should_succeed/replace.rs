extern crate creusot_contracts;

pub struct Something {
    pub a: u32,
    pub b: Option<Box<Something>>,
}

pub fn test(mut _a: Something, b: Something) {
    _a = b;
}
