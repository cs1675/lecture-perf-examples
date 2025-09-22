use serde::*;
use std::io::prelude::*;

#[derive(Serialize, Deserialize, Clone)]
pub enum Message {
	Put((u64, Vec<u8>)),
	Touch(Vec<u64>),
	TouchSeq((u64, usize)),
	Ok {
		work_duration: u64,
		touched: u64,
	},
	Noop,
	File,
	Err,
}

impl Message {
	pub fn write<T: Write>(&self, t: &mut T) -> Result<(), anyhow::Error> {
		let v = self.to_message();
		Ok(t.write_all(&v)?)
	}

	#[allow(unused)]
	pub fn write_with_buf<T: Write>(
		&self,
		t: &mut T,
	) -> Result<(), anyhow::Error> {
		let v = self.to_message();
		Ok(t.write_all(&v)?)
	}

	#[allow(unused)]
	pub fn read<T: Read>(t: &mut T) -> Result<Self, anyhow::Error> {
		let mut v = Vec::new();
		Self::read_with_buf(t, &mut v)?;
		Ok(Self::from_bytes(&mut v)?)
	}

	#[allow(unused)]
	pub fn read_with_buf<T: Read>(
		t: &mut T,
		buf: &mut Vec<u8>,
	) -> Result<Self, anyhow::Error> {
		buf.clear();
		let mut x = [0u8; 8];
		t.read_exact(&mut x)?;
		let sz = usize::from_be_bytes(x);
		buf.resize(sz, 0u8);
		t.read_exact(buf)?;
		Ok(Self::from_bytes(buf)?)
	}

	pub fn work_duration(&self) -> Option<u64> {
		match self {
			Self::Ok { work_duration, .. } => Some(*work_duration),
			_ => None,
		}
	}

	pub fn touched(&self) -> Option<u64> {
		match self {
			Self::Ok { touched, .. } => Some(*touched),
			_ => None,
		}
	}


	fn to_message(&self) -> Vec<u8> {
		let mut v = Vec::new();
		self.to_message_buf(&mut v);
		v
	}

	fn to_message_buf(&self, buf: &mut Vec<u8>) {
		buf.resize(8, 0);
		bincode::serialize_into(&mut *buf, self).unwrap();
		let sz = buf.len() - 8;
		buf[0..8].copy_from_slice(&sz.to_be_bytes());
	}

	//fn to_bytes(&self, buf: &mut [u8]) -> Result<u64, anyhow::Error> {
	//	let sz = bincode::serialized_size(self)?;
	//	bincode::serialize_into(buf, self)?;
	//	Ok(sz)
	//}

	//fn to_vec(&self, buf: &mut Vec<u8>) -> Result<(), anyhow::Error> {
	//	bincode::serialize_into(buf, self)?;
	//	Ok(())
	//}

	fn from_bytes(buf: &[u8]) -> Result<Self, anyhow::Error> {
		let packet: Self = bincode::deserialize_from(buf)?;
		Ok(packet)
	}
}
