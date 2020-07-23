//! Manager of `vod-meta` server [`State`].

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::anyhow;
use tokio::{fs, io::AsyncReadExt as _, sync::RwLock};

use super::{Playlist, PlaylistSlug, State};

/// Manager of `vod-meta` server [`State`].
///
/// It provides access to a synchronized [`State`] and takes care about
/// persisting it to filesystem to survive application restarts.
#[derive(Clone, Debug)]
pub struct Manager {
    /// Path to the file where the [`Manager::state`] should be persisted.
    file: Arc<PathBuf>,

    /// `vod-meta` server's [`State`] to keep synchronized and persisted.
    state: Arc<RwLock<State>>,
}

impl Manager {
    /// Instantiates new [`Manager`] to read from and persist the [`State`] in
    /// the provided `file`.
    ///
    /// If no `file` exists, the new empty one will be created.
    ///
    /// # Errors
    ///
    /// If the `file`:
    /// - cannot be read;
    /// - contains broken [`State`].
    pub async fn try_new<P: AsRef<Path>>(
        file: P,
    ) -> Result<Self, anyhow::Error> {
        let file = file.as_ref();

        let mut contents = vec![];
        fs::OpenOptions::new()
            .write(true)
            .create(true)
            .read(true)
            .open(&file)
            .await
            .map_err(|e| {
                anyhow!("Failed to open '{}' file: {}", file.display(), e)
            })?
            .read_to_end(&mut contents)
            .await
            .map_err(|e| {
                anyhow!("Failed to read '{}' file: {}", file.display(), e)
            })?;

        let state = if contents.is_empty() {
            State::default()
        } else {
            serde_json::from_slice(&contents).map_err(|e| {
                anyhow!(
                    "Failed to deserialize vod::meta::State read from \
                     '{}' file: {}",
                    file.display(),
                    e,
                )
            })?
        };

        Ok(Self {
            file: Arc::new(file.to_owned()),
            state: Arc::new(RwLock::new(state)),
        })
    }

    /// Returns the copy of the current actual [`State`].
    #[inline]
    pub async fn state(&self) -> State {
        self.state.read().await.clone()
    }

    /// Returns from the current actual [`State`] the copy of the [`Playlist`]
    /// identified by its `slug`.
    #[inline]
    pub async fn playlist(&self, slug: &PlaylistSlug) -> Option<Playlist> {
        self.state.read().await.0.get(slug).cloned()
    }

    /// Replaces the current [`State`] with a `new` one.
    ///
    /// # Errors
    ///
    /// If the `new` [`State`] fails to be persisted.
    pub async fn set_state(&self, new: State) -> Result<(), anyhow::Error> {
        fs::write(
            &*self.file,
            serde_json::to_vec(&new)
                .expect("Failed to serialize vod::meta::State"),
        )
        .await
        .map_err(|e| {
            anyhow!(
                "Failed to persist vod::meta::State into '{}' file: {}",
                self.file.display(),
                e,
            )
        })?;

        *self.state.write().await = new;

        Ok(())
    }
}
