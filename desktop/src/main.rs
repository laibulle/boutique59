use iced::{Application, Command, Element, Settings, Subscription};
use voxbox::{AmpControls, VoxBox};
use voxbox_ui::{Message, VoxBoxUi};

fn main() -> iced::Result {
    Desktop::run(Settings::default())
}

struct Desktop {
    ui: VoxBoxUi,
    plugin: VoxBox,
}

impl Application for Desktop {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Theme = iced::theme::Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        (
            Desktop {
                ui: VoxBoxUi::default(),
                plugin: VoxBox::default(),
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        String::from("VoxBox Desktop")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        self.ui.update(message);
        // Map current selected device's UI state to plugin controls and push to plugin
        if let Some(device) = self.ui.devices.get(self.ui.selected_index) {
            let controls = AmpControls {
                volume: device.gain,
                bass: device.bass,
                cut: device.cut,
                treble: device.treble,
                output: device.master,
            };
            self.plugin.set_ui_controls(controls);
        }
        Command::none()
    }

    fn view(&self) -> Element<'_, Message> {
        self.ui.view()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::none()
    }
}
