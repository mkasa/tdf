mod utils;
use std::sync::{Arc, Mutex};

#[tokio::main]
async fn main() {
	#[cfg(feature = "tracing")]
	console_subscriber::init();

	let file = std::env::args()
		.nth(1)
		.expect("Please enter a file to profile");

	utils::render_doc(file, Arc::new(Mutex::new(1.0)), Arc::new(Mutex::new((0.0, 0.0))), true).await;
}
