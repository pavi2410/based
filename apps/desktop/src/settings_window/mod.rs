//! Separate settings window (theme, typography, query defaults).

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    App, Context, Entity, FocusHandle, Focusable, IntoElement, ParentElement, Render,
    StyleRefinement, Styled, Window, div, prelude::*, px,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, IndexPath, ThemeMode,
    group_box::GroupBoxVariant,
    searchable_list::SearchableListItem,
    select::{SelectEvent, SelectState},
    setting::{NumberFieldOptions, SettingField, SettingGroup, SettingItem, SettingPage, Settings},
    v_flex,
};

use crate::app::prefs::{self, DEFAULT_PAGE_SIZE, DEFAULT_QUERY_TIMEOUT_SECS};
use crate::theme::{
    ThemeNameItem, ThemePreviewAxis, ThemePreviewSession, appearance_segmented, theme_name_select,
};

#[cfg(target_os = "macos")]
fn macos_settings_header_style() -> StyleRefinement {
    StyleRefinement::default().pt(px(32.0))
}

struct ThemeSelectState {
    select: Entity<SelectState<Vec<ThemeNameItem>>>,
    trigger_focus: FocusHandle,
    preview: Rc<RefCell<ThemePreviewSession>>,
    menu_was_open: bool,
}

impl ThemeSelectState {
    fn new(
        axis: ThemePreviewAxis,
        mode: ThemeMode,
        active_name: &str,
        window: &mut Window,
        cx: &mut Context<SettingsWindow>,
    ) -> Self {
        let items = ThemeNameItem::items_for_mode(mode, cx);
        let selected_index = items
            .iter()
            .position(|item| item.value().as_ref() == active_name)
            .map(IndexPath::new);

        let select = cx.new(|cx| SelectState::new(items, selected_index, window, cx));
        let trigger_focus = select.read(cx).focus_handle(cx);
        let preview = Rc::new(RefCell::new(ThemePreviewSession::new(axis)));
        let select_observe = select.clone();
        let preview_on_confirm = preview.clone();

        cx.subscribe(
            &select_observe,
            move |_, _select, event: &SelectEvent<Vec<ThemeNameItem>>, cx| {
                let SelectEvent::Confirm(Some(name)) = event else {
                    return;
                };
                match axis {
                    ThemePreviewAxis::Light => prefs::apply_light_theme(name.as_ref(), None, cx),
                    ThemePreviewAxis::Dark => prefs::apply_dark_theme(name.as_ref(), None, cx),
                }
                preview_on_confirm.borrow_mut().clear_after_commit();
            },
        )
        .detach();

        Self {
            select,
            trigger_focus,
            preview,
            menu_was_open: false,
        }
    }

    fn menu_open(&self, cx: &App) -> bool {
        self.select.read(cx).focus_handle(cx) != self.trigger_focus
    }

    fn sync_preview(&self, window: &mut Window, cx: &mut App) {
        let Some(ix) = self.select.read(cx).selected_index(cx) else {
            return;
        };
        let mode = match self.preview.borrow().axis {
            ThemePreviewAxis::Light => ThemeMode::Light,
            ThemePreviewAxis::Dark => ThemeMode::Dark,
        };
        let items = ThemeNameItem::items_for_mode(mode, cx);
        let Some(item) = items.get(ix.row) else {
            return;
        };
        self.preview
            .borrow_mut()
            .preview(item.value().as_ref(), Some(window), cx);
    }

    fn handle_menu_closed(&mut self, window: &mut Window, cx: &mut App) {
        if !self.preview.borrow().is_active() {
            return;
        }
        let committed = match self.preview.borrow().axis {
            ThemePreviewAxis::Light => prefs::light_theme_name(cx),
            ThemePreviewAxis::Dark => prefs::dark_theme_name(cx),
        };
        let selected = self
            .select
            .read(cx)
            .selected_value()
            .map(|value| value.as_ref());
        if selected == Some(committed) {
            self.preview.borrow_mut().revert(Some(window), cx);
        } else {
            self.preview.borrow_mut().clear_after_commit();
        }
    }

