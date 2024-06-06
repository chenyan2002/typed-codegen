use crate::utils::{crate_name, display_path};
use log::{debug, info, trace, warn};
use ra_ap_hir::{self as hir, Crate, HirDisplay};
use ra_ap_ide::RootDatabase;
use fxhash::FxHashSet;
use ra_ap_hir_def::FunctionId;

pub struct Builder<'a> {
    db: &'a RootDatabase,
    krate: Crate,
    pub visited: FxHashSet<FunctionId>,
    pub worklist: Vec<FunctionId>,
}
impl<'a> Builder<'a> {
    pub fn new(db: &'a RootDatabase, krate: Crate) -> Self {
        Self {
            db,
            krate,
            worklist: Vec::new(),
            visited: FxHashSet::default(),
        }
    }
    pub fn build(&mut self) {
        let name = crate_name(self.krate, self.db);
        let name = if let Some(ver) = self.krate.version(self.db) {
            format!("{name} {ver}")
        } else {
            name
        };
        info!("Auditing crate {}...", name);
        let module = self.krate.root_module();
        self.process_module(module);
        for impl_ in hir::Impl::all_in_crate(self.db, self.krate) {
            self.process_impl(impl_);
        }
        /*for f in &self.unsafe_funcs {
            warn!("{f} has unsafe blocks")
    }*/
    }
    fn process_module(&mut self, module: hir::Module) {
        trace!("Processing module: {}", module.display(self.db));
        let decls = module.declarations(self.db);
        for d in decls {
            self.process_def(d);
        }
    }
    fn process_def(&mut self, def: hir::ModuleDef) {
        trace!("Processing def: {:?}", def.name(self.db));
        match def {
            hir::ModuleDef::Module(module) => self.process_module(module),
            hir::ModuleDef::Function(func) => self.process_function(func),
            _ => (),
        }
    }
    fn process_impl(&mut self, impl_: hir::Impl) {
        impl_.items(self.db).into_iter().for_each(|item| {
            if let hir::AssocItem::Function(func) = item {
                self.process_function(func);
            }
        });
    }
    fn process_function(&mut self, func: hir::Function) {
        use ra_ap_hir::db::DefDatabase;
        use ra_ap_hir::{DefWithBody, HasAttrs};
        use ra_ap_hir_def::{hir::Expr, DefWithBodyId};
        use ra_ap_hir_def::resolver::{HasResolver, ValueNs};
        if !self.visited.insert(func.into()) {
            return;
        }
        let name = display_path(func.into(), self.db);
        trace!("Processing function: {name}");
        self.visited.insert(func.into());
        let body_id: DefWithBodyId = DefWithBody::from(func).into();
        let body = self.db.body(body_id);
        let resolver = body_id.resolver(self.db);
        for (_, expr) in body.exprs.iter() {
            match expr {
                Expr::Unsafe { .. } => {
                    warn!("{name} UNSAFE");
                }
                //Expr::Missing => debug!("{name} MISSING!"),
                Expr::MethodCall { receiver, method_name, .. } => {
                    let receiver = &body.exprs[*receiver];
                    let Expr::Path(ref path) = receiver else { log::error!("{name}: {:?} ==> {:?}", expr, receiver); continue; };
                    let val = resolver.resolve_path_in_value_ns(self.db, &path);
                    debug!("{name}: {:?} ==> {:?} --> {:?}", expr, path, val);
                }
                Expr::Path(path) => {
                    let val = resolver.resolve_path_in_value_ns_fully(self.db, &path);
                    if let Some(ValueNs::FunctionId(f)) = val {
                        self.process_function(f.into());
                    }
                }
                _ => (),
            }
        }
        let attrs = func.attrs(self.db);
        if let Some(export) = attrs.export_name() {
            warn!("{} exports {}", name, export);
        }
    }
}
