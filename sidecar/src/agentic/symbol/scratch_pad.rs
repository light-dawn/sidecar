//! Contains the scratch pad agent whose job is to work alongside the developer
//! and help them accomplish a task
//! This way the agent can look at all the events and the requests which are happening
//! and take a decision based on them on what should happen next

use std::{collections::HashSet, pin::Pin, sync::Arc};

use futures::{stream, Stream, StreamExt};
use tokio::sync::{mpsc::UnboundedSender, Mutex};

use crate::{
    agentic::symbol::{events::types::SymbolEvent, ui_event::UIEventWithID},
    chunking::text_document::{Position, Range},
};

use super::{
    errors::SymbolError,
    events::{
        edit::SymbolToEdit,
        environment_event::{EditorStateChangeRequest, EnvironmentEventType},
        human::{HumanAnchorRequest, HumanMessage},
        lsp::{LSPDiagnosticError, LSPSignal},
        message_event::{SymbolEventMessage, SymbolEventMessageProperties},
    },
    identifier::SymbolIdentifier,
    tool_box::ToolBox,
    tool_properties::ToolProperties,
    types::SymbolEventRequest,
};

#[derive(Debug, Clone)]
struct ScratchPadFilesActive {
    _file_content: String,
    _file_path: String,
}

impl ScratchPadFilesActive {
    fn _new(file_content: String, file_path: String) -> Self {
        Self {
            _file_content: file_content,
            _file_path: file_path,
        }
    }
}

// We should have a way to update our cache of all that has been done
// and what we are upto right now
// the ideal goal would be to rewrite the scratchpad in a good way so we are
// able to work on top of that
// a single LLM call should rewrite the sections which are present and take as input
// the lsp signal
// we also need to tell symbol_event_request agent what all things are possible, like: getting data from elsewhere
// looking at some other file and keeping that in its cache
// also what kind of information it should keep in:
// it can be state driven based on the user ask
// there will be files which the system has to keep in context, which can be dynamic as well
// we have to control it to not go over the 50kish limit ... cause it can grow by a lot
// but screw it, we keep it as it is
// lets keep it free-flow before we figure out the right way to go about doing this
// mega-scratchpad ftw
// Things to do:
// - [imp] how do we keep the cache hot after making updates or discovering new information, we want to keep the prefix hot and consistenet always
// - [not_sure] when recieving a LSP signal we might want to edit or gather more information how do we go about doing that?
// - can we get the user behavior to be about changes done in the past and what effects it has
// - meta programming on the canvas maybe in some ways?
// - can we just start tracking the relevant edits somehow.. just that
// - would go a long way most probably
// - help us prepare for now
// - even better just show the git diff until now
// - even dumber just run git-diff and store it as context anyways?
// - we need access to the root directory for git here

/// Different kind of events which can happen
/// We should move beyond symbol events tbh at this point :')

#[derive(Clone)]
pub struct ScratchPadAgent {
    _storage_fs_path: String,
    message_properties: SymbolEventMessageProperties,
    tool_box: Arc<ToolBox>,
    // if the scratch-pad agent is right now focussed, then we can't react to other
    // signals and have to pay utmost attention to the current task we are workign on
    focussing: Arc<Mutex<bool>>,
    fixing: Arc<Mutex<bool>>,
    // we store the previous user queries as a vec<string> here so we can show that to
    // the llm when its running inference
    previous_user_queries: Arc<Mutex<Vec<String>>>,
    symbol_event_sender: UnboundedSender<SymbolEventMessage>,
    // This is the cache which we have to send with every request
    _files_context: Arc<Mutex<Vec<ScratchPadFilesActive>>>,
    // This is the extra context which we send everytime with each request
    // this also helps with the prompt cache hits
    _extra_context: Arc<Mutex<String>>,
    reaction_sender: UnboundedSender<EnvironmentEventType>,
}

