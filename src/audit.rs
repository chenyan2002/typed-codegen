use log::{debug, trace, warn};
use ra_ap_hir::{self as hir, Crate, HirDisplay};
use ra_ap_ide::RootDatabase;

pub struct Builder<'a> {
    db: &'a RootDatabase,
    krate: Crate,
    name: String,
    pub methods: Vec<hir::Function>,
}
impl<'a> Builder<'a> {
    pub fn new(db: &'a RootDatabase, krate: Crate) -> Self {
        Self {
            db,
            krate,
            name: krate
                .display_name(db)
                .map(|c| c.canonical_name().to_string())
                .unwrap_or("<unknown>".to_string()),
            methods: Vec::new(),
        }
    }
    pub fn build(&mut self) {
        debug!("Auditing crate {}...", self.name);
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
        let func_name = func.name(self.db).to_smol_str();
        let body_id: DefWithBodyId = DefWithBody::from(func).into();
        let body = self.db.body(body_id);
        let entry = &body.exprs[body.body_expr];
        entry.walk_child_exprs(|id| {
            let expr = &body.exprs[id];
            if let Expr::Unsafe { .. } = expr {
                warn!("{}::{} contains unsafe block", self.name, func_name);
            }
        });
        let attrs = func.attrs(self.db);
        if let Some(export) = attrs.export_name() {
            warn!("{}::{} exports {}", self.name, func_name, export);
        }
    }
}
