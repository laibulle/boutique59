use iced::widget::{
    button, checkbox, column, container, progress_bar, row, scrollable, slider, text,
};
use iced::{Alignment, Background, Color, Element, Length, Vector};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    Amp,
    Pedal,
}

struct SkeuoButton;

impl button::StyleSheet for SkeuoButton {
    type Style = iced::theme::Theme;

    fn active(&self, _style: &Self::Style) -> button::Appearance {
        button::Appearance {
            background: Some(Background::Color(Color::from_rgb(0.22, 0.20, 0.17))),
            border_radius: 16.0.into(),
            border_width: 1.0,
            border_color: Color::from_rgb(0.80, 0.75, 0.62),
            shadow_offset: Vector::new(0.0, 3.0),
            text_color: Color::from_rgb(0.95, 0.92, 0.82),
            ..button::Appearance::default()
        }
    }
}

struct SkeuoContainer(Color);

impl container::StyleSheet for SkeuoContainer {
    type Style = iced::theme::Theme;

    fn appearance(&self, _style: &Self::Style) -> container::Appearance {
        container::Appearance {
            text_color: Some(Color::WHITE),
            background: Some(Background::Color(self.0)),
            border_radius: 18.0.into(),
            border_width: 1.0,
            border_color: Color::from_rgb(0.70, 0.63, 0.51),
            ..container::Appearance::default()
        }
    }
}

struct SkeuoSlider;

impl slider::StyleSheet for SkeuoSlider {
    type Style = iced::theme::Theme;

    fn active(&self, _style: &Self::Style) -> slider::Appearance {
        slider::Appearance {
            rail: slider::Rail {
                colors: (
                    Color::from_rgb(0.44, 0.40, 0.35),
                    Color::from_rgb(0.15, 0.13, 0.10),
                ),
                width: 8.0,
                border_radius: 10.0.into(),
            },
            handle: slider::Handle {
                shape: slider::HandleShape::Circle { radius: 10.0 },
                color: Color::from_rgb(0.95, 0.92, 0.82),
                border_width: 2.0,
                border_color: Color::from_rgb(0.48, 0.43, 0.34),
            },
        }
    }

    fn hovered(&self, _style: &Self::Style) -> slider::Appearance {
        self.active(_style)
    }

    fn dragging(&self, _style: &Self::Style) -> slider::Appearance {
        self.active(_style)
    }
}

struct SkeuoProgressBar;

impl progress_bar::StyleSheet for SkeuoProgressBar {
    type Style = iced::theme::Theme;

    fn appearance(&self, _style: &Self::Style) -> progress_bar::Appearance {
        progress_bar::Appearance {
            background: Background::Color(Color::from_rgb(0.13, 0.11, 0.09)),
            bar: Background::Color(Color::from_rgb(0.82, 0.75, 0.60)),
            border_radius: 10.0.into(),
        }
    }
}

fn skeuo_container(background: Color) -> iced::theme::Container {
    iced::theme::Container::Custom(Box::new(SkeuoContainer(background)))
}

#[derive(Debug, Clone)]
pub enum Message {
    SelectDevice(usize),
    ToggleBypass(bool),
    GainChanged(f32),
    BassChanged(f32),
    TrebleChanged(f32),
    CutChanged(f32),
    MasterChanged(f32),
}

#[derive(Debug, Clone)]
pub struct DeviceState {
    pub name: String,
    pub kind: DeviceKind,
    pub bypassed: bool,
    pub gain: f32,
    pub bass: f32,
    pub treble: f32,
    pub cut: f32,
    pub master: f32,
}

impl DeviceState {
    pub fn new_amp(name: &str) -> Self {
        Self {
            name: name.to_string(),
            kind: DeviceKind::Amp,
            bypassed: false,
            gain: 0.55,
            bass: 0.50,
            treble: 0.60,
            cut: 0.35,
            master: 0.50,
        }
    }

    pub fn new_pedal(name: &str) -> Self {
        Self {
            name: name.to_string(),
            kind: DeviceKind::Pedal,
            bypassed: false,
            gain: 0.40,
            bass: 0.45,
            treble: 0.50,
            cut: 0.30,
            master: 0.70,
        }
    }
}

#[derive(Debug, Clone)]
pub struct VoxBoxUi {
    pub devices: Vec<DeviceState>,
    pub selected_index: usize,
}

impl Default for VoxBoxUi {
    fn default() -> Self {
        Self {
            devices: vec![
                DeviceState::new_amp("VoxBox Top Boost"),
                DeviceState::new_pedal("Crunch Pedal"),
                DeviceState::new_pedal("Reverb Pedal"),
            ],
            selected_index: 0,
        }
    }
}

