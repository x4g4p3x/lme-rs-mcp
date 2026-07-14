//! In-memory cache of fitted models keyed by `fit_id`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use lme_rs::LmeFit;

#[derive(Debug)]
pub struct CachedFit {
    pub formula: String,
    pub data_path: PathBuf,
    pub reml: bool,
    pub fit: LmeFit,
}

#[derive(Debug, Default)]
pub struct FitSession {
    inner: Mutex<HashMap<String, CachedFit>>,
}

impl FitSession {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, fit_id: String, entry: CachedFit) {
        self.inner
            .lock()
            .expect("fit session lock poisoned")
            .insert(fit_id, entry);
    }

    pub fn get(&self, fit_id: &str) -> Option<CachedFit> {
        self.inner
            .lock()
            .expect("fit session lock poisoned")
            .get(fit_id)
            .map(|c| CachedFit {
                formula: c.formula.clone(),
                data_path: c.data_path.clone(),
                reml: c.reml,
                fit: c.fit.clone(),
            })
    }

    pub fn list(&self) -> Vec<(String, CachedFit)> {
        self.inner
            .lock()
            .expect("fit session lock poisoned")
            .iter()
            .map(|(id, c)| {
                (
                    id.clone(),
                    CachedFit {
                        formula: c.formula.clone(),
                        data_path: c.data_path.clone(),
                        reml: c.reml,
                        fit: c.fit.clone(),
                    },
                )
            })
            .collect()
    }

    pub fn remove(&self, fit_id: &str) -> bool {
        self.inner
            .lock()
            .expect("fit session lock poisoned")
            .remove(fit_id)
            .is_some()
    }
}