impl ScratchPadAgent {
    pub async fn new(
        scratch_pad_path: String,
        message_properties: SymbolEventMessageProperties,
        tool_box: Arc<ToolBox>,
        symbol_event_sender: UnboundedSender<SymbolEventMessage>,
        user_provided_context: Option<String>,
    ) -> Self {
        let (reaction_sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        let scratch_pad_agent = Self {
            _storage_fs_path: scratch_pad_path,
            message_properties,
            tool_box,
            symbol_event_sender,
            focussing: Arc::new(Mutex::new(false)),
            fixing: Arc::new(Mutex::new(false)),
            previous_user_queries: Arc::new(Mutex::new(vec![])),
            _files_context: Arc::new(Mutex::new(vec![])),
            _extra_context: Arc::new(Mutex::new(user_provided_context.unwrap_or_default())),
            reaction_sender,
        };
        // let cloned_scratch_pad_agent = scratch_pad_agent.clone();
        let mut reaction_stream = tokio_stream::wrappers::UnboundedReceiverStream::new(receiver);

        // we also want a timer event here which can fetch lsp signals ad-hoc and as required
        tokio::spawn(async move {
            while let Some(reaction_event) = reaction_stream.next().await {
                if reaction_event.is_shutdown() {
                    break;
                }
                // we are not going to react the events right now
                // let _ = cloned_scratch_pad_agent
                //     .react_to_event(reaction_event)
                //     .await;
            }
        });
        scratch_pad_agent
    }
}

impl ScratchPadAgent {
    /// We try to contain all the events which are coming in from the symbol
    /// which is being edited by the user, the real interface here will look like this
    pub async fn process_envrionment(
        self,
        mut stream: Pin<Box<dyn Stream<Item = EnvironmentEventType> + Send + Sync>>,
    ) {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();

        // this is our filtering thread which will run in the background
        let cloned_self = self.clone();

        let _ = tokio::spawn(async move {
            let cloned_sender = sender;
            // damn borrow-checker got hands
            let cloned_self = cloned_self;
            while let Some(event) = stream.next().await {
                match &event {
                    // if its a lsp signal and we are still fixing, then skip it
                    EnvironmentEventType::LSP(_) => {
                        // if we are fixing or focussing then skip the lsp signal
                        if cloned_self.is_fixing().await {
                            println!("scratchpad::discarding_lsp::busy_fixing");
                            continue;
                        }
                        if cloned_self.is_focussing().await {
                            println!("scratchpad::discarding_lsp::busy_focussing");
                            continue;
                        }
                    }
                    _ => {}
                };
                let _ = cloned_sender.send(event);
            }
        });

        let mut stream = tokio_stream::wrappers::UnboundedReceiverStream::new(receiver);
        println!("scratch_pad_agent::start_processing_environment");
        while let Some(event) = stream.next().await {
            match event {
                EnvironmentEventType::LSP(lsp_signal) => {
                    // we just want to react to the lsp signal over here, so we do just that
                    // if we are fixing or if we are focussing
                    if self.is_fixing().await {
                        println!("scratchpad::environment_event::discarding_lsp::busy_fixing");
                        continue;
                    }
                    if self.is_focussing().await {
                        println!("scratchpad::environment_event::discarding_lsp::busy_focussing");
                        continue;
                    }
                    let _ = self
                        .reaction_sender
                        .send(EnvironmentEventType::LSP(lsp_signal));
                }
                EnvironmentEventType::Human(message) => {
                    let _ = self.handle_human_message(message).await;
                    // whenever the human sends a request over here, encode it and try
                    // to understand how to handle it, some might require search, some
                    // might be more automagic
                }
                EnvironmentEventType::Symbol(_symbol_event) => {
                    // we know a symbol is going to be edited, what should we do about it?
                }
                EnvironmentEventType::EditorStateChange(_) => {
                    // not sure what to do about this right now, this event is used so the
                    // scratchpad can react to it, so for now do not do anything
                    // we might have to split the events later down the line
                }
                EnvironmentEventType::ShutDown => {
                    println!("scratch_pad_agent::shut_down");
                    let _ = self.reaction_sender.send(EnvironmentEventType::ShutDown);
                    break;
                }
            }
        }
    }

