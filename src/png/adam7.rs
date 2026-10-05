/// One of the seven Adam7 passes: the pixels at `x_start + n * x_step`,
/// `y_start + m * y_step` of the full image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Pass {
    pub(crate) x_start: u32,
    pub(crate) y_start: u32,
    pub(crate) x_step: u32,
    pub(crate) y_step: u32,
}

/// The passes in the order their data is stored.
pub(crate) const PASSES: [Pass; 7] = [
    Pass {
        x_start: 0,
        y_start: 0,
        x_step: 8,
        y_step: 8,
    },
    Pass {
        x_start: 4,
        y_start: 0,
        x_step: 8,
        y_step: 8,
    },
    Pass {
        x_start: 0,
        y_start: 4,
        x_step: 4,
        y_step: 8,
    },
    Pass {
        x_start: 2,
        y_start: 0,
        x_step: 4,
        y_step: 4,
    },
    Pass {
        x_start: 0,
        y_start: 2,
        x_step: 2,
        y_step: 4,
    },
    Pass {
        x_start: 1,
        y_start: 0,
        x_step: 2,
        y_step: 2,
    },
    Pass {
        x_start: 0,
        y_start: 1,
        x_step: 1,
        y_step: 2,
    },
];

impl Pass {
    /// Width and height in pixels of this pass for a `width` x `height` image.
    /// Either can be 0 for small images; the pass is then empty and has no
    /// scanlines, not even filter type bytes.
    pub(crate) fn size(&self, width: u32, height: u32) -> (u32, u32) {
        (
            Self::count(width, self.x_start, self.x_step),
            Self::count(height, self.y_start, self.y_step),
        )
    }

    /// How many of `start`, `start + step`, `start + 2 * step`, ... are below `size`.
    fn count(size: u32, start: u32, step: u32) -> u32 {
        if size <= start {
            return 0;
        }
        // `size - start` is at most 2^31 - 1, so this can't overflow.
        (size - start).div_ceil(step)
    }
}

#[cfg(test)]
mod tests;
