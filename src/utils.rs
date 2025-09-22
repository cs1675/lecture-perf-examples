use std::{
	alloc::{alloc_zeroed, Layout},
	fs::{File, OpenOptions},
	os::unix::prelude::*,
	path::Path,
	ops::{Deref, DerefMut},
};

pub const BLOCK_SZ: usize = 4096;

pub fn new_direct_file(path: &Path, size: usize) -> File {
	//println!("new file: {:?}", path);
	use libc::*;
	let flags = O_LARGEFILE | O_TRUNC | O_DIRECT | O_SYNC | O_RDWR | O_CREAT;
	let file = OpenOptions::new()
		.custom_flags(flags)
		.write(true)
		.mode(S_IRWXU)
		.open(path)
		.unwrap();
	file.set_len(size as u64).unwrap();
	file
}

pub fn manually_boxed_zeroed<T>() -> Box<T> {
	let layout = Layout::new::<T>();
	unsafe {
		let ptr = alloc_zeroed(layout) as *mut T;
		Box::from_raw(ptr)
	}
}

#[repr(C, align(512))]
pub struct AlignedBlock {
	inner: [u8; BLOCK_SZ],
}

impl Deref for AlignedBlock {
	type Target = [u8; BLOCK_SZ];

	fn deref(&self) -> &Self::Target {
		&self.inner
	}
}

impl DerefMut for AlignedBlock {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.inner
	}
}

impl AlignedBlock {
	pub fn new_boxed() -> Box<Self> {
		manually_boxed_zeroed::<AlignedBlock>()
	}
}
