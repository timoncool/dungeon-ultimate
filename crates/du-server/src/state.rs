//! Общее состояние сервера.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use du_db::Store;

use crate::gpu::{Gpu, Paths};
use crate::jobs::JobQueue;

pub struct Inner {
    /// Корень приложения: рядом лежат веса, инструменты и собранный фронт.
    pub root: PathBuf,
    pub store: Store,
    pub gpu: Arc<Gpu>,
    /// Единственная очередь всего, что трогает видеокарту.
    pub queue: JobQueue,
    /// Куда складывать сгенерированные картинки и озвучку.
    pub generated: PathBuf,
    /// Куда складывать загруженное игроком.
    pub uploads: PathBuf,
    /// Собранный фронт; None — если приложение запущено без него.
    pub web_root: Option<PathBuf>,
}

#[derive(Clone)]
pub struct AppState(pub Arc<Inner>);

impl std::ops::Deref for AppState {
    type Target = Inner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Inner {
    /// Владелец карты для фоновых потоков хода.
    pub fn gpu_handle(&self) -> Arc<Gpu> {
        self.gpu.clone()
    }
}

impl AppState {
    pub fn new(root: &Path) -> Result<Self, du_db::DbError> {
        let data = root.join("data");
        let generated = root.join("generated");
        let uploads = root.join("uploads");
        for dir in [&data, &generated, &uploads] {
            let _ = std::fs::create_dir_all(dir);
        }
        let store = Store::open(&data.join("dungeon.sqlite"))?;
        Ok(AppState(Arc::new(Inner {
            root: root.to_path_buf(),
            store,
            gpu: Arc::new(Gpu::new(root, Paths::under(root))),
            queue: JobQueue::new(),
            generated,
            uploads,
            web_root: crate::spa::find_web_root(root),
        })))
    }
}
