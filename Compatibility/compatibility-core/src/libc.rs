/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Our implementations of various things that Apple's libSystem would provide.
//! On other platforms these are part of the "libc", so let's call it that.
//!
//! Useful resources:
//! - Apple's [iOS Manual Pages](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/) (contains what would be `man` pages if iOS had a command line)

mod generic_char;

pub mod arpa;
pub mod clocale;
pub mod compiler_rt;
pub mod crypto;
pub mod ctype;
pub mod cxxabi;
pub mod dirent;
pub mod dlfcn;
pub mod dns_sd;
pub mod errno;
pub mod fnmatch;
pub mod glob;
pub mod ifaddrs;
pub mod keymgr;
pub mod libkern;
pub mod mach;
pub mod mach_o;
pub mod malloc;
pub mod math;
pub mod net;
pub mod netdb;
pub mod posix_io;
pub mod pthread;
pub mod sched;
pub mod semaphore;
pub mod setjmp;
pub mod signal;
pub mod stdio;
pub mod stdlib;
pub mod string;
pub mod sys;
pub mod sysctl;
pub mod time;
pub mod unistd;
pub mod wchar;
mod wide_time;
pub mod zlib;

pub const DYLIB: crate::dyld::HostDylib = crate::dyld::HostDylib {
    path: "/usr/lib/libSystem.B.dylib",
    aliases: &["/usr/lib/libSystem.dylib"],
    class_exports: &[],
    constant_exports: &[ctype::CONSTANTS, stdio::CONSTANTS, mach::init::CONSTANTS, cxxabi::CONSTANTS],
    function_exports: &[
        arpa::inet::FUNCTIONS,
        clocale::FUNCTIONS,
        compiler_rt::FUNCTIONS,
        ctype::FUNCTIONS,
        cxxabi::FUNCTIONS,
        crypto::FUNCTIONS,
        dirent::FUNCTIONS,
        dlfcn::FUNCTIONS,
        dns_sd::FUNCTIONS,
        errno::FUNCTIONS,
        fnmatch::FUNCTIONS,
        glob::FUNCTIONS,
        ifaddrs::FUNCTIONS,
        keymgr::FUNCTIONS,
        libkern::os_atomic::FUNCTIONS,
        mach::arm::task::FUNCTIONS,
        mach::arm::thread_act::FUNCTIONS,
        libkern::task::FUNCTIONS,
        mach::host::FUNCTIONS,
        mach::init::FUNCTIONS,
        mach::mach_port::FUNCTIONS,
        mach::message::FUNCTIONS,
        mach::semaphore::FUNCTIONS,
        mach::thread_info::FUNCTIONS,
        mach::time::FUNCTIONS,
        mach::vm_map::FUNCTIONS,
        mach_o::FUNCTIONS,
        malloc::FUNCTIONS,
        math::FUNCTIONS,
        net::if_::FUNCTIONS,
        netdb::FUNCTIONS,
        posix_io::FUNCTIONS,
        posix_io::stat::FUNCTIONS,
        posix_io::statvfs::FUNCTIONS,
        pthread::cond::FUNCTIONS,
        pthread::key::FUNCTIONS,
        pthread::mutex::FUNCTIONS,
        pthread::once::FUNCTIONS,
        pthread::thread::FUNCTIONS,
        sched::FUNCTIONS,
        semaphore::FUNCTIONS,
        setjmp::FUNCTIONS,
        signal::FUNCTIONS,
        stdio::FUNCTIONS,
        stdio::printf::FUNCTIONS,
        stdlib::FUNCTIONS,
        stdlib::qsort::FUNCTIONS,
        string::FUNCTIONS,
        sys::mman::FUNCTIONS,
        sys::mount::FUNCTIONS,
        sys::ptrace::FUNCTIONS,
        sys::timeb::FUNCTIONS,
        sys::socket::FUNCTIONS,
        sys::uio::FUNCTIONS,
        sys::utsname::FUNCTIONS,
        sys::wait::FUNCTIONS,
        sysctl::FUNCTIONS,
        time::FUNCTIONS,
        unistd::FUNCTIONS,
        wchar::FUNCTIONS,
        wide_time::FUNCTIONS,
    ],
};

/// Container for state of various child modules
#[derive(Default)]
pub struct State {
    pub(crate) gif_island:crate::peggle_gif_island::State,
    pub(crate) peggle_ui_cadence: crate::peggle_ui_cadence::State,
    pub(crate) peggle_clip_rect: crate::peggle_clip_rect::State,
    pub(crate) peggle_inflate_fast: crate::peggle_inflate_fast::State,
    pub(crate) peggle_native_batch: crate::peggle_native_batch::State,
    zlib: zlib::State,
    pub(crate) hd_file_zlib: crate::hd_file_zlib::State,
    pub(crate) hd_png_zlib: crate::hd_png_zlib::State,
    pub(crate) peggle_matrix: crate::peggle_matrix::State,
    pub(crate) peggle_endpoint_index: crate::peggle_endpoint_index::State,
    pub(crate) peggle_collision_swap: crate::peggle_collision_swap::State,
    pub(crate) peggle_collision_geometry: crate::peggle_collision_geometry::State,
    pub(crate) peggle_vertex_pack: crate::peggle_vertex_pack::State,
    pub(crate) peggle_font_lookup: crate::peggle_font_lookup::State,
    pub(crate) peggle_inflate_loops:crate::peggle_inflate_loops::State,
    pub(crate) peggle_pixel_pack:crate::peggle_pixel_pack::State,
    pub(crate) peggle_premultiply: crate::peggle_premultiply::State,
    pub(crate) peggle_mapped_char: crate::peggle_mapped_char::State,
    pub(crate) peggle_jpeg_idct: crate::peggle_jpeg_idct::State,
    pub(crate) peggle_jpeg_ycc: crate::peggle_jpeg_ycc::State,
    pub(crate) peggle_png_filter: crate::peggle_png_filter::State,
    pub(crate) peggle_endpoint_compare: crate::peggle_endpoint_compare::State,
    cxxabi: cxxabi::State,
    dirent: dirent::State,
    keymgr: keymgr::State,
    math: math::State,
    posix_io: posix_io::State,
    pub pthread: pthread::State,
    pub semaphore: semaphore::State,
    pub socket: sys::socket::State,
    stdlib: stdlib::State,
    string: string::State,
    pub stdio: stdio::State,
    time: time::State,
    errno: errno::State,
    clocale: clocale::State,
    mach_vm: mach::vm_map::State,
    malloc: malloc::State,
    mman: sys::mman::State,
}
