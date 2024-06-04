use crate::utils::{crate_name, display_path};
use log::{debug, trace, warn};
use ra_ap_hir::{self as hir, Crate, HirDisplay};
use ra_ap_ide::RootDatabase;

pub struct Builder<'a> {
    db: &'a RootDatabase,
    krate: Crate,
    pub methods: Vec<hir::Function>,
}
impl<'a> Builder<'a> {
    pub fn new(db: &'a RootDatabase, krate: Crate) -> Self {
        Self {
            db,
            krate,
            methods: Vec::new(),
        }
    }
    pub fn build(&mut self) {
        debug!("Auditing crate {}...", crate_name(self.krate, self.db));
        let module = self.krate.root_module();
        self.process_module(module)
    }
    fn process_module(&mut self, module: hir::Module) {
        trace!("Processing module: {}", module.display(self.db));
        let decls = module.declarations(self.db);
        for d in decls {
            self.process_def(d);
        }
        for impl_ in hir::Impl::all_in_crate(self.db, self.krate) {
            self.process_impl(impl_);
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
        trace!("Processing function: {}", func.display(self.db));
        let body_id: DefWithBodyId = DefWithBody::from(func).into();
        let body = self.db.body(body_id);
        let entry = &body.exprs[body.body_expr];
        let mut is_unsafe = false;
        entry.walk_child_exprs(|id| {
            let expr = &body.exprs[id];
            if let Expr::Unsafe { .. } = expr {
                is_unsafe = true;
            }
        });
        if is_unsafe {
            warn!(
                "{} contains unsafe block",
                display_path(func.into(), self.db)
            );
        }
        let attrs = func.attrs(self.db);
        if let Some(export) = attrs.export_name() {
            warn!("{} exports {}", display_path(func.into(), self.db), export);
        }
    }
}
