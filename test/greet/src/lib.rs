use candid::{self, CandidType, Deserialize, Principal};

#[derive(CandidType, Deserialize)]
pub struct List(Option<(candid::Int,Box<List>,)>);
#[derive(CandidType, Deserialize)]
pub struct Profile { pub age: u8, pub name: String }

#[ic_cdk::update]
fn get_profile(arg0: candid::Nat) -> std::result::Result<Profile, String> {
  unimplemented!()
}

