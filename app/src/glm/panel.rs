//! GLM Assistant panel — minimal, modal-style View.
//!
//! Scope of this Step 1.4 cut: the panel exists as a self-contained
//! WarpUI `View` that compiles cleanly. It owns:
//! - a `ViewHandle<EditorView>` for the user's prompt,
//! - a `ModelHandle<GlmConversation>` for chat state,
//! - a small set of `MouseStateHandle`s for buttons.
//!
//! What this DOES render today:
//! - "GLM Assistant · {model}" header.
//! - A column of message rows (one per `ChatMessage`).
//! - The pending assistant text, if a response is in flight.
//! - A small state line ("Idle" / "Generating…" / "Error: …").
//! - The prompt editor + Send / Stop / Reset buttons.
//!
//! What is NOT yet wired:
//! - Inserting this panel into the workspace render tree (Step 1.5).
//! - Markdown rendering / syntax highlighting (Phase 2+).
//! - Token-by-token streaming UX (Step 1.3.5; see conversation.rs).

use std::sync::Arc;

use warpui::elements::{
    Container, CrossAxisAlignment, Element, Empty, Flex, MainAxisAlignment, MainAxisSize,
    MouseStateHandle, Padding, ParentElement, Text,
};
use warpui::fonts::{Properties, Style, Weight};
use warpui::platform::Cursor;
use warpui::presenter::ChildView;
use warpui::ui_components::button::ButtonVariant;
use warpui::ui_components::components::UiComponent;
use warpui::{
    AppContext, Entity, ModelHandle, SingletonEntity, TypedActionView, View, ViewContext,
    ViewHandle,
};

use super::client::GlmClient;
use super::conversation::{Event as ConversationEvent, GlmConversation, State as ConversationState};
use super::settings::GlmSettings;
use super::types::{ChatMessage, ChatRole};
use crate::appearance::Appearance;
use crate::editor::{EditorOptions, EditorView, TextOptions};

const HEADER_TITLE: &str = "GLM Assistant";
const PLACEHOLDER_TEXT: &str = "Ask GLM anything…";
const SEND_BUTTON_LABEL: &str = "Send";
const STOP_BUTTON_LABEL: &str = "Stop";
const RESET_BUTTON_LABEL: &str = "New chat";
const NO_API_KEY_MESSAGE: &str =
    "No API key configured. Add one in Settings → GLM Assistant before sending.";
const EMPTY_HISTORY_HINT: &str = "Type a question below to start a new conversation.";

const TITLE_FONT_SIZE: f32 = 16.0;
const BODY_FONT_SIZE: f32 = 13.0;
const SMALL_FONT_SIZE: f32 = 12.0;

const PANEL_PADDING: f32 = 12.0;
const SECTION_GAP: f32 = 10.0;

#[derive(Default)]
struct MouseStateHandles {
    send: MouseStateHandle,
    stop: MouseStateHandle,
    reset: MouseStateHandle,
}

pub struct GlmAssistantPanel {
    conversation: ModelHandle<GlmConversation>,
    editor: ViewHandle<EditorView>,
    has_api_key: bool,
    mouse_state_handles: MouseStateHandles,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GlmAssistantPanelEvent {
    /// Operator requested the panel be dismissed.
    Close,
}

#[derive(Clone, Copy, Debug)]
pub enum GlmAssistantPanelAction {
    Send,
    Stop,
    Reset,
    Close,
}

impl GlmAssistantPanel {
    /// Build the panel with the given conversation handle. The caller is
    /// responsible for constructing `GlmConversation` with a `GlmClient`
    /// loaded from current `GlmSettings` + the API key from secure storage.
    pub fn new(
        conversation: ModelHandle<GlmConversation>,
        has_api_key: bool,
        ctx: &mut ViewContext<Self>,
    ) -> Self {
        let editor = ctx.add_typed_action_view(|ctx| {
            let appearance = Appearance::as_ref(ctx);
            let options = EditorOptions {
                text: TextOptions::ui_text(Some(BODY_FONT_SIZE), appearance),
                autogrow: true,
                soft_wrap: true,
                supports_vim_mode: true,
                ..Default::default()
            };
            EditorView::new(options, ctx)
        });
        editor.update(ctx, |editor, ctx| {
            editor.set_placeholder_text(PLACEHOLDER_TEXT, ctx);
        });

        ctx.subscribe_to_model(&conversation, |_me, _, event, ctx| {
            match event {
                ConversationEvent::Changed | ConversationEvent::Finished { .. } => ctx.notify(),
            }
        });
        ctx.observe(&conversation, |_, _, ctx| ctx.notify());

        Self {
            conversation,
            editor,
            has_api_key,
            mouse_state_handles: MouseStateHandles::default(),
        }
    }

