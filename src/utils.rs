use indicatif::{MultiProgress, ProgressBar};
use ra_ap_hir as hir;
use ra_ap_ide::RootDatabase;

pub fn crate_name(krate: hir::Crate, db: &RootDatabase) -> String {
    let name = &krate
        .display_name(db)
        .map(|s| s.to_string())
        .unwrap_or("<unknown>".to_string());
    name.replace('-', "_")
}
pub fn display_path(def: hir::ModuleDef, db: &RootDatabase) -> String {
    path(def, db).unwrap_or_else(|| "<unknown>".to_string())
}
fn path(def: hir::ModuleDef, db: &RootDatabase) -> Option<String> {
    use ra_ap_hir::AsAssocItem;
    let mut path = String::new();
    let krate = def.module(db).map(|m| m.krate());
    if let Some(name) = krate.map(|krate| crate_name(krate, db)) {
        path.push_str(&name);
    }
    let relative_path = if let Some(assoc) = def.as_assoc_item(db) {
        assoc_item_path(assoc, db)
    } else {
        def.canonical_path(db)
    };
    if let Some(relative_path) = relative_path {
        if !path.is_empty() {
            path.push_str("::");
        }
        path.push_str(&relative_path);
    }
    if path.is_empty() {
        None
    } else {
        Some(path)
    }
}
fn assoc_item_path(assoc: hir::AssocItem, db: &RootDatabase) -> Option<String> {
    let name = hir::ModuleDef::from(assoc)
        .name(db)
        .map(|name| name.display(db).to_string())?;
    let container = match assoc.container(db) {
        hir::AssocItemContainer::Trait(trait_) => hir::ModuleDef::from(trait_).canonical_path(db),
        hir::AssocItemContainer::Impl(impl_) => impl_
            .self_ty(db)
            .as_adt()
            .and_then(|adt| hir::ModuleDef::from(adt).canonical_path(db)),
    }?;
    Some(format!("{container}::{name}"))
}
pub fn create_bar(
    bars: &MultiProgress,
    msg: impl Into<std::borrow::Cow<'static, str>>,
) -> ProgressBar {
    let pb = bars.add(ProgressBar::new_spinner());
    pb.enable_steady_tick(std::time::Duration::from_millis(200));
    pb.set_message(msg);
    /*pb.set_style(
        indicatif::ProgressStyle::default_spinner()
            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ")
            .template("{spinner:.green} {msg}").unwrap(),
    );*/
    pb
}
