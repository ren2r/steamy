#[cfg(not(target_os = "linux"))]
use std::marker::PhantomData;

use std::time::Duration;
use std::io::{self, Cursor, Write};
use byteorder::{WriteBytesExt};

#[cfg(target_os = "linux")]
use usb;

#[cfg(not(target_os = "linux"))]
use hid;

use {Result as Res, Error, State, Feedback, Sensors, Led, Sound};

/// The controller.
#[cfg(target_os = "linux")]
pub struct Controller<'a> {
	handle: usb::DeviceHandle<'a>,

	product: u16,
	address: u8,
	index:   u16,
}

#[cfg(not(target_os = "linux"))]
pub struct Controller<'a> {
	handle:  hid::Handle,
	product: u16,
	marker:  PhantomData<&'a ()>,
}

impl<'a> Controller<'a> {
	#[doc(hidden)]
	#[cfg(target_os = "linux")]
	pub fn new<'b>(mut device: usb::Device<'b>, mut handle: usb::DeviceHandle<'b>, product: u16, endpoint: u8, index: u16) -> Res<Controller<'b>> {
		let mut address: Option<u8> = None;

		for i in 0 .. try!(device.device_descriptor()).num_configurations() {
			for interface in try!(device.config_descriptor(i)).interfaces() {
				if try!(handle.kernel_driver_active(interface.number())) {
					try!(handle.detach_kernel_driver(interface.number()));
				}

				for descriptor in interface.descriptors() {
					if descriptor.class_code() == 3 &&
					   descriptor.sub_class_code() == 0 &&
					   descriptor.protocol_code() == 0
					{
						try!(handle.claim_interface(descriptor.interface_number()));
					}

					for end in descriptor.endpoint_descriptors() {
						if end.number() == endpoint {
							address = Some(end.address());
						}
					}
				}
			}
		}

		let mut controller = Controller {
			handle:  handle,
			product: product,
			address: try!(address.ok_or(usb::Error::InvalidParam)),
			index:   index,
		};

		try!(controller.reset());

		Ok(controller)
	}

	#[cfg(not(target_os = "linux"))]
	pub fn new<'b>(handle: hid::Handle, product: u16) -> Res<Controller<'b>> {
		let mut controller = Controller {
			handle:  handle,
			product: product,
			marker:  PhantomData,
		};

		try!(controller.reset());

		Ok(controller)
	}

	fn reset(&mut self) -> Res<()> {
		try!(self.sensors().off());
		try!(self.control(|mut buf| {
			buf.write_u8(0x81)
		}));

		Ok(())
	}

	/// Check if the controller is remote.
	pub fn is_remote(&self) -> bool {
		self.product == 0x1142
	}

	/// Check if the controller is wired.
	pub fn is_wired(&self) -> bool {
		self.product == 0x1102
	}

	#[doc(hidden)]
	#[cfg(target_os = "linux")]
	pub fn control<T, F: FnOnce(Cursor<&mut [u8]>) -> io::Result<T>>(&mut self, func: F) -> Res<()> {
		let mut buf = [0u8; 64];

		try!(func(Cursor::new(&mut buf)));
		try!(self.handle.write_control(0x21, 0x09, 0x0300, self.index, &buf, Duration::from_secs(0)));

		Ok(())
	}

	#[cfg(not(target_os = "linux"))]
	pub fn control<T, F: FnOnce(Cursor<&mut [u8]>) -> io::Result<T>>(&mut self, func: F) -> Res<()> {
		let mut buf = [0u8; 65];
		buf[0] = 0x00;

		try!(func(Cursor::new(&mut buf[1..])));
		try!(self.handle.feature().send(&buf[..]));

		Ok(())
	}

	/// Get the led manager.
	pub fn led<'b>(&'b mut self) -> Led<'b, 'a> where 'a: 'b {
		Led::new(self)
	}

	/// Get the feedback builder.
	pub fn feedback<'b>(&'b mut self) -> Feedback<'b, 'a> where 'a: 'b {
		Feedback::new(self)
	}

	/// Get the sensor manager.
	pub fn sensors<'b>(&'b mut self) -> Sensors<'b, 'a> where 'a: 'b {
		Sensors::new(self)
	}

	/// Get the sound player.
	pub fn sound<'b>(&'b mut self) -> Sound<'b, 'a> where 'a: 'b {
		Sound::new(self)
	}

	/// Turn the controller off.
	pub fn off(&mut self) -> Res<()> {
		self.control(|mut buf| {
			buf.write(&[
				0x9f, 0x04, 0x6f, 0x66,
				0x66, 0x21
			][..])
		})
	}

	/// Read the raw state of the controller.
	#[cfg(target_os = "linux")]
	pub fn raw(&mut self, timeout: Duration) -> Res<[u8; 64]> {
		let mut buf = [0u8; 64];

		if try!(self.handle.read_interrupt(self.address, &mut buf, timeout)) != buf.len() {
			return Err(Error::InvalidParameter);
		}

		Ok(buf)
	}

	#[cfg(not(target_os = "linux"))]
	pub fn raw(&mut self, timeout: Duration) -> Res<[u8; 64]> {
		let mut buf = [0u8; 64];

		if try!(self.handle.data().read(&mut buf[..], timeout)).unwrap_or(0) != buf.len() {
			return Err(Error::InvalidParameter);
		}

		Ok(buf)
	}

	/// Get the current state of the controller.
	pub fn state(&mut self, timeout: Duration) -> Res<State> {
		match try!(State::parse(Cursor::new(&try!(self.raw(timeout))[..]))) {
			state@State::Power(true) => {
				try!(self.reset());

				Ok(state)
			}

			state => {
				Ok(state)
			}
		}
	}
}
