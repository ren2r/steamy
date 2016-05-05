use std::cell::RefCell;
use uinput;
use {Result as Res, Error};
use config::{self, Config, group, binding, Binding};
use input::{self, Event};
use super::util::iter;
use super::{button_diamond, trackpad_left};

pub struct Mapper {
	pub(super) config: Config,
	pub(super) device: RefCell<uinput::Device>,
	pub(super) preset: u32,
}

impl Mapper {
	pub fn new(config: Config) -> Res<Self> {
		let builder = uinput::default()?.name("steamy")?;

		// Enable events from modes.
		let builder = config.groups.iter()
			.map(|(_, group)|
				group.mode)
			.fold(builder, |builder, mode|
				match mode {
					group::Mode::JoystickMove | group::Mode::MouseJoystick =>
						builder.event(uinput::event::absolute::Position::X).unwrap().min(-32768).max(32767).fuzz(16).flat(128)
						       .event(uinput::event::absolute::Position::Y).unwrap().min(-32768).max(32767).fuzz(16).flat(128),

					group::Mode::AbsoluteMouse =>
						builder.event(uinput::event::relative::Position::X).unwrap()
						       .event(uinput::event::relative::Position::Y).unwrap(),

					_ =>
						builder
				});

		// Enable events from bindings.
		let builder = config.groups.iter()
			.flat_map(|(_, group)|
				match group.bindings {
					group::Bindings::FourButtons { ref a, ref b, ref x, ref y } =>
						iter(a.iter().chain(b.iter()).chain(x.iter()).chain(y.iter())),

					group::Bindings::DPad { ref north, ref south, ref east, ref west, ref click } =>
						iter(north.iter().chain(south.iter()).chain(east.iter()).chain(west.iter()).chain(click.iter())),

					group::Bindings::AbsoluteMouse { ref click, ref double } =>
						iter(click.iter().chain(double.iter())),

					group::Bindings::Trigger { ref click } =>
						iter(click.iter()),

					group::Bindings::ScrollWheel { ref cw, ref ccw, ref click } =>
						iter(cw.iter().chain(ccw.iter()).chain(click.iter())),

					group::Bindings::MouseJoystick { ref click } =>
						iter(click.iter()),

					group::Bindings::JoystickMove { ref click } =>
						iter(click.iter()),

					group::Bindings::TouchMenu { ref buttons } =>
						iter(buttons.iter().flat_map(|v| v.iter()))
				})
			.flat_map(|binding|
				binding.iter())
			.filter(|&binding|
				if let &Binding::Action(..) = binding {
					false
				}
				else {
					true
				})
			.fold(builder, |builder, binding|
				builder.event(binding).unwrap());

		Ok(Mapper {
			config: config,
			device: RefCell::new(builder.create()?),
			preset: 0,
		})
	}

	pub fn send(&mut self, event: Event) -> Res<()> {
		match event {
			Event::Button(btn@input::Button::A, press) |
			Event::Button(btn@input::Button::B, press) |
			Event::Button(btn@input::Button::X, press) |
			Event::Button(btn@input::Button::Y, press) => {
				if let Some(bindings) = self.source(config::Input::ButtonDiamond, true, false)
					.and_then(|id| self.bindings(id))
				{
					button_diamond::button(&mut *self.device.borrow_mut(), bindings, btn, press)?;
				}
			}

			Event::Button(btn@input::Button::Up, press) |
			Event::Button(btn@input::Button::Down, press) |
			Event::Button(btn@input::Button::Left, press) |
			Event::Button(btn@input::Button::Right, press) |
			Event::Button(btn@input::Button::Pad, press) => {
				if let Some(bindings) = self.source(config::Input::TrackpadLeft, true, false)
					.and_then(|id| self.bindings(id))
				{
					trackpad_left::button(&mut *self.device.borrow_mut(), bindings, btn, press)?;
				}
			}

			event =>
				println!("{:?}", event)
		}

		self.device.borrow_mut().synchronize()?;

		Ok(())
	}

	fn source(&self, input: config::Input, active: bool, shift: bool) -> Option<u32> {
		self.config.presets.get(&self.preset).unwrap().sources.values()
			.find(|s|
				s.input  == input &&
				s.active == active &&
				s.shift  == shift)
			.map(|s|
				s.id)
	}

	fn bindings(&self, id: u32) -> Option<&group::Bindings> {
		self.config.groups.get(&id).map(|g| &g.bindings)
	}

}
