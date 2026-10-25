/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Utilities for presenting frames to the window using an abstract OpenGL ES
//! implementation.

use super::gles11_raw as gles11; // constants and types only
use super::GLES;
use crate::matrix::Matrix;
use std::time::{Duration, Instant};

pub struct FpsCounter {
    time: std::time::Instant,
    frames: u32,
}
impl FpsCounter {
    pub fn start() -> Self {
        FpsCounter {
            time: Instant::now(),
            frames: 0,
        }
    }

    pub fn count_frame(&mut self, label: std::fmt::Arguments<'_>) {
        self.frames += 1;
        let now = Instant::now();
        let duration = now - self.time;
        if duration >= Duration::from_secs(1) {
            self.time = now;
            echo!(
                "touchHLE: {} FPS: {:.2}",
                label,
                std::mem::take(&mut self.frames) as f32 / duration.as_secs_f32()
            );
        }
    }
}

/// Present the the latest frame (e.g. the app's splash screen or rendering
/// output), provided as a texture bound to `GL_TEXTURE_2D`, by drawing it on
/// the window. It may be rotated, scaled and/or letterboxed as necessary. The
/// virtual cursor is also drawn if it should be currently visible.
///
/// The provided context must be current.
pub unsafe fn present_frame(
    gles: &mut dyn GLES,
    viewport: (u32, u32, u32, u32),
    rotation_matrix: Matrix<2>,
    virtual_cursor_visible_at: Option<(f32, f32, bool)>,
) {
    present_impl(gles,viewport,rotation_matrix,virtual_cursor_visible_at,false,false);
}
/// An owned shared texture is already complete and clamped. Avoid modifying
/// its sampler object while it is attached in the producer context.
pub unsafe fn present_shared_frame(gles:&mut dyn GLES,viewport:(u32,u32,u32,u32),rotation_matrix:Matrix<2>,cursor:Option<(f32,f32,bool)>,opaque:bool) {
    present_impl(gles,viewport,rotation_matrix,cursor,true,opaque);
}
unsafe fn present_impl(gles:&mut dyn GLES,viewport:(u32,u32,u32,u32),rotation_matrix:Matrix<2>,virtual_cursor_visible_at:Option<(f32,f32,bool)>,shared:bool,opaque:bool) {
    // While this is a generic utility, it is closely tied to
    // crate::frameworks::opengles::eagl::present_renderbuffer, which handles
    // backing up and restoring OpenGL ES state that this function might touch,
    // so these need to be updated in tandem.

    use gles11::types::*;

    // The window draw target is platform-dependent. UIKit has no default
    // framebuffer zero and SDL swaps its own color renderbuffer.
    gles.BindWindowFramebuffer();
    // Original iPhone-sized frames (e.g. 320x480) are non-power-of-two.
    // Native GLES 1 supports these with clamp-to-edge and no mipmaps; leaving
    // the default REPEAT wrap makes the presentation texture incomplete.
    if !shared { for pname in [gles11::TEXTURE_WRAP_S, gles11::TEXTURE_WRAP_T] {
        gles.TexParameteri(gles11::TEXTURE_2D, pname, gles11::CLAMP_TO_EDGE as _);
    }

    }

    // Draw the quad
    gles.Viewport(
        viewport.0 as _,
        viewport.1 as _,
        viewport.2 as _,
        viewport.3 as _,
    );
    gles.ClearColor(0.0, 0.0, 0.0, 1.0);
    gles.Clear(gles11::COLOR_BUFFER_BIT | gles11::DEPTH_BUFFER_BIT | gles11::STENCIL_BUFFER_BIT);
    gles.BindBuffer(gles11::ARRAY_BUFFER, 0);
    let vertices: [f32; 12] = [
        -1.0, -1.0, -1.0, 1.0, 1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0,
    ];
    gles.EnableClientState(gles11::VERTEX_ARRAY);
    gles.VertexPointer(2, gles11::FLOAT, 0, vertices.as_ptr() as *const GLvoid);
    let tex_coords: [f32; 12] = [0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0];
    gles.EnableClientState(gles11::TEXTURE_COORD_ARRAY);
    gles.TexCoordPointer(2, gles11::FLOAT, 0, tex_coords.as_ptr() as *const GLvoid);
    let matrix = presentation_texture_matrix(rotation_matrix);
    gles.MatrixMode(gles11::TEXTURE);
    gles.LoadMatrixf(matrix.columns().as_ptr() as *const _);
    gles.Enable(gles11::TEXTURE_2D);
    let env_state=super::direct_texture::opaque_begin(gles,opaque);
    gles.DrawArrays(gles11::TRIANGLES, 0, 6);
    super::direct_texture::opaque_end(gles,env_state);
    // clean this up so we don't need to worry about it in e.g. Core Animation
    gles.LoadIdentity();

    // Display virtual cursor
    if let Some((x, y, pressed)) = virtual_cursor_visible_at {
        let (vx, vy, vw, vh) = viewport;
        let x = x - vx as f32;
        let y = y - vy as f32;

        gles.DisableClientState(gles11::TEXTURE_COORD_ARRAY);
        gles.Disable(gles11::TEXTURE_2D);

        gles.Enable(gles11::BLEND);
        gles.BlendFunc(gles11::ONE, gles11::ONE_MINUS_SRC_ALPHA);
        gles.Color4f(0.0, 0.0, 0.0, if pressed { 2.0 / 3.0 } else { 1.0 / 3.0 });

        let radius = 10.0;

        let mut vertices = vertices;
        for i in (0..vertices.len()).step_by(2) {
            vertices[i] = (vertices[i] * radius + x) / (vw as f32 / 2.0) - 1.0;
            vertices[i + 1] = 1.0 - (vertices[i + 1] * radius + y) / (vh as f32 / 2.0);
        }
        gles.VertexPointer(2, gles11::FLOAT, 0, vertices.as_ptr() as *const GLvoid);
        gles.DrawArrays(gles11::TRIANGLES, 0, 6);
    }
    if super::gpu_transfer::enabled() {
        let _flush_timing=crate::diagnostics::scope(crate::diagnostics::Phase::HostSharedFlush);
        gles.Flush();
    }
}

fn presentation_texture_matrix(rotation: Matrix<2>) -> Matrix<4> {
    // Texture coordinates occupy [0, 1], so rotate about their center.
    // Rotating about the origin sends landscape coordinates outside the
    // texture, where clamp-to-edge collapses the picture to an edge pixel.
    let mut columns = *Matrix::<4>::from(&rotation).columns();
    let center = rotation.transform([0.5, 0.5]);
    columns[3][0] = 0.5 - center[0];
    columns[3][1] = 0.5 - center[1];
    Matrix::from_columns(columns)
}

#[cfg(test)]
mod tests {
    #[test]
    fn rotated_texture_corners_stay_in_the_unit_square() {
        for angle in [0.0, std::f32::consts::FRAC_PI_2, std::f32::consts::PI,
                      -std::f32::consts::FRAC_PI_2] {
            let matrix = super::presentation_texture_matrix(crate::matrix::Matrix::z_rotation(angle));
            let center = matrix.transform([0.5, 0.5, 0.0, 1.0]);
            assert!((center[0] - 0.5).abs() < 0.00001 && (center[1] - 0.5).abs() < 0.00001);
            for [x, y] in [[0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 1.0]] {
                let corner = matrix.transform([x, y, 0.0, 1.0]);
                assert!((-0.00001..=1.00001).contains(&corner[0]));
                assert!((-0.00001..=1.00001).contains(&corner[1]));
            }
        }
    }
}
