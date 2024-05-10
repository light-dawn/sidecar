use super::{
    base::ToolType,
    code_edit::{find::FindCodeSelectionInput, types::CodeEdit},
    code_symbol::important::{CodeSymbolImportantRequest, CodeSymbolImportantWideSearch},
    errors::ToolError,
    filtering::broker::{CodeToEditFilterRequest, CodeToEditSymbolRequest},
    grep::file::FindInFileRequest,
    lsp::{
        diagnostics::LSPDiagnosticsInput, gotodefintion::GoToDefinitionRequest,
        gotoimplementations::GoToImplementationRequest, open_file::OpenFileRequest,
    },
    rerank::base::ReRankEntriesForBroker,
};

#[derive(Debug, Clone)]
pub enum ToolInput {
    CodeEditing(CodeEdit),
    LSPDiagnostics(LSPDiagnosticsInput),
    FindCodeSnippets(FindCodeSelectionInput),
    ReRank(ReRankEntriesForBroker),
    RequestImportantSymbols(CodeSymbolImportantRequest),
    RequestImportantSybmolsCodeWide(CodeSymbolImportantWideSearch),
    GoToDefinition(GoToDefinitionRequest),
    OpenFile(OpenFileRequest),
    GrepSingleFile(FindInFileRequest),
    SymbolImplementations(GoToImplementationRequest),
    FilterCodeSnippetsForEditing(CodeToEditFilterRequest),
    FilterCodeSnippetsForEditingSingleSymbols(CodeToEditSymbolRequest),
}

impl ToolInput {
    pub fn tool_type(&self) -> ToolType {
        match self {
            ToolInput::CodeEditing(_) => ToolType::CodeEditing,
            ToolInput::LSPDiagnostics(_) => ToolType::LSPDiagnostics,
            ToolInput::FindCodeSnippets(_) => ToolType::FindCodeSnippets,
            ToolInput::ReRank(_) => ToolType::ReRank,
            ToolInput::RequestImportantSymbols(_) => ToolType::RequestImportantSymbols,
            ToolInput::RequestImportantSybmolsCodeWide(_) => ToolType::FindCodeSymbolsCodeBaseWide,
            ToolInput::GoToDefinition(_) => ToolType::GoToDefinitions,
            ToolInput::OpenFile(_) => ToolType::OpenFile,
            ToolInput::GrepSingleFile(_) => ToolType::GrepInFile,
            ToolInput::SymbolImplementations(_) => ToolType::GoToImplementations,
            ToolInput::FilterCodeSnippetsForEditing(_) => ToolType::FilterCodeSnippetsForEditing,
            ToolInput::FilterCodeSnippetsForEditingSingleSymbols(_) => {
                ToolType::FilterCodeSnippetsSingleSymbolForEditing
            }
        }
    }

    pub fn symbol_implementations(self) -> Result<GoToImplementationRequest, ToolError> {
        if let ToolInput::SymbolImplementations(symbol_implementations) = self {
            Ok(symbol_implementations)
        } else {
            Err(ToolError::WrongToolInput)
        }
    }

    pub fn grep_single_file(self) -> Result<FindInFileRequest, ToolError> {
        if let ToolInput::GrepSingleFile(grep_single_file) = self {
            Ok(grep_single_file)
        } else {
            Err(ToolError::WrongToolInput)
        }
    }

    pub fn is_file_open(self) -> Result<OpenFileRequest, ToolError> {
        if let ToolInput::OpenFile(open_file) = self {
            Ok(open_file)
        } else {
            Err(ToolError::WrongToolInput)
        }
    }

    pub fn is_go_to_definition(self) -> Result<GoToDefinitionRequest, ToolError> {
        if let ToolInput::GoToDefinition(definition_request) = self {
            Ok(definition_request)
        } else {
            Err(ToolError::WrongToolInput)
        }
    }

    pub fn is_code_edit(self) -> Result<CodeEdit, ToolError> {
        if let ToolInput::CodeEditing(code_edit) = self {
            Ok(code_edit)
        } else {
            Err(ToolError::WrongToolInput)
        }
    }

    pub fn is_lsp_diagnostics(self) -> Result<LSPDiagnosticsInput, ToolError> {
        if let ToolInput::LSPDiagnostics(lsp_diagnostics) = self {
            Ok(lsp_diagnostics)
        } else {
            Err(ToolError::WrongToolInput)
        }
    }

    pub fn is_code_find(self) -> Result<FindCodeSelectionInput, ToolError> {
        if let ToolInput::FindCodeSnippets(find_code_snippets) = self {
            Ok(find_code_snippets)
        } else {
            Err(ToolError::WrongToolInput)
        }
    }

    pub fn is_rerank(self) -> Result<ReRankEntriesForBroker, ToolError> {
        if let ToolInput::ReRank(rerank) = self {
            Ok(rerank)
        } else {
            Err(ToolError::WrongToolInput)
        }
    }

    pub fn code_symbol_search(
        self,
    ) -> Result<either::Either<CodeSymbolImportantRequest, CodeSymbolImportantWideSearch>, ToolError>
    {
        if let ToolInput::RequestImportantSymbols(request_code_symbol_important) = self {
            Ok(either::Either::Left(request_code_symbol_important))
        } else if let ToolInput::RequestImportantSybmolsCodeWide(request_code_symbol_important) =
            self
        {
            Ok(either::Either::Right(request_code_symbol_important))
        } else {
            Err(ToolError::WrongToolInput)
        }
    }

    pub fn code_symbol_important(self) -> Result<CodeSymbolImportantRequest, ToolError> {
        if let ToolInput::RequestImportantSymbols(request_code_symbol_important) = self {
            Ok(request_code_symbol_important)
        } else {
            Err(ToolError::WrongToolInput)
        }
    }

    pub fn codebase_wide_important_symbols(
        self,
    ) -> Result<CodeSymbolImportantWideSearch, ToolError> {
        if let ToolInput::RequestImportantSybmolsCodeWide(request_code_symbol_important) = self {
            Ok(request_code_symbol_important)
        } else {
            Err(ToolError::WrongToolInput)
        }
    }

    pub fn filter_code_snippets_for_editing(self) -> Result<CodeToEditFilterRequest, ToolError> {
        if let ToolInput::FilterCodeSnippetsForEditing(filter_code_snippets_for_editing) = self {
            Ok(filter_code_snippets_for_editing)
        } else {
            Err(ToolError::WrongToolInput)
        }
    }

    pub fn filter_code_snippets_single_symbol(self) -> Result<CodeToEditSymbolRequest, ToolError> {
        if let ToolInput::FilterCodeSnippetsForEditingSingleSymbols(
            filter_code_snippets_for_editing,
        ) = self
        {
            Ok(filter_code_snippets_for_editing)
        } else {
            Err(ToolError::WrongToolInput)
        }
    }

    pub fn filter_code_snippets_request(
        self,
    ) -> Result<either::Either<CodeToEditFilterRequest, CodeToEditSymbolRequest>, ToolError> {
        if let ToolInput::FilterCodeSnippetsForEditing(filter_code_snippets_for_editing) = self {
            Ok(either::Left(filter_code_snippets_for_editing))
        } else if let ToolInput::FilterCodeSnippetsForEditingSingleSymbols(
            filter_code_snippets_for_editing,
        ) = self
        {
            Ok(either::Right(filter_code_snippets_for_editing))
        } else {
            Err(ToolError::WrongToolInput)
        }
    }
}
