use candid::{self, CandidType, Deserialize, Principal};

#[derive(CandidType, Deserialize)]
pub struct List(Option<(candid::Int,Box<List>,)>);
#[derive(CandidType, Deserialize)]
pub struct Profile { pub age: u8, pub name: String }

#[update]
fn greet(arg0: String) -> String {
  unimplemented!()
}

#[update]
fn get_profile(arg0: u64) -> std::result::Result<Profile, String> {
  todo!()
}
