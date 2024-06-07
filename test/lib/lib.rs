pub struct T;
impl T {
    pub fn chain(&self) -> &Self {
        &Self
    }
    pub fn inner(&self) -> Result<(), ()> {
        expand!(self.chain().chain());
        Ok(())
    }
}

#[macro_export]
macro_rules! expand {
    ($e: expr) => {{
        unsafe { $e }
    }};
}
