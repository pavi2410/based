use super::*;

impl ConnectionWizardPanel {
    pub(super) fn postgres_fields(&self, v: Div, muted: Hsla, cx: &Context<Self>) -> Div {
        v.child(
            h_flex()
                .gap_2()
                .child(labeled_field(
                    "Host",
                    muted,
                    Input::new(&self.host).cleanable(true).aria_label("Host"),
                ))
                .child(labeled_fixed(
                    "Port",
                    muted,
                    96.0,
                    Input::new(&self.port).cleanable(true).aria_label("Port"),
                )),
        )
        .child(
            h_flex()
                .gap_2()
                .child(labeled_field(
                    "User",
                    muted,
                    Input::new(&self.username)
                        .content_type(InputContentType::Username)
                        .cleanable(true)
                        .aria_label("User"),
                ))
                .child(labeled_field(
                    "Password",
                    muted,
                    Input::new(&self.password)
                        .content_type(InputContentType::Password)
                        .mask_toggle()
                        .aria_label("Password"),
                )),
        )
        .child(labeled_field(
            "Database",
            muted,
            Input::new(&self.database)
                .cleanable(true)
                .aria_label("Database"),
        ))
        .child(
            h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .child(div().text_sm().child("SSL"))
                .child(
                    Switch::new("wizard-ssl")
                        .with_size(Size::Small)
                        .checked(self.ssl_enabled)
                        .on_click(cx.listener(|panel, checked, _, cx| {
                            panel.ssl_enabled = *checked;
                            cx.notify();
                        })),
                ),
        )
        .when(self.ssl_enabled, |v| {
            v.child(labeled_field(
                "SSL mode",
                muted,
                Select::new(&self.ssl_mode).w_full(),
            ))
        })
        .child(
            h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .child(div().text_sm().child("SSH tunnel"))
                .child(
                    Switch::new("wizard-ssh")
                        .with_size(Size::Small)
                        .checked(self.ssh_enabled)
                        .on_click(cx.listener(|panel, checked, _, cx| {
                            panel.ssh_enabled = *checked;
                            cx.notify();
                        })),
                ),
        )
        .when(self.ssh_enabled, |v| {
            v.child(
                h_flex()
                    .gap_2()
                    .child(labeled_field(
                        "SSH host",
                        muted,
                        Input::new(&self.ssh_host)
                            .cleanable(true)
                            .aria_label("SSH host"),
                    ))
                    .child(labeled_fixed(
                        "SSH port",
                        muted,
                        96.0,
                        Input::new(&self.ssh_port)
                            .cleanable(true)
                            .aria_label("SSH port"),
                    )),
            )
            .child(labeled_field(
                "SSH user",
                muted,
                Input::new(&self.ssh_user)
                    .content_type(InputContentType::Username)
                    .cleanable(true)
                    .aria_label("SSH user"),
            ))
            .child(
                v_flex()
                    .gap_1()
                    .child(div().text_xs().text_color(muted).child("Key path"))
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Input::new(&self.ssh_key_path)
                                    .cleanable(true)
                                    .aria_label("SSH key path")
                                    .flex_1(),
                            )
                            .child(Button::new("wizard-ssh-browse").label("Browse…").on_click(
                                cx.listener(|panel, _, window, cx| {
                                    panel.browse_ssh_key(window, cx);
                                }),
                            )),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child("Leave empty to use ssh-agent."),
                    ),
            )
            .child(labeled_field(
                "Key passphrase",
                muted,
                Input::new(&self.ssh_key_passphrase)
                    .content_type(InputContentType::Password)
                    .mask_toggle()
                    .aria_label("SSH key passphrase"),
            ))
        })
    }

    pub(super) fn mongo_fields(&self, v: Div, muted: Hsla, cx: &Context<Self>) -> Div {
        v.child(labeled_field(
            "URI",
            muted,
            Input::new(&self.mongo_uri)
                .content_type(InputContentType::Url)
                .cleanable(true)
                .aria_label("Connection URI")
                .font_family(prefs::code_font_family(cx)),
        ))
        .child(labeled_field(
            "Database (optional)",
            muted,
            Input::new(&self.mongo_database)
                .cleanable(true)
                .aria_label("Database"),
        ))
    }

    pub(super) fn redis_fields(&self, v: Div, muted: Hsla, cx: &Context<Self>) -> Div {
        v.child(labeled_field(
            "URL",
            muted,
            Input::new(&self.redis_url)
                .content_type(InputContentType::Url)
                .cleanable(true)
                .aria_label("Connection URL")
                .font_family(prefs::code_font_family(cx)),
        ))
        .child(
            div()
                .text_xs()
                .text_color(muted)
                .child("Use rediss:// for TLS and /N to pick a DB index."),
        )
    }

    pub(super) fn sqlite_fields(&self, v: Div, muted: Hsla, cx: &Context<Self>) -> Div {
        v.child(
            v_flex()
                .gap_1()
                .child(div().text_xs().text_color(muted).child("Database path"))
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(
                            Input::new(&self.sqlite_path)
                                .cleanable(true)
                                .aria_label("Database path")
                                .flex_1(),
                        )
                        .child(
                            Button::new("wizard-sqlite-browse")
                                .label("Browse…")
                                .on_click(cx.listener(|panel, _, window, cx| {
                                    panel.browse_sqlite(window, cx);
                                })),
                        ),
                ),
        )
        .child(
            Checkbox::new("wizard-sqlite-ro")
                .label("Read-only")
                .checked(self.sqlite_read_only)
                .on_click(cx.listener(|panel, checked, _, cx| {
                    panel.sqlite_read_only = *checked;
                    cx.notify();
                })),
        )
    }
}