    fn tick(&mut self, window: &mut Window, cx: &mut App) -> bool {
        let open = self.menu_open(cx);
        if open {
            self.sync_preview(window, cx);
        } else if self.menu_was_open {
            self.handle_menu_closed(window, cx);
        }
        self.menu_was_open = open;
        open
    }
}

pub struct SettingsWindow {
    focus_handle: FocusHandle,
    light_theme: ThemeSelectState,
    dark_theme: ThemeSelectState,
}

impl SettingsWindow {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let light_name = prefs::light_theme_name(cx).to_string();
        let dark_name = prefs::dark_theme_name(cx).to_string();
        Self {
            focus_handle: cx.focus_handle(),
            light_theme: ThemeSelectState::new(
                ThemePreviewAxis::Light,
                ThemeMode::Light,
                &light_name,
                window,
                cx,
            ),
            dark_theme: ThemeSelectState::new(
                ThemePreviewAxis::Dark,
                ThemeMode::Dark,
                &dark_name,
                window,
                cx,
            ),
        }
    }

    fn pages(&self, _: &mut Window, _cx: &mut Context<Self>) -> Vec<SettingPage> {
        let light_select = self.light_theme.select.clone();
        let dark_select = self.dark_theme.select.clone();
        vec![
            SettingPage::new("Appearance")
                .default_open(true)
                .icon(Icon::new(IconName::Settings2))
                .groups(vec![
                    SettingGroup::new().title("Theme").items(vec![
                        SettingItem::new(
                            "Appearance",
                            SettingField::render(|_, _window, cx| {
                                appearance_segmented("settings", prefs::appearance_mode(cx))
                            }),
                        )
                        .description("Light, dark, or match the system."),
                        SettingItem::new(
                            "Light theme",
                            SettingField::render(move |_, _, _cx| {
                                theme_name_select(light_select.clone())
                            }),
                        )
                        .layout(gpui::Axis::Vertical)
                        .description(
                            "Theme used in light mode. Arrow keys preview before you confirm.",
                        ),
                        SettingItem::new(
                            "Dark theme",
                            SettingField::render(move |_, _, _cx| {
                                theme_name_select(dark_select.clone())
                            }),
                        )
                        .layout(gpui::Axis::Vertical)
                        .description(
                            "Theme used in dark mode. Arrow keys preview before you confirm.",
                        ),
                    ]),
                    SettingGroup::new().title("Typography").items(vec![
                        SettingItem::new(
                            "UI font size",
                            SettingField::number_input(
                                NumberFieldOptions {
                                    min: 10.0,
                                    max: 24.0,
                                    step: 1.0,
                                },
                                |cx| prefs::ui_font_size(cx) as f64,
                                |val, cx| prefs::set_ui_font_size(val as f32, cx),
                            )
                            .default_value(prefs::DEFAULT_UI_FONT_SIZE as f64),
                        )
                        .description("Base UI font size in pixels."),
                        SettingItem::new(
                            "Monospace font size",
                            SettingField::number_input(
                                NumberFieldOptions {
                                    min: 10.0,
                                    max: 22.0,
                                    step: 1.0,
                                },
                                |cx| prefs::mono_font_size(cx) as f64,
                                |val, cx| prefs::set_mono_font_size(val as f32, cx),
                            )
                            .default_value(prefs::DEFAULT_MONO_FONT_SIZE as f64),
                        )
                        .description("Monospace font size for editors and SQL."),
                    ]),
                    SettingGroup::new().title("Tables").items(vec![
                        SettingItem::new(
                            "Compact tables",
                            SettingField::switch(
                                |cx| prefs::table_density(cx) == prefs::TableDensity::Compact,
                                |on, cx| {
                                    prefs::set_table_density(
                                        if on {
                                            prefs::TableDensity::Compact
                                        } else {
                                            prefs::TableDensity::Comfortable
                                        },
                                        cx,
                                    );
                                },
                            ),
                        )
                        .description(
                            "Tighter monospace rows in data grids; turn off for roomier cells.",
                        ),
                        SettingItem::new(
                            "Zebra striping",
                            SettingField::switch(
                                |cx| prefs::table_prefs(cx).stripe,
                                prefs::set_table_stripe,
                            ),
                        )
                        .description("Alternate row shading in data grids."),
                    ]),
                ]),
            SettingPage::new("Query defaults")
                .icon(Icon::new(IconName::Settings))
                .groups(vec![
                    SettingGroup::new().title("Data fetching").items(vec![
                        SettingItem::new(
                            "Page size",
                            SettingField::number_input(
                                NumberFieldOptions {
                                    min: 50.0,
                                    max: 5000.0,
                                    step: 50.0,
                                },
                                |cx| prefs::page_size(cx) as f64,
                                |val, cx| prefs::set_page_size(val as u64, cx),
                            )
                            .default_value(DEFAULT_PAGE_SIZE as f64),
                        )
                        .description("Rows fetched per page in SQL table viewers."),
                        SettingItem::new(
                            "Query timeout",
                            SettingField::number_input(
                                NumberFieldOptions {
                                    min: 5.0,
                                    max: 600.0,
                                    step: 5.0,
                                },
                                |cx| prefs::query_timeout_secs(cx) as f64,
                                |val, cx| prefs::set_query_timeout_secs(val as u32, cx),
                            )
                            .default_value(DEFAULT_QUERY_TIMEOUT_SECS as f64),
                        )
                        .description(
                            "Maximum seconds to wait for a query (enforcement in editors is planned).",
                        ),
                    ]),
                    SettingGroup::new().title("Table interaction").items(vec![
                        SettingItem::new(
                            "Sortable columns",
                            SettingField::switch(
                                |cx| prefs::table_prefs(cx).sortable,
                                prefs::set_table_sortable,
                            ),
                        )
                        .description("Click column headers to sort results."),
                        SettingItem::new(
                            "Resize columns",
                            SettingField::switch(
                                |cx| prefs::table_prefs(cx).col_resizable,
                                prefs::set_table_col_resizable,
                            ),
                        )
                        .description("Drag column borders to change width."),
                        SettingItem::new(
                            "Reorder columns",
                            SettingField::switch(
                                |cx| prefs::table_prefs(cx).col_movable,
                                prefs::set_table_col_movable,
                            ),
                        )
                        .description("Drag column headers to rearrange."),
                        SettingItem::new(
                            "Row selection",
                            SettingField::switch(
                                |cx| prefs::table_prefs(cx).row_selectable,
                                prefs::set_table_row_selectable,
                            ),
                        )
                        .description("Select entire rows in data grids."),
                        SettingItem::new(
                            "Cell selection",
                            SettingField::switch(
                                |cx| prefs::table_prefs(cx).cell_selectable,
                                prefs::set_table_cell_selectable,
                            ),
                        )
                        .description("Select individual cells; enables keyboard navigation."),
                        SettingItem::new(
                            "Loop selection",
                            SettingField::switch(
                                |cx| prefs::table_prefs(cx).loop_selection,
                                prefs::set_table_loop_selection,
                            ),
                        )
                        .description("Keyboard selection wraps at table edges."),
                    ]),
                ]),
        ]
    }
}

impl Focusable for SettingsWindow {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let light_open = self.light_theme.tick(window, cx);
        let dark_open = self.dark_theme.tick(window, cx);
        if light_open || dark_open {
            cx.notify();
        }

        let bg = cx.theme().background;
        let fg = cx.theme().foreground;

        let mut settings = Settings::new("based-settings")
            .with_group_variant(GroupBoxVariant::Outline)
            .pages(self.pages(window, cx));
        #[cfg(target_os = "macos")]
        {
            settings = settings.header_style(&macos_settings_header_style());
        }

        v_flex()
            .size_full()
            .bg(bg)
            .text_color(fg)
            .when(!cfg!(target_os = "macos"), |this| this.pt(px(36.0)))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .size_full()
                    .bg(bg)
                    .text_color(fg)
                    .child(settings),
            )
    }
}
