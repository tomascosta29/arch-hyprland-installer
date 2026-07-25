//! Shared helpers for decoding media artwork into GTK paintables.

use gdk_pixbuf::prelude::*;
use gdk_pixbuf::PixbufLoader;
use gtk4::gdk;
use gtk4::gdk_pixbuf;

pub fn texture_from_bytes(data: &[u8], size: i32) -> Option<gdk::Texture> {
    let loader = PixbufLoader::new();
    loader.write(data).ok()?;
    loader.close().ok()?;
    let pixbuf = loader.pixbuf()?;
    let scaled = pixbuf.scale_simple(size, size, gdk_pixbuf::InterpType::Bilinear)?;
    Some(gdk::Texture::for_pixbuf(&scaled))
}
