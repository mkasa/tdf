use std::{io::Write, num::NonZeroU32};

use crossterm::{
	cursor::MoveTo,
	event::EventStream,
	execute,
	terminal::{disable_raw_mode, enable_raw_mode}
};
use image::DynamicImage;
use kittage::{
	AsyncInputReader, ImageDimensions, ImageId, NumberOrId, PixelFormat,
	action::Action,
	delete::{ClearOrDelete, DeleteConfig, WhichToDelete},
	display::{CursorMovementPolicy, DisplayConfig, DisplayLocation},
	error::TransmitError,
	image::Image,
	medium::Medium
};
use ratatui::layout::Position;

use crate::converter::MaybeTransferred;

pub struct KittyReadyToDisplay<'tui> {
	pub img: &'tui mut MaybeTransferred,
	pub page_num: usize,
	pub pos: Position,
	pub display_loc: DisplayLocation
}

pub enum KittyDisplay<'tui> {
	NoChange,
	ClearImages,
	DisplayImages(Vec<KittyReadyToDisplay<'tui>>)
}

/// Wraps a writer to send output through tmux DCS passthrough sequences.
/// Each write-to-flush cycle is wrapped in `\x1bPtmux;...\x1b\\` with ESC bytes doubled.
struct TmuxPassthroughWriter<W: Write> {
	w: W,
	active: bool
}

impl<W: Write> TmuxPassthroughWriter<W> {
	fn new(w: W) -> Self {
		Self { w, active: false }
	}
}

impl<W: Write> Write for TmuxPassthroughWriter<W> {
	fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
		if !self.active {
			self.w.write_all(b"\x1bPtmux;")?;
			self.active = true;
		}
		for &byte in buf {
			if byte == 0x1b {
				self.w.write_all(&[0x1b, 0x1b])?;
			} else {
				self.w.write_all(&[byte])?;
			}
		}
		Ok(buf.len())
	}

	fn flush(&mut self) -> std::io::Result<()> {
		if self.active {
			self.w.write_all(b"\x1b\\")?;
			self.active = false;
		}
		self.w.flush()
	}
}

pub struct DbgWriter<W: Write> {
	w: W,
	#[cfg(debug_assertions)]
	buf: String
}

impl<W: Write> Write for DbgWriter<W> {
	fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
		#[cfg(debug_assertions)]
		{
			if let Ok(s) = std::str::from_utf8(buf) {
				self.buf.push_str(s);
			}
		}
		self.w.write(buf)
	}

	fn flush(&mut self) -> std::io::Result<()> {
		#[cfg(debug_assertions)]
		{
			log::debug!("Writing to kitty: {:?}", self.buf);
			self.buf.clear();
		}
		self.w.flush()
	}
}

pub async fn run_action<'es>(
	action: Action<'_, '_>,
	ev_stream: &'es mut EventStream,
	tmux_offset: Option<(u16, u16)>
) -> Result<ImageId, TransmitError<<&'es mut EventStream as AsyncInputReader>::Error>> {
	if tmux_offset.is_some() {
		let writer = DbgWriter {
			w: TmuxPassthroughWriter::new(std::io::stdout().lock()),
			#[cfg(debug_assertions)]
			buf: String::new()
		};
		action
			.execute_async(writer, ev_stream)
			.await
			.map(|(_, i)| i)
	} else {
		let writer = DbgWriter {
			w: std::io::stdout().lock(),
			#[cfg(debug_assertions)]
			buf: String::new()
		};
		action
			.execute_async(writer, ev_stream)
			.await
			.map(|(_, i)| i)
	}
}

pub async fn do_shms_work(ev_stream: &mut EventStream, tmux_offset: Option<(u16, u16)>) -> bool {
	let img = DynamicImage::new_rgb8(1, 1);
	let pid = std::process::id();
	let shm_name = format!("tdf_test_{pid}");

	#[cfg(unix)]
	let shm_name = &*shm_name;

	let Ok(mut k_img) = kittage::image::Image::shm_from(img, shm_name) else {
		return false;
	};

	// apparently the terminal won't respond to queries unless they have an Id instead of a number
	k_img.num_or_id = NumberOrId::Id(NonZeroU32::new(u32::MAX).unwrap());

	enable_raw_mode().unwrap();

	let res = run_action(Action::Query(&k_img), ev_stream, tmux_offset).await;

	disable_raw_mode().unwrap();

	res.is_ok()
}

