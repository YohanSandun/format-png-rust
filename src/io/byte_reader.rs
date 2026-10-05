use crate::error::Error;

pub struct ByteReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> ByteReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub(crate) fn read_u8(&mut self) -> Result<u8, Error> {
        if self.pos >= self.data.len() {
            return Err(Error::UnexpectedEndOfInput);
        }

        let byte = self.data[self.pos];
        self.pos += 1;
        Ok(byte)
    }

    pub(crate) fn read_u32(&mut self) -> Result<u32, Error> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_be_bytes(bytes.try_into().unwrap()))
    }

    pub(crate) fn read_bytes(&mut self, length: usize) -> Result<&'a [u8], Error> {
        let end = self
            .pos
            .checked_add(length)
            .ok_or(Error::UnexpectedEndOfInput)?;

        let data = self
            .data
            .get(self.pos..end)
            .ok_or(Error::UnexpectedEndOfInput)?;

        self.pos = end;
        Ok(data)
    }

    /// Returns `true` once every byte has been read.
    pub(crate) fn is_empty(&self) -> bool {
        self.pos == self.data.len()
    }
}

#[cfg(test)]
mod tests;
