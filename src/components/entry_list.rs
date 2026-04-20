use crate::model::{AppMsg, DeleteMsg, ListMsg};
use ratatui::{layout::Rect, Frame};
use tuirealm::{
    command::{Cmd, CmdResult},
    event::{Event, Key, KeyEvent, NoUserEvent},
    props::{AttrValue, Attribute},
    Component, MockComponent, State,
};

#[derive(Default)]
pub struct EntryListComponent;

impl MockComponent for EntryListComponent {
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

impl Component<AppMsg, NoUserEvent> for EntryListComponent {
    fn on(&mut self, ev: Event<NoUserEvent>) -> Option<AppMsg> {
        match ev {
            Event::Keyboard(KeyEvent { code: Key::Up, .. }) => Some(AppMsg::List(ListMsg::MoveUp)),
            Event::Keyboard(KeyEvent {
                code: Key::Down, ..
            }) => Some(AppMsg::List(ListMsg::MoveDown)),
            Event::Keyboard(KeyEvent {
                code: Key::Char(' '),
                ..
            }) => Some(AppMsg::List(ListMsg::ToggleSelect)),
            Event::Keyboard(KeyEvent {
                code: Key::Char('a'),
                ..
            }) => Some(AppMsg::List(ListMsg::SelectAll)),
            Event::Keyboard(KeyEvent {
                code: Key::Char('d'),
                ..
            }) => Some(AppMsg::Delete(DeleteMsg::Request)),
            Event::Keyboard(KeyEvent {
                code: Key::Char('q'),
                ..
            }) => Some(AppMsg::Quit),
            _ => None,
        }
    }
}