    pub fn conversation(&self) -> &ModelHandle<GlmConversation> {
        &self.conversation
    }

    pub fn set_has_api_key(&mut self, has_key: bool, ctx: &mut ViewContext<Self>) {
        if self.has_api_key != has_key {
            self.has_api_key = has_key;
            ctx.notify();
        }
    }

    /// Refresh non-secret settings (model, base URL, system prompt, ...).
    /// Caller is responsible for swapping the underlying `GlmClient` if the
    /// base URL / API key changed.
    pub fn apply_settings(&mut self, settings: GlmSettings, ctx: &mut ViewContext<Self>) {
        self.conversation.update(ctx, |conv, ctx| {
            conv.set_settings(settings, ctx);
        });
    }

    /// Replace the underlying `GlmClient` (e.g. after API key rotation).
    pub fn replace_client(&mut self, client: Arc<GlmClient>, ctx: &mut ViewContext<Self>) {
        self.conversation.update(ctx, |conv, _| {
            conv.replace_client(client);
        });
    }

    fn send_current_input(&mut self, ctx: &mut ViewContext<Self>) {
        if !self.has_api_key {
            log::warn!("GLM Assistant: send ignored — no API key configured");
            return;
        }
        let prompt = self.editor.update(ctx, |editor, ctx| {
            let text = editor.buffer_text(ctx).to_string();
            editor.clear_buffer_and_reset_undo_stack(ctx);
            text
        });
        let trimmed = prompt.trim();
        if trimmed.is_empty() {
            return;
        }
        let prompt_owned = trimmed.to_string();
        self.conversation.update(ctx, |conv, ctx| {
            conv.send(prompt_owned, ctx);
        });
    }

    fn stop_request(&mut self, ctx: &mut ViewContext<Self>) {
        self.conversation.update(ctx, |conv, _| {
            conv.cancel_in_flight();
        });
        ctx.notify();
    }

    fn reset_conversation(&mut self, ctx: &mut ViewContext<Self>) {
        self.conversation.update(ctx, |conv, ctx| {
            conv.reset(ctx);
        });
    }

    fn close(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.emit(GlmAssistantPanelEvent::Close);
    }

    fn render_header(&self, app: &AppContext, appearance: &Appearance) -> Box<dyn Element> {
        let theme = appearance.theme();
        let model_name = self.conversation.as_ref(app).settings().model.clone();
        let title = Text::new(
            format!("{HEADER_TITLE} · {model_name}"),
            appearance.ui_font_family(),
            TITLE_FONT_SIZE,
        )
        .with_style(Properties {
            style: Style::Normal,
            weight: Weight::Bold,
        })
        .with_color(theme.active_ui_text_color().into())
        .finish();
        Container::new(title)
            .with_padding_bottom(SECTION_GAP)
            .finish()
    }

    fn render_messages(&self, app: &AppContext, appearance: &Appearance) -> Box<dyn Element> {
        let conversation = self.conversation.as_ref(app);
        let theme = appearance.theme();

        if conversation.history().is_empty() && conversation.pending_assistant().is_none() {
            let hint_text = if self.has_api_key {
                EMPTY_HISTORY_HINT
            } else {
                NO_API_KEY_MESSAGE
            };
            let hint = Text::new(hint_text, appearance.ui_font_family(), BODY_FONT_SIZE)
                .with_color(theme.nonactive_ui_text_color().into())
                .finish();
            return hint;
        }

        let mut column = Flex::column()
            .with_main_axis_size(MainAxisSize::Min)
            .with_cross_axis_alignment(CrossAxisAlignment::Start);

        for msg in conversation.history() {
            column = column.with_child(render_message(msg, appearance));
        }

        if let Some(pending) = conversation.pending_assistant() {
            let pending_msg = ChatMessage::assistant(pending.to_string());
            column = column.with_child(render_message(&pending_msg, appearance));
        }

        column.finish()
    }

    fn render_state_line(&self, app: &AppContext, appearance: &Appearance) -> Box<dyn Element> {
        let theme = appearance.theme();
        let conversation = self.conversation.as_ref(app);
        let label = match conversation.state() {
            ConversationState::Idle => match conversation.last_usage() {
                Some(usage) => format!("Idle · last turn used {} tokens", usage.total_tokens),
                None => "Idle".to_string(),
            },
            ConversationState::InFlight => "Generating…".to_string(),
            ConversationState::Error(msg) => format!("Error: {msg}"),
        };
        Text::new(label, appearance.ui_font_family(), SMALL_FONT_SIZE)
            .with_color(theme.nonactive_ui_text_color().into())
            .finish()
    }

