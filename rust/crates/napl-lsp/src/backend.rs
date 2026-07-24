//! The `LanguageServer` implementation: an in-memory document store plus the
//! request/notification handlers that delegate to the hover, navigation, and
//! diagnostics layers. It never runs gen — `napl watch` owns auto-compile.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;

use tower_lsp_server::ls_types::{
    CodeLens, CodeLensOptions, CodeLensParams, DidChangeTextDocumentParams,
    DidChangeWatchedFilesParams, DidChangeWatchedFilesRegistrationOptions, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, FileSystemWatcher, GlobPattern,
    GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams, HoverProviderCapability,
    InitializeParams, InitializeResult, InitializedParams, Location, OneOf, ReferenceParams,
    Registration, ServerCapabilities, ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind,
    Uri,
};
use tower_lsp_server::{Client, LanguageServer};

use crate::{diagnostics, hover, navigation, VERSION};

/// The NAPL language server backend.
pub struct Backend {
    client: Client,
    documents: RwLock<HashMap<String, String>>,
}

impl Backend {
    /// Build a backend bound to `client`.
    #[must_use]
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: RwLock::new(HashMap::new()),
        }
    }

    fn store_document(&self, uri: &Uri, text: String) {
        if let Ok(mut documents) = self.documents.write() {
            documents.insert(uri.as_str().to_string(), text);
        }
    }

    fn remove_document(&self, uri: &Uri) {
        if let Ok(mut documents) = self.documents.write() {
            documents.remove(uri.as_str());
        }
    }

    fn document_text(&self, uri: &Uri) -> Option<String> {
        if let Ok(documents) = self.documents.read() {
            if let Some(text) = documents.get(uri.as_str()) {
                return Some(text.clone());
            }
        }
        let path = uri.to_file_path()?;
        std::fs::read_to_string(path).ok()
    }

    fn open_uris(&self) -> Vec<String> {
        self.documents
            .read()
            .map(|documents| documents.keys().cloned().collect())
            .unwrap_or_default()
    }

    async fn publish(&self, uri: &Uri) {
        let Some(path) = uri_path(uri) else {
            return;
        };
        let text = self.document_text(uri).unwrap_or_default();
        let diagnostics = diagnostics::compute(&path, &text);
        self.client
            .publish_diagnostics(uri.clone(), diagnostics, None)
            .await;
    }

    async fn refresh_open_documents(&self) {
        for key in self.open_uris() {
            if let Ok(uri) = key.parse::<Uri>() {
                self.publish(&uri).await;
            }
        }
    }
}

fn uri_path(uri: &Uri) -> Option<PathBuf> {
    uri.to_file_path().map(std::borrow::Cow::into_owned)
}

impl LanguageServer for Backend {
    async fn initialize(&self, _params: InitializeParams) -> tower_lsp_server::jsonrpc::Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                code_lens_provider: Some(CodeLensOptions {
                    resolve_provider: Some(false),
                }),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: "napl".to_string(),
                version: Some(VERSION.to_string()),
            }),
            ..InitializeResult::default()
        })
    }

    async fn initialized(&self, _params: InitializedParams) {
        let options = DidChangeWatchedFilesRegistrationOptions {
            watchers: vec![FileSystemWatcher {
                glob_pattern: GlobPattern::String("**/.napl/**".to_string()),
                kind: None,
            }],
        };
        let registration = Registration {
            id: "napl-watched-files".to_string(),
            method: "workspace/didChangeWatchedFiles".to_string(),
            register_options: serde_json::to_value(options).ok(),
        };
        let client = self.client.clone();
        tokio::spawn(async move {
            let _ = client.register_capability(vec![registration]).await;
        });
    }

    async fn shutdown(&self) -> tower_lsp_server::jsonrpc::Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        self.store_document(&uri, params.text_document.text);
        self.publish(&uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(change) = params.content_changes.into_iter().last() {
            self.store_document(&uri, change.text);
        }
        self.publish(&uri).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(text) = params.text {
            self.store_document(&uri, text);
        }
        self.publish(&uri).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.remove_document(&uri);
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }

    async fn did_change_watched_files(&self, _params: DidChangeWatchedFilesParams) {
        self.refresh_open_documents().await;
    }

    async fn hover(&self, params: HoverParams) -> tower_lsp_server::jsonrpc::Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let Some(path) = uri_path(&uri) else {
            return Ok(None);
        };
        let Some(text) = self.document_text(&uri) else {
            return Ok(None);
        };
        Ok(hover::hover(
            &path,
            &text,
            params.text_document_position_params.position,
        ))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> tower_lsp_server::jsonrpc::Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let Some(path) = uri_path(&uri) else {
            return Ok(None);
        };
        let Some(text) = self.document_text(&uri) else {
            return Ok(None);
        };
        Ok(navigation::definition(
            &path,
            &text,
            params.text_document_position_params.position,
        ))
    }

    async fn references(
        &self,
        params: ReferenceParams,
    ) -> tower_lsp_server::jsonrpc::Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let Some(path) = uri_path(&uri) else {
            return Ok(None);
        };
        Ok(navigation::references(
            &path,
            params.text_document_position.position,
        ))
    }

    async fn code_lens(
        &self,
        params: CodeLensParams,
    ) -> tower_lsp_server::jsonrpc::Result<Option<Vec<CodeLens>>> {
        let uri = params.text_document.uri;
        let Some(path) = uri_path(&uri) else {
            return Ok(None);
        };
        let Some(text) = self.document_text(&uri) else {
            return Ok(None);
        };
        Ok(Some(navigation::code_lens(&path, &text)))
    }
}
