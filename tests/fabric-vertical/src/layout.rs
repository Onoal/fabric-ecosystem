use std::path::PathBuf;

pub(crate) struct HarnessLayout {
    root: PathBuf,
}

impl HarnessLayout {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub(crate) fn ensure_filesystem_layout(&self) {
        for directory in [
            self.root.join("identity"),
            self.root.join("authority"),
            self.root.join("secrets"),
            self.root.join("service"),
            self.root.join("connectivity"),
            self.database_root(),
            self.kv_root(),
            self.worker_runtime_root(),
            self.artifacts_root(),
        ] {
            std::fs::create_dir_all(&directory).expect("create fabric vertical harness layout");
        }
    }

    pub(crate) fn identity_db(&self) -> PathBuf {
        self.root.join("identity").join("identity.sqlite")
    }

    pub(crate) fn authority_db(&self) -> PathBuf {
        self.root.join("authority").join("authority.sqlite")
    }

    pub(crate) fn secrets_db(&self) -> PathBuf {
        self.root.join("secrets").join("secrets.sqlite")
    }

    pub(crate) fn service_db(&self) -> PathBuf {
        self.root.join("service").join("service.sqlite")
    }

    pub(crate) fn connectivity_db(&self) -> PathBuf {
        self.root.join("connectivity").join("connectivity.sqlite")
    }

    pub(crate) fn database_root(&self) -> PathBuf {
        self.root.join("resources").join("database")
    }

    pub(crate) fn kv_root(&self) -> PathBuf {
        self.root.join("resources").join("kv")
    }

    pub(crate) fn worker_runtime_root(&self) -> PathBuf {
        self.root.join("worker").join("runtime")
    }

    pub(crate) fn artifacts_root(&self) -> PathBuf {
        self.root.join("artifacts")
    }
}
