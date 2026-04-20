use crate::model::AppMsg;
use ratatui::{layout::Rect, Frame};
use tuirealm::{
    command::{Cmd, CmdResult},
    event::{Event, NoUserEvent},
    props::{AttrValue, Attribute},
    Component, MockComponent, State,
};

#[derive(Default)]
pub struct HeaderComponent;

impl MockComponent for HeaderComponent {
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

impl Component<AppMsg, NoUserEvent> for HeaderComponent {
    fn on(&mut self, ev: Event<NoUserEvent>) -> Option<AppMsg> {
        match ev {
            Event::Tick => Some(AppMsg::Tick),
            _ => None,
        }
    }
}