    async fn _react_to_event(&self, event: EnvironmentEventType) {
        match event {
            EnvironmentEventType::Human(human_event) => {
                let _ = self._react_to_human_event(human_event).await;
            }
            EnvironmentEventType::EditorStateChange(editor_state_change) => {
                self._react_to_edits(editor_state_change).await;
            }
            EnvironmentEventType::LSP(lsp_signal) => {
                self._react_to_lsp_signal(lsp_signal).await;
            }
            _ => {}
        }
    }

    async fn handle_human_message(&self, human_message: HumanMessage) -> Result<(), SymbolError> {
        match human_message {
            HumanMessage::Anchor(anchor_request) => self.human_message_anchor(anchor_request).await,
            HumanMessage::Followup(_followup_request) => Ok(()),
        }
    }

    async fn _react_to_human_event(&self, human_event: HumanMessage) -> Result<(), SymbolError> {
        match human_event {
            HumanMessage::Anchor(anchor_request) => {
                let _ = self._handle_user_anchor_request(anchor_request).await;
            }
            HumanMessage::Followup(_followup_request) => {}
        }
        Ok(())
    }

    async fn human_message_anchor(
        &self,
        anchor_request: HumanAnchorRequest,
    ) -> Result<(), SymbolError> {
        let start_instant = std::time::Instant::now();
        println!("scratch_pad_agent::human_message_anchor::start");
        let anchored_symbols = anchor_request.anchored_symbols();
        let symbols_to_edit_request = self
            .tool_box
            .symbol_to_edit_request(
                anchored_symbols,
                anchor_request.user_query(),
                anchor_request.anchor_request_context(),
                self.message_properties.clone(),
            )
            .await?;

        let cloned_anchored_request = anchor_request.clone();
        // we are going to react to the user message
        let _ = self
            .reaction_sender
            .send(EnvironmentEventType::Human(HumanMessage::Anchor(
                cloned_anchored_request,
            )));

        // keep track of the user request in our state
        {
            let mut user_queries = self.previous_user_queries.lock().await;
            user_queries.push(anchor_request.to_string());
        }

        // we start making the edits
        {
            let mut focussed = self.focussing.lock().await;
            *focussed = true;
        }
        let edits_done = stream::iter(symbols_to_edit_request.into_iter().map(|data| {
            (
                data,
                self.message_properties.clone(),
                self.symbol_event_sender.clone(),
            )
        }))
        .map(
            |(symbol_to_edit_request, message_properties, symbol_event_sender)| async move {
                let (sender, receiver) = tokio::sync::oneshot::channel();
                let symbol_event_request = SymbolEventRequest::new(
                    symbol_to_edit_request.symbol_identifier().clone(),
                    SymbolEvent::Edit(symbol_to_edit_request), // defines event type
                    ToolProperties::new(),
                );
                let event = SymbolEventMessage::message_with_properties(
                    symbol_event_request,
                    message_properties,
                    sender,
                );
                let _ = symbol_event_sender.send(event);
                receiver.await
            },
        )
        // run 100 edit requests in parallel to prevent race conditions
        .buffer_unordered(100)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .filter_map(|s| s.ok())
        .collect::<Vec<_>>();

        let cloned_user_query = anchor_request.user_query().to_owned();
        // the editor state has changed, so we need to react to that now
        let _ = self
            .reaction_sender
            .send(EnvironmentEventType::EditorStateChange(
                EditorStateChangeRequest::new(edits_done, cloned_user_query),
            ));
        // we are not focussed anymore, we can go about receiving events as usual
        {
            let mut focussed = self.focussing.lock().await;
            *focussed = false;
        }
        println!(
            "scratch_pad_agent::human_message_anchor::end::time_taken({}ms)",
            start_instant.elapsed().as_millis()
        );
        // send end of iteration event over here to the frontend
        let _ = self
            .message_properties
            .ui_sender()
            .send(UIEventWithID::code_iteration_finished(
                self.message_properties.request_id_str().to_owned(),
            ));
        Ok(())
    }