impl VoxBoxUi {
    pub fn update(&mut self, message: Message) {
        match message {
            Message::SelectDevice(index) => {
                if index < self.devices.len() {
                    self.selected_index = index;
                }
            }
            Message::ToggleBypass(value) => {
                if let Some(device) = self.devices.get_mut(self.selected_index) {
                    device.bypassed = value;
                }
            }
            Message::GainChanged(value) => {
                if let Some(device) = self.devices.get_mut(self.selected_index) {
                    device.gain = value;
                }
            }
            Message::BassChanged(value) => {
                if let Some(device) = self.devices.get_mut(self.selected_index) {
                    device.bass = value;
                }
            }
            Message::TrebleChanged(value) => {
                if let Some(device) = self.devices.get_mut(self.selected_index) {
                    device.treble = value;
                }
            }
            Message::CutChanged(value) => {
                if let Some(device) = self.devices.get_mut(self.selected_index) {
                    device.cut = value;
                }
            }
            Message::MasterChanged(value) => {
                if let Some(device) = self.devices.get_mut(self.selected_index) {
                    device.master = value;
                }
            }
        }
    }

    fn render_control(
        &self,
        label: &str,
        value: f32,
        on_change: impl Fn(f32) -> Message + 'static,
        unit: &str,
    ) -> iced::widget::Row<'_, Message> {
        row![
            text(label).size(16).width(Length::Fixed(100.0)),
            slider(0.0..=1.0, value, on_change)
                .step(0.001)
                .width(Length::Fill)
                .style(iced::theme::Slider::Custom(Box::new(SkeuoSlider))),
            text(format!("{:.0}{}", value * 100.0, unit)).width(Length::Fixed(60.0)),
        ]
        .spacing(12)
        .align_items(Alignment::Center)
    }

    fn render_knob(&self, label: &str, value: f32) -> Element<'_, Message> {
        container(
            column![
                container(text(format!("{:.0}", value * 10.0)).size(16))
                    .width(Length::Fixed(52.0))
                    .height(Length::Fixed(52.0))
                    .center_x()
                    .center_y()
                    .style(skeuo_container(Color::from_rgb(0.20, 0.18, 0.15)))
                    .padding(8),
                text(label).size(12),
            ]
            .spacing(8)
            .align_items(Alignment::Center),
        )
        .style(skeuo_container(Color::from_rgb(0.24, 0.20, 0.16)))
        .padding(10)
        .width(Length::Fixed(84.0))
        .into()
    }

    fn render_amp_faceplate(&self, selected: &DeviceState) -> Element<'_, Message> {
        let status_color = if selected.bypassed {
            Color::from_rgb(0.66, 0.12, 0.12)
        } else {
            Color::from_rgb(0.20, 0.76, 0.24)
        };

        container(
            column![
                row![
                    column![
                        text(&selected.name).size(24),
                        text("Classic Tube Tone").size(14),
                    ]
                    .spacing(4)
                    .width(Length::Fill),
                    container(
                        text(if selected.bypassed { "MUTE" } else { "LIVE" })
                            .size(14)
                            .horizontal_alignment(iced::alignment::Horizontal::Center),
                    )
                    .padding(10)
                    .style(skeuo_container(status_color))
                    .width(Length::Fixed(90.0)),
                ]
                .spacing(16)
                .align_items(Alignment::Center),
                row![
                    self.render_knob("Gain", selected.gain),
                    self.render_knob("Bass", selected.bass),
                    self.render_knob("Treble", selected.treble),
                    self.render_knob("Cut", selected.cut),
                    self.render_knob("Master", selected.master),
                ]
                .spacing(12)
                .align_items(Alignment::Center),
            ]
            .spacing(20),
        )
        .style(skeuo_container(Color::from_rgb(0.12, 0.10, 0.08)))
        .padding(16)
        .width(Length::Fill)
        .into()
    }

    fn render_pedal_box(&self, selected: &DeviceState) -> Element<'_, Message> {
        let pedal_color = Color::from_rgb(0.14, 0.11, 0.09);
        let led_color = if selected.bypassed {
            Color::from_rgb(0.60, 0.06, 0.06)
        } else {
            Color::from_rgb(0.14, 0.92, 0.38)
        };

        container(
            column![
                text(&selected.name).size(22),
                row![
                    container(text(if selected.bypassed { "OFF" } else { "ON" }).size(14))
                        .padding(10)
                        .style(skeuo_container(Color::from_rgb(0.10, 0.09, 0.08))),
                    container(text(" "))
                        .width(Length::Fixed(16.0))
                        .height(Length::Fixed(16.0))
                        .style(skeuo_container(led_color))
                        .padding(4),
                    text("Stomp Switch").size(14),
                ]
                .spacing(12)
                .align_items(Alignment::Center),
                row![
                    self.render_knob("Gain", selected.gain),
                    self.render_knob("Tone", selected.treble),
                    self.render_knob("Level", selected.master),
                ]
                .spacing(14)
                .align_items(Alignment::Center),
            ]
            .spacing(18),
        )
        .style(skeuo_container(pedal_color))
        .padding(16)
        .width(Length::Fill)
        .into()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let selected = &self.devices[self.selected_index];

        let main_panel = if selected.kind == DeviceKind::Amp {
            self.render_amp_faceplate(selected)
        } else {
            self.render_pedal_box(selected)
        };

        let control_panel = container(
            column![
                main_panel,
                container(
                    column![
                        self.render_control("Gain", selected.gain, Message::GainChanged, "%"),
                        self.render_control("Bass", selected.bass, Message::BassChanged, "%"),
                        self.render_control("Treble", selected.treble, Message::TrebleChanged, "%"),
                        self.render_control("Cut", selected.cut, Message::CutChanged, "%"),
                        self.render_control("Output", selected.master, Message::MasterChanged, "%"),
                        row![
                            checkbox("Bypass", selected.bypassed, Message::ToggleBypass),
                            text(if selected.bypassed {
                                "bypassed"
                            } else {
                                "active"
                            })
                            .size(16),
                        ]
                        .spacing(12)
                        .align_items(Alignment::Center),
                    ]
                    .spacing(14)
                    .padding(14),
                )
                .style(skeuo_container(Color::from_rgb(0.18, 0.16, 0.13)))
                .width(Length::Fill),
            ]
            .spacing(20)
            .padding(12),
        )
        .style(skeuo_container(Color::from_rgb(0.16, 0.14, 0.11)))
        .padding(20)
        .width(Length::FillPortion(3));

        let chain_buttons = self.devices.iter().enumerate().fold(
            column![text("Pedalboard").size(20)],
            |column, (index, device)| {
                let label_color = if device.kind == DeviceKind::Amp {
                    Color::from_rgb(0.78, 0.68, 0.45)
                } else {
                    Color::from_rgb(0.54, 0.76, 0.98)
                };

                column.push(
                    button(
                        column![
                            row![
                                container(
                                    text(match device.kind {
                                        DeviceKind::Amp => "HEAD",
                                        DeviceKind::Pedal => "PEDAL",
                                    })
                                    .size(10)
                                    .horizontal_alignment(iced::alignment::Horizontal::Center)
                                )
                                .padding(6)
                                .style(skeuo_container(label_color)),
                                text(&device.name)
                                    .size(16)
                                    .width(Length::Fill)
                                    .horizontal_alignment(iced::alignment::Horizontal::Left),
                            ]
                            .spacing(10)
                            .align_items(Alignment::Center),
                            text(if device.bypassed {
                                "bypassed"
                            } else {
                                "active"
                            })
                            .size(12),
                        ]
                        .spacing(6),
                    )
                    .on_press(Message::SelectDevice(index))
                    .style(iced::theme::Button::custom(SkeuoButton))
                    .padding(12)
                    .width(Length::Fill),
                )
            },
        );

        let sidebar = container(scrollable(chain_buttons).width(Length::Fill))
            .padding(10)
            .style(skeuo_container(Color::from_rgb(0.15, 0.13, 0.10)))
            .width(Length::FillPortion(1));

        let meters = column![
            text("Level Meters").size(18),
            row![
                column![
                    text("Input"),
                    progress_bar(0.0..=1.0, selected.gain.clamp(0.0, 1.0))
                        .style(iced::theme::ProgressBar::Custom(Box::new(SkeuoProgressBar))),
                ]
                .spacing(6),
                column![
                    text("Output"),
                    progress_bar(0.0..=1.0, selected.master.clamp(0.0, 1.0))
                        .style(iced::theme::ProgressBar::Custom(Box::new(SkeuoProgressBar))),
                ]
                .spacing(6),
            ]
            .spacing(16)
            .width(Length::Fill),
        ]
        .spacing(10)
        .padding(10)
        .width(Length::FillPortion(1));

        let right_panel = column![control_panel, meters]
            .spacing(20)
            .width(Length::FillPortion(3));

        let layout = row![sidebar, right_panel].spacing(16).padding(20);

        container(layout)
            .center_x()
            .center_y()
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}