    fn render_buttons(&self, app: &AppContext, appearance: &Appearance) -> Box<dyn Element> {
        let ui_builder = appearance.ui_builder();
        let in_flight = self.conversation.as_ref(app).state().is_in_flight();

        let send_or_stop = if in_flight {
            ui_builder
                .button(
                    ButtonVariant::Warn,
                    self.mouse_state_handles.stop.clone(),
                )
                .with_text_label(STOP_BUTTON_LABEL.to_string())
                .build()
                .on_click(|ctx, _, _| ctx.dispatch_typed_action(GlmAssistantPanelAction::Stop))
                .with_cursor(Cursor::PointingHand)
                .finish()
        } else {
            ui_builder
                .button(
                    ButtonVariant::Accent,
                    self.mouse_state_handles.send.clone(),
                )
                .with_text_label(SEND_BUTTON_LABEL.to_string())
                .build()
                .on_click(|ctx, _, _| ctx.dispatch_typed_action(GlmAssistantPanelAction::Send))
                .with_cursor(Cursor::PointingHand)
                .finish()
        };

        let reset = ui_builder
            .button(
                ButtonVariant::Basic,
                self.mouse_state_handles.reset.clone(),
            )
            .with_text_label(RESET_BUTTON_LABEL.to_string())
            .build()
            .on_click(|ctx, _, _| ctx.dispatch_typed_action(GlmAssistantPanelAction::Reset))
            .with_cursor(Cursor::PointingHand)
            .finish();

        Flex::row()
            .with_main_axis_alignment(MainAxisAlignment::End)
            .with_main_axis_size(MainAxisSize::Min)
            .with_child(Container::new(reset).with_padding_right(8.0).finish())
            .with_child(send_or_stop)
            .finish()
    }

    fn render_panel(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let editor_view = ChildView::new(&self.editor).finish();

        let body = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(self.render_header(app, appearance))
            .with_child(self.render_messages(app, appearance))
            .with_child(
                Container::new(self.render_state_line(app, appearance))
                    .with_padding_top(SECTION_GAP)
                    .with_padding_bottom(SECTION_GAP / 2.0)
                    .finish(),
            )
            .with_child(editor_view)
            .with_child(
                Container::new(self.render_buttons(app, appearance))
                    .with_padding_top(SECTION_GAP)
                    .finish(),
            )
            .finish();

        Container::new(body)
            .with_padding(Padding::uniform(PANEL_PADDING))
            .finish()
    }
}

#[allow(unused)]
fn render_message(msg: &ChatMessage, appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();
    let label = match msg.role {
        ChatRole::System => "system",
        ChatRole::User => "you",
        ChatRole::Assistant => "glm",
    };
    let header = Text::new(label, appearance.ui_font_family(), SMALL_FONT_SIZE)
        .with_style(Properties {
            style: Style::Normal,
            weight: Weight::Bold,
        })
        .with_color(theme.nonactive_ui_text_color().into())
        .finish();
    let body = Text::new(
        msg.content.clone(),
        appearance.ui_font_family(),
        BODY_FONT_SIZE,
    )
    .with_color(theme.active_ui_text_color().into())
    .finish();
    Container::new(
        Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Start)
            .with_main_axis_size(MainAxisSize::Min)
            .with_child(header)
            .with_child(body)
            .finish(),
    )
    .with_padding_bottom(SECTION_GAP / 2.0)
    .finish()
}

impl Entity for GlmAssistantPanel {
    type Event = GlmAssistantPanelEvent;
}

impl TypedActionView for GlmAssistantPanel {
    type Action = GlmAssistantPanelAction;

    fn handle_action(&mut self, action: &GlmAssistantPanelAction, ctx: &mut ViewContext<Self>) {
        match action {
            GlmAssistantPanelAction::Send => self.send_current_input(ctx),
            GlmAssistantPanelAction::Stop => self.stop_request(ctx),
            GlmAssistantPanelAction::Reset => self.reset_conversation(ctx),
            GlmAssistantPanelAction::Close => self.close(ctx),
        }
    }
}

impl View for GlmAssistantPanel {
    fn ui_name() -> &'static str {
        "GlmAssistantPanel"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        self.render_panel(app)
    }
}

/// Suppress dead-code warnings for the empty-state hint when the unused
/// branch is the only consumer.
const _: &str = EMPTY_HISTORY_HINT;
