use log::{debug, trace};
use ra_ap_hir::{self as hir, Crate, HirDisplay};
use ra_ap_ide::RootDatabase;

pub struct Builder<'a> {
    db: &'a RootDatabase,
    krate: Crate,
}
impl<'a> Builder<'a> {
    pub fn new(db: &'a RootDatabase, krate: Crate) -> Self {
        Self { db, krate }
    }
    pub fn build(&self) {
        trace!("Scanning project...");
        let module = self.krate.root_module();
        self.process_module(module);
    }
    fn process_module(&self, module: hir::Module) {
        trace!("Processing module: {}", module.display(self.db));
        let decls = module.declarations(self.db);
        for d in decls {
            self.process_def(d);
        }
    }
    fn process_def(&self, def: hir::ModuleDef) {
        trace!("Processing def: {:?}", def.name(self.db));
        match def {
            hir::ModuleDef::Module(module) => self.process_module(module),
            hir::ModuleDef::Function(func) => self.process_function(func),
            hir::ModuleDef::Adt(_) => (),
            _ => (),
        }
    }
    fn process_function(&self, func: hir::Function) {
        use ra_ap_hir::HasAttrs;
        trace!("Processing function: {}", func.display(self.db));
        let attrs = func.attrs(self.db);
        let is_cdk = attrs.iter().find(|attr| {
            let attr = attr.path().segments().last().map(|s| s.to_smol_str());
            attr == Some("update".into())
                || attr == Some("query".into())
                || attr == Some("init".into())
        });
        if let Some(t) = is_cdk {
            debug!("Path: {:?}", t.path().segments());
            debug!("Func: {}", func.name(self.db).as_str().unwrap());
        } else {
            log::warn!("Function is not a cdk function: {}", func.display(self.db));
            return;
        }
        let args = func.params_without_self(self.db);
        let args = args.iter().map(|p| p.ty()).collect::<Vec<_>>();
        let args = args
            .into_iter()
            .map(|t| t.display(self.db).to_string())
            .collect::<Vec<_>>()
            .join(", ");
        debug!("Args: {}", args);
        let ret = if func.is_async(self.db) {
            func.async_ret_type(self.db).unwrap()
        } else {
            func.ret_type(self.db)
        };
        debug!("Ret: {}", ret.display(self.db));
    }
}
