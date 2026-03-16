use core::num::NonZeroU32;

struct Animation {
    /// `z`: Gap in ms to wait before displaying the next frame
    frame_gap_ms: NonZeroU32
}

enum BackgroundCanvas {
    Color {
        /// `Y`: rgba - e.g. 0xff000088 for semi-translucent red
        background_color: u32,
        /// `X`: How to change the background color of the canvas vs the previous frame
        color_change: ColorChange,
    },
    // `r` key
    PreviousFrame {
        // 1-indexed
        frame: NonZeroU32
    },
    // `c` frame - how is this different from the above? I can't figure out any difference from the
    // documentation
    UsePreviousFrame {
        // 1-indexed
        frame: NonZeroU32
    },
}

#[repr(u8)]
enum ColorChange {
    FullAlphaBlend = 0,
    SimpleReplacement = 1,
}

// `s` key
#[repr(u8)]
enum AnimationState {
    // Just stop the animation
    Stop = 1,
    // Run, but stop at end instead of looping
    RunToEnd = 2,
    // Run and keep looping when we reach the end
    RunLooping = 3,
}

// `v` key
enum LoopCount {
    // `v=1`
    LoopForever,
    // `v=({0} - 1)`
    Limited(NonZeroU32)
}
