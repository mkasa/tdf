use std::{io::{Cursor, Write}, num::NonZeroU32};


use crossterm::{
	cursor::MoveTo,
	event::EventStream,
	execute,
	terminal::{disable_raw_mode, enable_raw_mode}
};
use image::DynamicImage;
use kittage::{
	AsyncInputReader, ImageDimensions, ImageId, NumberOrId, PixelFormat,
	Verbosity,
	action::Action,
	delete::{ClearOrDelete, DeleteConfig, WhichToDelete},
	display::{CursorMovementPolicy, DisplayConfig, DisplayLocation},
	error::TransmitError,
	image::Image,
	medium::Medium
};
use ratatui::layout::Position;

use crate::converter::MaybeTransferred;

/// Diacritical marks for row/column encoding in Kitty unicode placeholders.
/// From Kitty's gen/rowcolumn-diacritics.txt (256 combining marks, class 230).
#[rustfmt::skip]
static DIACRITICS: [char; 256] = [
	'\u{0305}', '\u{030D}', '\u{030E}', '\u{0310}', '\u{0312}', '\u{033D}', '\u{033E}', '\u{033F}',
	'\u{0346}', '\u{034A}', '\u{034B}', '\u{034C}', '\u{0350}', '\u{0351}', '\u{0352}', '\u{0357}',
	'\u{035B}', '\u{0363}', '\u{0364}', '\u{0365}', '\u{0366}', '\u{0367}', '\u{0368}', '\u{0369}',
	'\u{036A}', '\u{036B}', '\u{036C}', '\u{036D}', '\u{036E}', '\u{036F}', '\u{0483}', '\u{0484}',
	'\u{0485}', '\u{0486}', '\u{0487}', '\u{0592}', '\u{0593}', '\u{0594}', '\u{0595}', '\u{0597}',
	'\u{0598}', '\u{0599}', '\u{059C}', '\u{059D}', '\u{059E}', '\u{059F}', '\u{05A0}', '\u{05A1}',
	'\u{05A8}', '\u{05A9}', '\u{05AB}', '\u{05AC}', '\u{05AF}', '\u{05C4}', '\u{0610}', '\u{0611}',
	'\u{0612}', '\u{0613}', '\u{0614}', '\u{0615}', '\u{0616}', '\u{0617}', '\u{0657}', '\u{0658}',
	'\u{0659}', '\u{065A}', '\u{065B}', '\u{065D}', '\u{065E}', '\u{06D6}', '\u{06D7}', '\u{06D8}',
	'\u{06D9}', '\u{06DA}', '\u{06DB}', '\u{06DC}', '\u{06DF}', '\u{06E0}', '\u{06E1}', '\u{06E2}',
	'\u{06E4}', '\u{06E7}', '\u{06E8}', '\u{06EB}', '\u{06EC}', '\u{0730}', '\u{0732}', '\u{0733}',
	'\u{0735}', '\u{0736}', '\u{073A}', '\u{073D}', '\u{073F}', '\u{0740}', '\u{0741}', '\u{0743}',
	'\u{0745}', '\u{0747}', '\u{0749}', '\u{074A}', '\u{07EB}', '\u{07EC}', '\u{07ED}', '\u{07EE}',
	'\u{07EF}', '\u{07F0}', '\u{07F1}', '\u{07F3}', '\u{0816}', '\u{0817}', '\u{0818}', '\u{0819}',
	'\u{081B}', '\u{081C}', '\u{081D}', '\u{081E}', '\u{081F}', '\u{0820}', '\u{0821}', '\u{0822}',
	'\u{0823}', '\u{0825}', '\u{0826}', '\u{0827}', '\u{0829}', '\u{082A}', '\u{082B}', '\u{082C}',
	'\u{082D}', '\u{0951}', '\u{0953}', '\u{0954}', '\u{0F82}', '\u{0F83}', '\u{0F86}', '\u{0F87}',
	'\u{135D}', '\u{135E}', '\u{135F}', '\u{17DD}', '\u{193A}', '\u{1A17}', '\u{1A75}', '\u{1A76}',
	'\u{1A77}', '\u{1A78}', '\u{1A79}', '\u{1A7A}', '\u{1A7B}', '\u{1A7C}', '\u{1B6B}', '\u{1B6D}',
	'\u{1B6E}', '\u{1B6F}', '\u{1B70}', '\u{1B71}', '\u{1B72}', '\u{1B73}', '\u{1CD0}', '\u{1CD1}',
	'\u{1CD2}', '\u{1CDA}', '\u{1CDB}', '\u{1CE0}', '\u{1DC0}', '\u{1DC1}', '\u{1DC3}', '\u{1DC4}',
	'\u{1DC5}', '\u{1DC6}', '\u{1DC7}', '\u{1DC8}', '\u{1DC9}', '\u{1DCB}', '\u{1DCC}', '\u{1DD1}',
	'\u{1DD2}', '\u{1DD3}', '\u{1DD4}', '\u{1DD5}', '\u{1DD6}', '\u{1DD7}', '\u{1DD8}', '\u{1DD9}',
	'\u{1DDA}', '\u{1DDB}', '\u{1DDC}', '\u{1DDD}', '\u{1DDE}', '\u{1DDF}', '\u{1DE0}', '\u{1DE1}',
	'\u{1DE2}', '\u{1DE3}', '\u{1DE4}', '\u{1DE5}', '\u{1DE6}', '\u{1DFE}', '\u{20D0}', '\u{20D1}',
	'\u{20D4}', '\u{20D5}', '\u{20D6}', '\u{20D7}', '\u{20DB}', '\u{20DC}', '\u{20E1}', '\u{20E7}',
	'\u{20E9}', '\u{20F0}', '\u{2CEF}', '\u{2CF0}', '\u{2CF1}', '\u{2DE0}', '\u{2DE1}', '\u{2DE2}',
	'\u{2DE3}', '\u{2DE4}', '\u{2DE5}', '\u{2DE6}', '\u{2DE7}', '\u{2DE8}', '\u{2DE9}', '\u{2DEA}',
	'\u{2DEB}', '\u{2DEC}', '\u{2DED}', '\u{2DEE}', '\u{2DEF}', '\u{2DF0}', '\u{2DF1}', '\u{2DF2}',
	'\u{2DF3}', '\u{2DF4}', '\u{2DF5}', '\u{2DF6}', '\u{2DF7}', '\u{2DF8}', '\u{2DF9}', '\u{2DFA}',
	'\u{2DFB}', '\u{2DFC}', '\u{2DFD}', '\u{2DFE}', '\u{2DFF}', '\u{A66F}', '\u{A67C}', '\u{A67D}',
	'\u{A6F0}', '\u{A6F1}', '\u{A8E0}', '\u{A8E1}', '\u{A8E2}', '\u{A8E3}', '\u{A8E4}', '\u{A8E5}',
];

