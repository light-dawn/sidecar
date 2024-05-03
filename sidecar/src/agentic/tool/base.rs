//! Contains the basic tool and how to extract data from it

use axum::async_trait;

use super::{errors::ToolError, input::ToolInput, output::ToolOutput};

#[derive(Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ToolType {
    AskDocumentation,
    AskUser,
    CodeEditing,
    Search,
    GoToDefinitions,
    GoToReferences,
    FileSystem,
    FolderOutline,
    Terminal,
}

#[async_trait]
pub trait Tool {
    async fn invoke(&self, input: ToolInput) -> Result<ToolOutput, ToolError>;
}
