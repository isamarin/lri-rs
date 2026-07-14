use crate::error::LriError;

const TEN_MASK: u64 = 1023;

pub fn tenbit(packd: &[u8], count: usize, upack: &mut [u16]) -> Result<(), LriError> {
	let required_len = count.saturating_mul(10).div_ceil(8);

	if count > upack.len() {
		return Err(LriError::PixelCountMismatch {
			expected: count,
			got: upack.len(),
		});
	}

	if required_len > packd.len() {
		return Err(LriError::PixelCountMismatch {
			expected: required_len,
			got: packd.len(),
		});
	}

	let full_chunks = required_len / 5;
	let remain = required_len % 5;

	for idx in 0..full_chunks {
		let start = required_len - (idx + 1) * 5;
		let chnk = &packd[start..start + 5];
		let long = u64::from_be_bytes([
			0x00, 0x00, 0x00, chnk[0], chnk[1], chnk[2], chnk[3], chnk[4],
		]);

		let out = idx * 4;
		upack[out] = ((long >> 30) & TEN_MASK) as u16;
		upack[out + 1] = ((long >> 20) & TEN_MASK) as u16;
		upack[out + 2] = ((long >> 10) & TEN_MASK) as u16;
		upack[out + 3] = (long & TEN_MASK) as u16;
	}

	if remain > 0 {
		let mut long_bytes = [0u8; 8];
		long_bytes[..remain].copy_from_slice(&packd[..remain]);
		let long = u64::from_le_bytes(long_bytes);

		let count_remain = count % 4;
		let start = count - count_remain;
		for idx in 0..count_remain {
			upack[start + idx] = ((long >> (10 * idx)) & TEN_MASK) as u16;
		}
	}

	Ok(())
}