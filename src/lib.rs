use std::num::{NonZeroU32, NonZeroUsize};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// Returns a per-process random base for Kitty image IDs, so that multiple tdf
/// instances in the same tmux pane don't collide.  The base is derived from
/// PID + timestamp and lives in the lower 24 bits (the ID is encoded as an RGB
/// foreground color for unicode placeholders).  We reserve the top ~1000 values
/// for page offsets, which is more than enough.
pub fn image_id_base() -> NonZeroU32 {
	static BASE: OnceLock<NonZeroU32> = OnceLock::new();
	*BASE.get_or_init(|| {
		let pid = std::process::id() as u64;
		let nanos = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.unwrap_or_default()
			.as_nanos() as u64;
		// Mix pid and time, mask to 24-bit range minus some headroom for pages
		let mixed = pid.wrapping_mul(6364136223846793005).wrapping_add(nanos);
		let base = (mixed & 0x00FF_FFFF) as u32;
		// Clamp to leave room for page offsets; ensure nonzero
		let base = (base % 0x00FF_F000).max(1);
		// SAFETY: max(1) guarantees nonzero
		NonZeroU32::new(base).unwrap()
	})
}

#[derive(PartialEq)]
pub enum PrerenderLimit {
	All,
	Limited(NonZeroUsize)
}

pub mod converter;
pub mod kitty;
pub mod renderer;
pub mod skip;
pub mod tui;

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum FitOrFill {
	Fit,
	Fill
}

pub struct ScaledResult {
	width: f32,
	height: f32,
	scale_factor: f32
}

#[must_use]
pub fn scale_img_for_area(
	(img_width, img_height): (f32, f32),
	(area_width, area_height): (f32, f32),
	fit_or_fill: FitOrFill
) -> ScaledResult {
	// and get its aspect ratio
	let img_aspect_ratio = img_width / img_height;

	// Then we get the full pixel dimensions of the area provided to us, and the aspect ratio
	// of that area
	let area_aspect_ratio = area_width / area_height;

	// and get the ratio that this page would have to be scaled by to fit perfectly within the
	// area provided to us.
	// we do this first by comparing the aspect ratio of the page with the aspect ratio of the
	// area to fit it within. If the aspect ratio of the page is larger, then we need to scale
	// the width of the page to fill perfectly within the height of the area. Otherwise, we
	// scale the height to fit perfectly. The dimension that _is not_ scaled to fit perfectly
	// is scaled by the same factor as the dimension that _is_ scaled perfectly.
	let scale_factor = match (img_aspect_ratio > area_aspect_ratio, fit_or_fill) {
		(true, FitOrFill::Fit) | (false, FitOrFill::Fill) => area_width / img_width,
		(false, FitOrFill::Fit) | (true, FitOrFill::Fill) => area_height / img_height
	};

	ScaledResult {
		width: img_width * scale_factor,
		height: img_height * scale_factor,
		scale_factor
	}
}