pub struct KittyReadyToDisplay<'tui> {
	pub img: KittyImage<'tui>,
	pub page_num: usize,
	pub pos: Position,
	pub display_loc: DisplayLocation,
	pub cell_w: u16,
	pub cell_h: u16,
}

pub enum KittyImage<'tui> {
	Cached(&'tui mut MaybeTransferred),
	Dynamic(DynamicImage)
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

fn write_action_without_response(
	action: &Action<'_, '_>,
	tmux_offset: Option<(u16, u16)>
) -> std::io::Result<()> {
	if tmux_offset.is_some() {
		let writer = DbgWriter {
			w: TmuxPassthroughWriter::new(std::io::stdout().lock()),
			#[cfg(debug_assertions)]
			buf: String::new()
		};
		action.write_transmit_to(writer, Verbosity::Silent).map(|_| ())
	} else {
		let writer = DbgWriter {
			w: std::io::stdout().lock(),
			#[cfg(debug_assertions)]
			buf: String::new()
		};
		action.write_transmit_to(writer, Verbosity::Silent).map(|_| ())
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

/// Write unicode placeholder characters for a Kitty image to stdout (tmux's virtual terminal).
/// The image ID is encoded in the 24-bit foreground color; row/column indices are 1:1 with
/// display cells.
fn write_unicode_placeholders(
	stdout: &mut impl Write,
	image_id: u32,
	pos: Position,
	cell_w: u16,
	cell_h: u16
) -> std::io::Result<()> {
	let r = ((image_id >> 16) & 0xFF) as u8;
	let g = ((image_id >> 8) & 0xFF) as u8;
	let b = (image_id & 0xFF) as u8;

	let mut buf = String::new();
	for row in 0..cell_h {
		buf.push_str(&format!(
			"\x1b[{};{}H",
			pos.y as u32 + row as u32 + 1,
			pos.x as u32 + 1
		));
		buf.push_str(&format!("\x1b[38;2;{r};{g};{b}m"));
		let row_diac = DIACRITICS[row as usize % 256];
		for col in 0..cell_w {
			let col_diac = DIACRITICS[col as usize % 256];
			buf.push('\u{10EEEE}');
			buf.push(row_diac);
			buf.push(col_diac);
		}
	}
	buf.push_str("\x1b[0m");
	stdout.write_all(buf.as_bytes())?;
	stdout.flush()
}

fn tmux_display_config(display_loc: DisplayLocation, cell_w: u16, cell_h: u16) -> DisplayConfig {
	let mut config = DisplayConfig {
		location: display_loc,
		cursor_movement: CursorMovementPolicy::DontMove,
		create_virtual_placement: true,
		..DisplayConfig::default()
	};

	if config.location.columns == 0 {
		config.location.columns = cell_w;
	}
	if config.location.rows == 0 {
		config.location.rows = cell_h;
	}

	config
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
			if tmux_offset.is_none() {
				run_action(
					Action::Delete(DeleteConfig {
						effect: ClearOrDelete::Clear,
						which: WhichToDelete::All
					}),
					ev_stream,
					None
				)
				.await
				.map_err(|e| (vec![], "Couldn't clear previous images", e))?;
			}

			let KittyDisplay::DisplayImages(images) = display else {
				return Ok(());
			};

			images
		}
	};

	if tmux_offset.is_some() {
		let mut err = None;
		for KittyReadyToDisplay {
			img,
			page_num,
			pos,
			display_loc,
			cell_w,
			cell_h,
		} in images
		{
			let config = tmux_display_config(display_loc, cell_w, cell_h);
			let image_id = match img {
				KittyImage::Cached(image_state) => match image_state {
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

						let pid = match fake_image.num_or_id {
							NumberOrId::Id(id) => id,
							NumberOrId::Number(n) => n
						};

						let res = run_action(
							Action::TransmitAndDisplay {
								image: fake_image,
								config,
								placement_id: Some(pid)
							},
							ev_stream,
							tmux_offset
						)
						.await;

						match res {
							Ok(img_id) => {
								*image_state = MaybeTransferred::Transferred(img_id);
								img_id
							}
							Err(e) => {
								let e = err.get_or_insert_with(|| (vec![], e));
								e.0.push(page_num);
								continue;
							}
						}
					}
					MaybeTransferred::Transferred(image_id) => {
						let res = run_action(
							Action::Display {
								image_id: *image_id,
								placement_id: *image_id,
								config
							},
							ev_stream,
							tmux_offset
						)
						.await;

						match res {
							Ok(_) => *image_id,
							Err(e) => {
								let e = err.get_or_insert_with(|| (vec![], e));
								e.0.push(page_num);
								continue;
							}
						}
					}
				},
				KittyImage::Dynamic(dynamic) => {
					let mut png = Vec::new();
					if let Err(e) = dynamic.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
					{
						let e = err.get_or_insert_with(|| {
							(
								vec![],
								TransmitError::Writing(std::io::Error::other(format!(
									"Couldn't encode zoom image to PNG: {e}"
								)))
							)
						});
						e.0.push(page_num);
						continue;
					}
					let image = Image {
						num_or_id: NumberOrId::Id(
							crate::image_id_base().saturating_add(page_num as u32)
						),
						format: PixelFormat::Png(None),
						medium: Medium::Direct {
							chunk_size: None,
							data: png.into()
						}
					};
					let pid = match image.num_or_id {
						NumberOrId::Id(id) => id,
						NumberOrId::Number(n) => n
					};
					let action = Action::TransmitAndDisplay {
						image,
						config,
						placement_id: Some(pid)
					};
					if let Err(e) = write_action_without_response(&action, tmux_offset) {
						let e = err.get_or_insert_with(|| (vec![], TransmitError::Writing(e)));
						e.0.push(page_num);
						continue;
					}
					pid
				}
			};

			write_unicode_placeholders(
				&mut std::io::stdout().lock(),
				image_id.get(),
				pos,
				cell_w,
				cell_h
			)
			.unwrap();
		}

		return match err {
			Some((replace, e)) => Err((replace, "Couldn't transfer image to the terminal", e)),
			None => Ok(())
		};
	}

	let mut err = None;
	for KittyReadyToDisplay {
		img,
		page_num,
		pos,
		display_loc,
		..
	} in images
	{
		let config = DisplayConfig {
			location: display_loc,
			cursor_movement: CursorMovementPolicy::DontMove,
			..DisplayConfig::default()
		};

		let this_err = match img {
			KittyImage::Cached(image_state) => match image_state {
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
							*image_state = MaybeTransferred::Transferred(img_id);
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
			},
			KittyImage::Dynamic(_) => unreachable!("dynamic kitty images are only used under tmux")
		};

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
