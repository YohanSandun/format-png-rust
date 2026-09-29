#![cfg(test)]

use super::{PASSES, Pass};

fn sizes(width: u32, height: u32) -> Vec<(u32, u32)> {
    PASSES.iter().map(|pass| pass.size(width, height)).collect()
}

// ---------- PASSES ----------

#[test]
fn passes_cover_every_pixel_of_an_8x8_block_once() {
    let mut hits = [[0; 8]; 8];
    for pass in PASSES {
        for y in (pass.y_start..8).step_by(pass.y_step as usize) {
            for x in (pass.x_start..8).step_by(pass.x_step as usize) {
                hits[y as usize][x as usize] += 1;
            }
        }
    }

    assert_eq!(hits, [[1; 8]; 8]);
}

#[test]
fn first_pass_is_every_eighth_pixel() {
    assert_eq!(PASSES[0], Pass { x_start: 0, y_start: 0, x_step: 8, y_step: 8 });
}

// ---------- size ----------

#[test]
fn size_of_8x8_image() {
    assert_eq!(sizes(8, 8), [(1, 1), (1, 1), (2, 1), (2, 2), (4, 2), (4, 4), (8, 4)]);
}

#[test]
fn size_of_13x7_image() {
    assert_eq!(sizes(13, 7), [(2, 1), (2, 1), (4, 1), (3, 2), (7, 2), (6, 4), (13, 3)]);
}

#[test]
fn size_of_1x1_image_leaves_most_passes_empty() {
    assert_eq!(sizes(1, 1), [(1, 1), (0, 1), (1, 0), (0, 1), (1, 0), (0, 1), (1, 0)]);
}

#[test]
fn size_of_3x2_image() {
    assert_eq!(sizes(3, 2), [(1, 1), (0, 1), (1, 0), (1, 1), (2, 0), (1, 1), (3, 1)]);
}

#[test]
fn pass_pixel_counts_add_up_to_image_size() {
    for (width, height) in [(1, 1), (2, 3), (7, 7), (9, 17), (13, 7), (100, 1), (1, 100)] {
        let total: u32 = sizes(width, height).iter().map(|(w, h)| w * h).sum();

        assert_eq!(total, width * height, "{width}x{height}");
    }
}

#[test]
fn size_at_max_dimensions_does_not_overflow() {
    let max = (1 << 31) - 1;

    assert_eq!(PASSES[0].size(max, max), (268_435_456, 268_435_456));
    assert_eq!(PASSES[6].size(max, max), (max, 1_073_741_823));
}
