use crate::model::{AppMsg, DeleteMsg};
use ratatui::{layout::Rect, Frame};
use tuirealm::{
    command::{Cmd, CmdResult},
    event::{Event, Key, KeyEvent, NoUserEvent},
    props::{AttrValue, Attribute},
    Component, MockComponent, State,
};

#[derive(Default)]
pub struct StatusBarComponent;

impl MockComponent for StatusBarComponent {
    fn view(&mut self, _frame: &mut Frame, _area: Rect) {}
    fn query(&self, _attr: Attribute) -> Option<AttrValue> {
        None
    }
    fn attr(&mut self, _attr: Attribute, _value: AttrValue) {}
    fn state(&self) -> State {
        State::None
    }
    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

impl Component<AppMsg, NoUserEvent> for StatusBarComponent {
    fn on(&mut self, ev: Event<NoUserEvent>) -> Option<AppMsg> {
        match ev {
            Event::Keyboard(KeyEvent {
                code: Key::Char('y'),
                ..
            }) => Some(AppMsg::Delete(DeleteMsg::ConfirmYes)),
            Event::Keyboard(KeyEvent {
                code: Key::Char('n'),
                ..
            })
            | Event::Keyboard(KeyEvent { code: Key::Esc, .. }) => {
                Some(AppMsg::Delete(DeleteMsg::ConfirmNo))
            }
            Event::Keyboard(KeyEvent {
                code: Key::Char('q'),
                ..
            }) => Some(AppMsg::Quit),
            _ => None,
        }
    }
}
