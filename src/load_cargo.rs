use anyhow::Result;
use log::{debug, trace};
use ra_ap_hir::Crate;
use ra_ap_ide::RootDatabase;
use ra_ap_load_cargo::{load_workspace, LoadCargoConfig, ProcMacroServerChoice};
use ra_ap_paths::{AbsPathBuf, Utf8PathBuf};
use ra_ap_project_model::{
    CargoConfig, PackageData, ProjectManifest, ProjectWorkspace, TargetData,
};
use ra_ap_vfs::Vfs;
use std::path::Path;

pub fn load_cargo_project(path: &Path) -> Result<(RootDatabase, Vfs, TargetData)> {
    let cargo_config = cargo_config();
    let load_config = load_config();
    let mut ws = load_project_workspace(path, &cargo_config)?;
    let (_, target) = select_package_and_target(&ws)?;
    if load_config.load_out_dirs_from_check {
        let build_scripts = ws.run_build_scripts(&cargo_config, &|msg| {
            trace!("{}", msg);
        })?;
        ws.set_build_scripts(build_scripts);
    }
    let (db, vfs, _proc) = load_workspace(ws, &cargo_config.extra_env, &load_config)?;
    Ok((db, vfs, target))
}

fn load_project_workspace(path: &Path, cargo_config: &CargoConfig) -> Result<ProjectWorkspace> {
    let path_buf = std::env::current_dir()?.join(path).canonicalize()?;
    debug!("Loading project workspace: {:?}", path_buf);
    let utf8_path = Utf8PathBuf::from_path_buf(path_buf).unwrap();
    let root = AbsPathBuf::assert(utf8_path);
    let root = ProjectManifest::discover_single(root.as_path())?;
    ProjectWorkspace::load(root, cargo_config, &|msg| {
        trace!("{}", msg);
    })
}
fn cargo_config() -> CargoConfig {
    CargoConfig {
        sysroot: Some(ra_ap_project_model::RustLibSource::Discover),
        target: Some("wasm32-unknown-unknown".to_string()),
        ..Default::default()
    }
}
fn load_config() -> LoadCargoConfig {
    LoadCargoConfig {
        load_out_dirs_from_check: false,
        prefill_caches: false,
        with_proc_macro_server: ProcMacroServerChoice::Sysroot,
    }
}
fn select_package_and_target(ws: &ProjectWorkspace) -> Result<(PackageData, TargetData)> {
    use ra_ap_project_model::{ProjectWorkspaceKind, TargetKind};
    let cargo = match ws.kind {
        ProjectWorkspaceKind::Cargo { ref cargo, .. } => cargo,
        _ => return Err(anyhow::anyhow!("not a cargo workspace")),
    };
    let packages: Vec<_> = cargo
        .packages()
        .filter(|idx| cargo[*idx].is_member)
        .collect();
    if packages.len() != 1 {
        return Err(anyhow::anyhow!(
            "expected exactly one package, got {}",
            packages.len()
        ));
    }
    let package_idx = packages[0];
    let package = cargo[package_idx].clone();
    debug!("Package: {:?}", package.name);
    let targets: Vec<_> = package
        .targets
        .iter()
        .cloned()
        .filter(|idx| matches!(&cargo[*idx].kind, TargetKind::Bin | TargetKind::Lib { .. }))
        .collect();
    if targets.len() != 1 {
        return Err(anyhow::anyhow!(
            "expected exactly one target, got {}",
            targets.len()
        ));
    }
    let target = cargo[targets[0]].clone();
    debug!("Target: {:?}, {:?}", target.name, target.kind);
    Ok((package, target))
}
pub fn find_root_crate(db: &RootDatabase, vfs: &Vfs, target: &TargetData) -> Result<Crate> {
    let crates = Crate::all(db);
    let root_path = target.root.as_path();
    let krate = crates.into_iter().find(|krate| {
        let vfs_path = vfs.file_path(krate.root_file(db));
        let crate_root_path = vfs_path.as_path().unwrap();
        crate_root_path == root_path
    });
    krate.ok_or_else(|| anyhow::anyhow!("root crate not found"))
}
