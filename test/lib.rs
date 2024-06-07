
struct T;
impl T {
 fn chain(&self) -> &Self { Self }
 fn inner(&self) -> Result<()> { Ok(()) }
}

#[ic_cdk::init]
fn init() {
    ic_certified_assets::init();
}

macro_rules! expand {
  ($e: expr) => {{ unsafe { $e } }}
}

#[ic_cdk::query]
fn greet(name: String) -> String {
    let t = T;
    T.chain().inner().expect("ERR");
    format!("Hello, {}! {}", expand!(name), name.len())
}