/// Like `run_action`, but first positions the cursor. When in tmux, the MoveTo and the Kitty
/// command share a single DCS passthrough sequence so tmux can't reset the cursor in between.
async fn run_action_at<'es>(
	action: Action<'_, '_>,
	ev_stream: &'es mut EventStream,
	tmux_offset: Option<(u16, u16)>,
	pos: Position
) -> Result<ImageId, TransmitError<<&'es mut EventStream as AsyncInputReader>::Error>> {
	if let Some((col_off, row_off)) = tmux_offset {
		let mut writer = DbgWriter {
			w: TmuxPassthroughWriter::new(std::io::stdout().lock()),
			#[cfg(debug_assertions)]
			buf: String::new()
		};
		// Write the cursor position into the same DCS passthrough as the Kitty command
		write!(
			writer,
			"\x1b[{};{}H",
			pos.y as u32 + row_off as u32 + 1,
			pos.x as u32 + col_off as u32 + 1
		)
		.unwrap();
		action
			.execute_async(writer, ev_stream)
			.await
			.map(|(_, i)| i)
	} else {
		execute!(std::io::stdout(), MoveTo(pos.x, pos.y)).unwrap();
		run_action(action, ev_stream, None).await
	}
}

pub async fn display_kitty_images<'es>(
	display: KittyDisplay<'_>,
	ev_stream: &'es mut EventStream,
	tmux_offset: Option<(u16, u16)>
) -> Result<
	(),
	(
		Vec<usize>,
		&'static str,
		TransmitError<<&'es mut EventStream as AsyncInputReader>::Error>
	)
> {
	let images = match display {
		KittyDisplay::NoChange => return Ok(()),
		KittyDisplay::DisplayImages(_) | KittyDisplay::ClearImages => {
			run_action(
				Action::Delete(DeleteConfig {
					effect: ClearOrDelete::Clear,
					which: WhichToDelete::All
				}),
				ev_stream,
				tmux_offset
			)
			.await
			.map_err(|e| (vec![], "Couldn't clear previous images", e))?;

			let KittyDisplay::DisplayImages(images) = display else {
				return Ok(());
			};

			images
		}
	};

	let mut err = None;
	for KittyReadyToDisplay {
		img,
		page_num,
		pos,
		display_loc
	} in images
	{
		let config = DisplayConfig {
			location: display_loc,
			cursor_movement: CursorMovementPolicy::DontMove,
			..DisplayConfig::default()
		};

		log::debug!("going to display img {img:#?}");
		log::debug!("displaying with config {config:#?}");

		let this_err = match img {
			MaybeTransferred::NotYet(image) => {
				let mut fake_image = Image {
					num_or_id: image.num_or_id,
					format: PixelFormat::Rgb24(
						ImageDimensions {
							width: 0,
							height: 0
						},
						None
					),
					medium: Medium::Direct {
						chunk_size: None,
						data: (&[]).into()
					}
				};
				std::mem::swap(image, &mut fake_image);

				let res = run_action_at(
					Action::TransmitAndDisplay {
						image: fake_image,
						config,
						placement_id: None
					},
					ev_stream,
					tmux_offset,
					pos
				)
				.await;

				match res {
					Ok(img_id) => {
						*img = MaybeTransferred::Transferred(img_id);
						Ok(())
					}
					Err(e) => Err((page_num, e))
				}
			}
			MaybeTransferred::Transferred(image_id) => run_action_at(
				Action::Display {
					image_id: *image_id,
					placement_id: *image_id,
					config
				},
				ev_stream,
				tmux_offset,
				pos
			)
			.await
			.map(|_| ())
			.map_err(|e| (page_num, e))
		};

		log::debug!("this_err is {this_err:#?}");

		if let Err((id, e)) = this_err {
			let e = err.get_or_insert_with(|| (vec![], e));
			e.0.push(id);
		}
	}

	match err {
		Some((replace, e)) => Err((replace, "Couldn't transfer image to the terminal", e)),
		None => Ok(())
	}
}
