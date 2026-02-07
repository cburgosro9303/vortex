//! Create workspace use case.

use std::path::PathBuf;

use vortex_domain::persistence::WorkspaceManifest;

use crate::ports::{WorkspaceError, WorkspaceRepository};

/// Input for creating a new workspace.
#[derive(Debug, Clone)]
pub struct CreateWorkspaceInput {
    /// Directory where the workspace will be created.
    pub path: PathBuf,
    /// Name of the workspace.
    pub name: String,
}

/// Use case for creating a new workspace.
pub struct CreateWorkspace<R: WorkspaceRepository> {
    workspace_repo: R,
}

impl<R: WorkspaceRepository> CreateWorkspace<R> {
    /// Creates a new `CreateWorkspace` use case.
    #[must_use]
    pub const fn new(workspace_repo: R) -> Self {
        Self { workspace_repo }
    }

    /// Creates a new workspace at the specified path.
    ///
    /// # Errors
    /// - Returns error if workspace already exists at path
    /// - Returns error if directory creation fails
    pub async fn execute(
        &self,
        input: CreateWorkspaceInput,
    ) -> Result<WorkspaceManifest, WorkspaceError> {
        self.workspace_repo.create(&input.path, &input.name).await
    }
}
