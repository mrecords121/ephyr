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

    /// `vod-meta` server's [`State`] to keep synchronized and persisted, along
    /// with its current version.
    ///
    /// Version is used for CAS (compare and swap) operations.
    state: Arc<RwLock<(State, u8)>>,
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
            state: Arc::new(RwLock::new((state, 0))),
        })
    }

    /// Returns the copy of the current actual [`State`].
    #[inline]
    pub async fn state(&self) -> State {
        self.state.read().await.0.clone()
    }

    /// Returns the copy of the current actual [`State`] along with it's current
    /// version.
    #[inline]
    pub async fn state_and_version(&self) -> (State, u8) {
        let state = self.state.read().await;
        (state.0.clone(), state.1)
    }

    /// Returns from the current actual [`State`] the copy of the [`Playlist`]
    /// identified by its `slug`.
    #[inline]
    pub async fn playlist(&self, slug: &PlaylistSlug) -> Option<Playlist> {
        (self.state.read().await.0).0.get(slug).cloned()
    }

    /// Replaces the current [`State`] with a `new` one.
    ///
    /// If `ver` is specified, then makes sure that it matches the version of
    /// the current [`State`], and if it doesn't, then no-op. This provides a
    /// basic [optimistic concurrency][1] allowing to modify the current
    /// [`State`] without holding the inner lock the whole modifying time.
    ///
    /// # Errors
    ///
    /// If the `new` [`State`] fails to be persisted.
    ///
    /// [1]: https://en.wikipedia.org/wiki/Optimistic_concurrency_control
    pub async fn set_state(
        &self,
        new: State,
        ver: Option<u8>,
    ) -> Result<(), anyhow::Error> {
        let mut state = self.state.write().await;

        if let Some(new_ver) = ver {
            if new_ver != state.1 {
                return Ok(());
            }
        }

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

        state.0 = new;
        state.1 = state.1.checked_add(1).unwrap_or_default();

        Ok(())
    }
}