    async fn _handle_user_anchor_request(&self, anchor_request: HumanAnchorRequest) {
        println!("scratch_pad::handle_user_anchor_request");
        // we are busy with the edits going on, so we can discard lsp signals for a while
        // figure out what to do over here
        let file_paths = anchor_request
            .anchored_symbols()
            .into_iter()
            .filter_map(|anchor_symbol| anchor_symbol.fs_file_path())
            .collect::<Vec<_>>();
        let mut already_seen_files: HashSet<String> = Default::default();
        let mut user_context_files = vec![];
        for fs_file_path in file_paths.into_iter() {
            if already_seen_files.contains(&fs_file_path) {
                continue;
            }
            already_seen_files.insert(fs_file_path.to_owned());
            let file_contents = self
                .tool_box
                .file_open(fs_file_path, self.message_properties.clone())
                .await;
            if let Ok(file_contents) = file_contents {
                user_context_files.push({
                    let file_path = file_contents.fs_file_path();
                    let language = file_contents.language();
                    let content = file_contents.contents_ref();
                    ScratchPadFilesActive::_new(
                        format!(
                            r#"<file>
<fs_file_path>
{file_path}
</fs_file_path>
<content>
```{language}
{content}
```
</content>
</file>"#
                        ),
                        file_path.to_owned(),
                    )
                });
            }
        }
        // update our cache over here
        {
            let mut files_context = self._files_context.lock().await;
            *files_context = user_context_files.to_vec();
        }
        let file_paths_interested = user_context_files
            .iter()
            .map(|context_file| context_file._file_path.to_owned())
            .collect::<Vec<_>>();
        let user_context_files = user_context_files
            .into_iter()
            .map(|context_file| context_file._file_content)
            .collect::<Vec<_>>();
        println!("scratch_pad_agent::tool_box::agent_human_request");
        let _ = self
            .tool_box
            .scratch_pad_agent_human_request(
                self._storage_fs_path.to_owned(),
                anchor_request.user_query().to_owned(),
                user_context_files,
                file_paths_interested,
                anchor_request
                    .anchored_symbols()
                    .into_iter()
                    .map(|anchor_symbol| {
                        let content = anchor_symbol.content();
                        let fs_file_path = anchor_symbol.fs_file_path().unwrap_or_default();
                        let line_range_header = format!(
                            "{}-{}:{}",
                            fs_file_path,
                            anchor_symbol.possible_range().start_line(),
                            anchor_symbol.possible_range().end_line()
                        );
                        format!(
                            r#"Location: {line_range_header}
```
{content}
```"#
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
                self.message_properties.clone(),
            )
            .await;
    }

    /// We want to react to the various edits which have happened and the request they were linked to
    /// and come up with next steps and try to understand what we can do to help the developer
    async fn _react_to_edits(&self, editor_state_change: EditorStateChangeRequest) {
        println!("scratch_pad::react_to_edits");
        // figure out what to do over here
        let user_context_files;
        {
            let files_context = self._files_context.lock().await;
            user_context_files = (*files_context).to_vec();
        }
        let file_paths_in_focus = user_context_files
            .iter()
            .map(|context_file| context_file._file_path.to_owned())
            .collect::<Vec<String>>();
        let user_context_files = user_context_files
            .into_iter()
            .map(|context_file| context_file._file_content)
            .collect::<Vec<_>>();
        let user_query = editor_state_change.user_query().to_owned();
        let edits_made = editor_state_change.consume_edits_made();
        let extra_context;
        {
            extra_context = (*self._extra_context.lock().await).to_owned();
        }
        {
            let mut extra_context = self._extra_context.lock().await;
            *extra_context = (*extra_context).to_owned()
                + "\n"
                + &edits_made
                    .iter()
                    .map(|edit| edit.clone().to_string())
                    .collect::<Vec<_>>()
                    .join("\n");
        }
        let _ = self
            .tool_box
            .scratch_pad_edits_made(
                &self._storage_fs_path,
                &user_query,
                &extra_context,
                file_paths_in_focus,
                edits_made
                    .into_iter()
                    .map(|edit| edit.to_string())
                    .collect::<Vec<_>>(),
                user_context_files,
                self.message_properties.clone(),
            )
            .await;

        // Now we want to grab the diagnostics which come in naturally
        // or via the files we are observing, there are race conditions here which
        // we want to tackle for sure
        // check for diagnostic_symbols
        // let cloned_self = self.clone();
        // let _ = tokio::spawn(async move {
        //     // sleep for 2 seconds before getting the signals
        //     let _ = tokio::time::sleep(Duration::from_secs(2)).await;
        //     cloned_self.grab_diagnostics().await;
        // });
    }

    /// We get to react to the lsp signal over here
    async fn _react_to_lsp_signal(&self, lsp_signal: LSPSignal) {
        let focussed;
        {
            focussed = *(self.focussing.lock().await);
        }
        if focussed {
            return;
        }
        match lsp_signal {
            LSPSignal::Diagnostics(diagnostics) => {
                self._react_to_diagnostics(diagnostics).await;
            }
        }
    }

    async fn _react_to_diagnostics(&self, diagnostics: Vec<LSPDiagnosticError>) {
        // we are busy fixing 🧘‍♂️
        {
            let mut fixing = self.fixing.lock().await;
            *fixing = true;
        }
        let file_paths_focussed;
        {
            file_paths_focussed = self
                ._files_context
                .lock()
                .await
                .iter()
                .map(|file_content| file_content._file_path.to_owned())
                .collect::<HashSet<String>>();
        }
        let diagnostic_messages = diagnostics
            .into_iter()
            .filter(|diagnostic| file_paths_focussed.contains(diagnostic.fs_file_path()))
            .map(|diagnostic| {
                let diagnostic_file_path = diagnostic.fs_file_path();
                let diagnostic_message = diagnostic.diagnostic_message();
                let diagnostic_snippet = diagnostic.snippet();
                format!(
                    r#"<fs_file_path>
{diagnostic_file_path}
</fs_file_path>
<message>
{diagnostic_message}
</message>
<snippet_with_error>
{diagnostic_snippet}
</snippet_with_error>"#
                )
            })
            .collect::<Vec<_>>();
        if diagnostic_messages.is_empty() {
            return;
        }
        println!("scratch_pad::reacting_to_diagnostics");
        let files_context;
        {
            files_context = (*self._files_context.lock().await).to_vec();
        }
        let extra_context;
        {
            extra_context = (*self._extra_context.lock().await).to_owned();
        }
        let interested_file_paths = files_context
            .iter()
            .map(|file_context| file_context._file_path.to_owned())
            .collect::<Vec<_>>();
        let _ = self
            .tool_box
            .scratch_pad_diagnostics(
                &self._storage_fs_path,
                diagnostic_messages,
                interested_file_paths,
                files_context
                    .into_iter()
                    .map(|files_context| files_context._file_content)
                    .collect::<Vec<_>>(),
                extra_context,
                self.message_properties.clone(),
            )
            .await;

        // we try to make code edits to fix the diagnostics
        let _ = self._code_edit_for_diagnostics().await;

        // we are done fixing so start skipping
        {
            let mut fixing = self.fixing.lock().await;
            *fixing = false;
        }
    }

    // Now that we have reacted to the update on the scratch-pad we can start
    // thinking about making code edits for this
    async fn _code_edit_for_diagnostics(&self) {
        // we want to give the scratch-pad as input to the agent and the files
        // which are visible as the context where it can make the edits
        // we can be a bit smarter and make the eidts over the file one after
        // the other
        // what about the cache hits over here? thats one of the major issues
        // on how we want to tack it
        // fuck the cache hit just raw dog the edits in parallel on the files
        // which we are tracking using the scratch-pad and the files
        let scratch_pad_content = self
            .tool_box
            .file_open(
                self._storage_fs_path.to_owned(),
                self.message_properties.clone(),
            )
            .await;
        if let Err(e) = scratch_pad_content.as_ref() {
            println!("scratch_pad_agnet::scratch_pad_reading::error");
            eprintln!("{:?}", e);
        }
        let scratch_pad_content = scratch_pad_content.expect("if let Err to hold");
        let active_file_paths;
        {
            active_file_paths = self
                ._files_context
                .lock()
                .await
                .iter()
                .map(|file_context| file_context._file_path.to_owned())
                .collect::<Vec<_>>();
        }
        // we should optimse for cache hit over here somehow
        let mut files_context = vec![];
        for active_file in active_file_paths.to_vec().into_iter() {
            let file_contents = self
                .tool_box
                .file_open(active_file, self.message_properties.clone())
                .await;
            if let Ok(file_contents) = file_contents {
                let fs_file_path = file_contents.fs_file_path();
                let language_id = file_contents.language();
                let contents = file_contents.contents_ref();
                files_context.push(format!(
                    r#"# FILEPATH: {fs_file_path}
    ```{language_id}
    {contents}
    ```"#
                ));
            }
        }
        let scratch_pad_contents_ref = scratch_pad_content.contents_ref();
        let mut edits_made = vec![];
        for active_file in active_file_paths.iter() {
            let symbol_identifier = SymbolIdentifier::with_file_path(&active_file, &active_file);
            let user_instruction = format!(
                r#"I am sharing with you the scratchpad where I am keeping track of all the things I am working on. I want you to make edits which help move the tasks forward.
It's important to remember that some edits might require additional steps before we can go about doing them, so feel free to ignore them.
Only make the edits to be the best of your ability in {active_file}

My scratchpad looks like this:
{scratch_pad_contents_ref}

Please help me out by making the necessary code edits"#
            );
            let symbol_event_request = SymbolEventRequest::simple_edit_request(
                symbol_identifier,
                SymbolToEdit::new(
                    active_file.to_owned(),
                    Range::new(Position::new(0, 0, 0), Position::new(10000, 0, 0)),
                    active_file.to_owned(),
                    vec![user_instruction.to_owned()],
                    false,
                    false,
                    true,
                    user_instruction,
                    None,
                    false,
                    Some(files_context.to_vec().join("\n")),
                    true,
                ),
                ToolProperties::new(),
            );
            let (sender, receiver) = tokio::sync::oneshot::channel();
            let symbol_event_message = SymbolEventMessage::message_with_properties(
                symbol_event_request,
                self.message_properties.clone(),
                sender,
            );
            let _ = self.symbol_event_sender.send(symbol_event_message);
            // we are going to react to this automagically since the environment
            // will give us feedback about this (copium)
            let output = receiver.await;
            edits_made.push(output);
        }

        let edits_made = edits_made
            .into_iter()
            .filter_map(|edit| edit.ok())
            .collect::<Vec<_>>();

        // Now we can send these edits to the scratchpad to have a look
        let _ = self
            .reaction_sender
            .send(EnvironmentEventType::EditorStateChange(
                EditorStateChangeRequest::new(
                    edits_made,
                    "I fixed some diagnostic erorrs".to_owned(),
                ),
            ));
    }

    async fn _grab_diagnostics(&self) {
        let files_focussed;
        {
            files_focussed = self
                ._files_context
                .lock()
                .await
                .iter()
                .map(|file| file._file_path.to_owned())
                .collect::<Vec<_>>();
        }
        let diagnostics = self
            .tool_box
            .get_lsp_diagnostics_for_files(files_focussed, self.message_properties.clone())
            .await
            .unwrap_or_default();
        let _ = self
            .reaction_sender
            .send(EnvironmentEventType::LSP(LSPSignal::Diagnostics(
                diagnostics,
            )));
    }

    async fn is_fixing(&self) -> bool {
        let fixing;
        {
            fixing = *(self.fixing.lock().await);
        }
        fixing
    }

    async fn is_focussing(&self) -> bool {
        let focussing;
        {
            focussing = *(self.focussing.lock().await);
        }
        focussing
    }
}
