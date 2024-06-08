pub trait MyTrait {
  fn trait_func() -> u8 { 42 }
}

pub struct T;
impl T {
    pub fn chain(&self) -> &Self {
        &Self
    }
    pub fn unsafe_inner(&self) -> Result<(), ()> {
        expand!(self.chain().chain());
        Ok(())
    }
}
impl MyTrait for T {
  fn trait_func() -> u8 { expand!(43) }
}
impl From<u8> for T {
  fn from(_: u8) -> T { T }
}
impl std::ops::Add for T {
  type Output = Self;
  fn add(self, _: Self) -> Self { expand!(T) }
}

#[macro_export]
macro_rules! expand {
    ($e: expr) => {{
        unsafe { $e }
    }};
}

#[link(wasm_import_module = "ic0")]
extern "C" {
    pub fn stable64_size() -> u64;
}
#[link(wasm_import_module = "whatever")]
extern "C" {
    pub fn non_ic_func();
}