//! AI settings page: provider, model, endpoint and keychain-backed API keys.

use based_ai::ProviderKind;
use based_storage::SecretStore;
use gpui_kit::component::{
    ActiveTheme, Disableable as _, Icon, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputState},
    setting::{SettingField, SettingGroup, SettingItem, SettingPage},
};
use gpui_kit::{
    App, Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled, Window, div,
    prelude::*,
};

use crate::app::prefs;

pub fn page() -> SettingPage {
    SettingPage::new("AI")
        .icon(Icon::new(IconName::Bot))
        .groups(vec![model_group(), keys_group()])
}

fn model_group() -> SettingGroup {
    let providers = ProviderKind::ALL
        .iter()
        .map(|k| (k.as_str().into(), k.label().into()))
        .collect();
    SettingGroup::new().title("Model").items(vec![
        SettingItem::new(
            "Provider",
            SettingField::dropdown(
                providers,
                |cx: &App| prefs::ai_prefs(cx).provider().as_str().into(),
                |val: SharedString, cx: &mut App| {
                    if let Some(kind) = ProviderKind::from_id(&val) {
                        prefs::set_ai_provider(kind, cx);
                    }
                },
            )
            .default_value(ProviderKind::default().as_str().to_string()),
        )
        .description("Backend used for generating and explaining queries."),
        SettingItem::new(
            "Model",
            SettingField::input(
                |cx: &App| prefs::ai_prefs(cx).model().to_owned().into(),
                |val: SharedString, cx: &mut App| prefs::set_ai_model(val.to_string(), cx),
            ),
        )
        .description("Resets to the provider default when the provider changes."),
        SettingItem::new(
            "Endpoint",
            SettingField::input(
                |cx: &App| {
                    prefs::ai_prefs(cx)
                        .base_url()
                        .unwrap_or_default()
                        .to_owned()
                        .into()
                },
                |val: SharedString, cx: &mut App| prefs::set_ai_base_url(val.to_string(), cx),
            ),
        )
        .description("Optional base URL for proxies, OpenAI-compatible servers or remote Ollama."),
    ])
}

fn keys_group() -> SettingGroup {
    let items: Vec<SettingItem> = ProviderKind::ALL
        .into_iter()
        .filter(|k| k.needs_key())
        .map(|kind| {
            SettingItem::new(
                format!("{} API key", kind.label()),
                SettingField::render(move |_, window, cx| {
                    let id = SharedString::from(format!("ai-key-{}", kind.as_str()));
                    window.use_keyed_state(id, cx, |window, cx| KeyField::new(kind, window, cx))
                }),
            )
        })
        .collect();
    SettingGroup::new().title("API keys").items(items)
}

enum KeyStatus {
    Checking,
    Busy,
    Set,
    Unset,
    Failed(SharedString),
}

/// Masked key entry backed by the OS keychain. Keychain calls run off the UI thread.
struct KeyField {
    provider: ProviderKind,
    input: Entity<InputState>,
    status: KeyStatus,
}

impl KeyField {
    fn new(provider: ProviderKind, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder("Paste API key")
        });
        let mut field = Self {
            provider,
            input,
            status: KeyStatus::Checking,
        };
        let id = provider.as_str();
        field.run(cx, move |s| s.get_ai_key(id).map(|k| k.is_some()));
        field
    }

    /// Runs a blocking keychain job; its result (is the key set?) becomes the status.
    fn run(
        &mut self,
        cx: &mut Context<Self>,
        job: impl FnOnce(SecretStore) -> anyhow::Result<bool> + Send + 'static,
    ) {
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { job(SecretStore::new()) })
                .await;
            let _ = this.update(cx, |field, cx| {
                field.status = match result {
                    Ok(true) => KeyStatus::Set,
                    Ok(false) => KeyStatus::Unset,
                    Err(e) => KeyStatus::Failed(format!("{e:#}").into()),
                };
                cx.notify();
            });
        })
        .detach();
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let key = self.input.read(cx).value().trim().to_owned();
        if key.is_empty() {
            return;
        }
        self.input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.status = KeyStatus::Busy;
        cx.notify();
        let id = self.provider.as_str();
        self.run(cx, move |s| s.set_ai_key(id, &key).map(|()| true));
    }

    fn remove(&mut self, cx: &mut Context<Self>) {
        self.status = KeyStatus::Busy;
        cx.notify();
        let id = self.provider.as_str();
        self.run(cx, move |s| s.delete_ai_key(id).map(|()| false));
    }
}

impl Render for KeyField {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (text, busy, is_set) = match &self.status {
            KeyStatus::Checking => ("Checking...".into(), true, false),
            KeyStatus::Busy => ("Working...".into(), true, false),
            KeyStatus::Set => ("Saved in keychain".into(), false, true),
            KeyStatus::Unset => ("Not set".into(), false, false),
            KeyStatus::Failed(e) => (e.clone(), false, false),
        };
        h_flex()
            .gap_2()
            .items_center()
            .child(Input::new(&self.input).mask_toggle().w_64())
            .child(
                Button::new("ai-key-save")
                    .small()
                    .label("Save")
                    .disabled(busy)
                    .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
            )
            .when(is_set, |row| {
                row.child(
                    Button::new("ai-key-remove")
                        .small()
                        .ghost()
                        .label("Remove")
                        .disabled(busy)
                        .on_click(cx.listener(|this, _, _, cx| this.remove(cx))),
                )
            })
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(text),
            )
    }
}
