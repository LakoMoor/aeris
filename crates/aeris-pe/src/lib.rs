//! Minimal PE32+ (64-bit Windows executable) parser and loader.
//!
//! `Pe::parse` validates an image, `Pe::load_into` lays it out in memory,
//! applies base relocations and binds imports through a resolver supplied by
//! the OS, so the same code serves the kernel and host tests.

#![cfg_attr(not(test), no_std)]
extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum PeError {
    /// Not a PE file, or truncated.
    Format,
    /// A PE for another CPU (only x86-64 is supported).
    Machine(u16),
    /// A 32-bit (PE32) image.
    Not64Bit,
    /// An offset points outside the file or image.
    OutOfBounds,
    /// An import the OS does not provide.
    Unresolved,
    /// The destination buffer is smaller than SizeOfImage.
    BufferTooSmall,
}

#[derive(Debug, Clone)]
pub struct Section {
    pub name: String,
    pub va: u32,
    pub vsize: u32,
    pub raw_off: u32,
    pub raw_size: u32,
    pub flags: u32,
}

impl Section {
    pub fn executable(&self) -> bool {
        self.flags & 0x2000_0000 != 0
    }
    pub fn writable(&self) -> bool {
        self.flags & 0x8000_0000 != 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    pub dll: String,
    /// Function name, or `#ordinal`.
    pub name: String,
    /// RVA of the IAT slot to patch.
    pub iat_rva: u32,
}

pub struct Pe<'a> {
    data: &'a [u8],
    pub image_base: u64,
    pub entry_rva: u32,
    pub size_of_image: u32,
    pub size_of_headers: u32,
    pub subsystem: u16,
    pub dll_characteristics: u16,
    pub sections: Vec<Section>,
    import_dir: (u32, u32),
    reloc_dir: (u32, u32),
}

fn u16_at(d: &[u8], o: usize) -> Result<u16, PeError> {
    d.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or(PeError::OutOfBounds)
}
fn u32_at(d: &[u8], o: usize) -> Result<u32, PeError> {
    d.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or(PeError::OutOfBounds)
}
fn u64_at(d: &[u8], o: usize) -> Result<u64, PeError> {
    d.get(o..o + 8).map(|b| u64::from_le_bytes(b.try_into().unwrap_or([0; 8]))).ok_or(PeError::OutOfBounds)
}

fn cstr(d: &[u8], o: usize) -> Result<String, PeError> {
    let tail = d.get(o..).ok_or(PeError::OutOfBounds)?;
    let end = tail.iter().position(|&b| b == 0).ok_or(PeError::OutOfBounds)?;
    Ok(String::from_utf8_lossy(&tail[..end]).to_string())
}

impl<'a> Pe<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Pe<'a>, PeError> {
        if data.get(0..2) != Some(b"MZ") {
            return Err(PeError::Format);
        }
        let pe = u32_at(data, 0x3C)? as usize;
        if data.get(pe..pe + 4) != Some(b"PE\0\0") {
            return Err(PeError::Format);
        }
        let coff = pe + 4;
        let machine = u16_at(data, coff)?;
        if machine != 0x8664 {
            return Err(PeError::Machine(machine));
        }
        let nsec = u16_at(data, coff + 2)? as usize;
        let opt_size = u16_at(data, coff + 16)? as usize;
        let opt = coff + 20;
        if u16_at(data, opt)? != 0x20B {
            return Err(PeError::Not64Bit);
        }
        let entry_rva = u32_at(data, opt + 16)?;
        let image_base = u64_at(data, opt + 24)?;
        let size_of_image = u32_at(data, opt + 56)?;
        let size_of_headers = u32_at(data, opt + 60)?;
        let subsystem = u16_at(data, opt + 68)?;
        let dll_characteristics = u16_at(data, opt + 70)?;
        let ndirs = u32_at(data, opt + 108)? as usize;
        let dir = |i: usize| -> Result<(u32, u32), PeError> {
            if i >= ndirs {
                return Ok((0, 0));
            }
            Ok((u32_at(data, opt + 112 + i * 8)?, u32_at(data, opt + 116 + i * 8)?))
        };
        let import_dir = dir(1)?;
        let reloc_dir = dir(5)?;
        let mut sections = Vec::new();
        let sh = opt + opt_size;
        for i in 0..nsec {
            let o = sh + i * 40;
            let raw = data.get(o..o + 8).ok_or(PeError::OutOfBounds)?;
            let end = raw.iter().position(|&b| b == 0).unwrap_or(8);
            sections.push(Section {
                name: String::from_utf8_lossy(&raw[..end]).to_string(),
                vsize: u32_at(data, o + 8)?,
                va: u32_at(data, o + 12)?,
                raw_size: u32_at(data, o + 16)?,
                raw_off: u32_at(data, o + 20)?,
                flags: u32_at(data, o + 36)?,
            });
        }
        Ok(Pe { data, image_base, entry_rva, size_of_image, size_of_headers, subsystem, dll_characteristics, sections, import_dir, reloc_dir })
    }

    pub fn is_console(&self) -> bool {
        self.subsystem == 3
    }

    /// Translate an RVA to a file offset (headers map 1:1).
    fn rva_to_off(&self, rva: u32) -> Result<usize, PeError> {
        if rva < self.size_of_headers {
            return Ok(rva as usize);
        }
        for s in &self.sections {
            let span = s.vsize.max(s.raw_size);
            if rva >= s.va && rva < s.va + span {
                return Ok((s.raw_off + (rva - s.va)) as usize);
            }
        }
        Err(PeError::OutOfBounds)
    }

    pub fn imports(&self) -> Result<Vec<Import>, PeError> {
        let mut out = Vec::new();
        if self.import_dir.0 == 0 {
            return Ok(out);
        }
        let mut d = self.rva_to_off(self.import_dir.0)?;
        loop {
            let ilt = u32_at(self.data, d)?;
            let name_rva = u32_at(self.data, d + 12)?;
            let iat = u32_at(self.data, d + 16)?;
            if name_rva == 0 && iat == 0 {
                break;
            }
            let dll = cstr(self.data, self.rva_to_off(name_rva)?)?;
            let lookup = if ilt != 0 { ilt } else { iat };
            let mut i = 0u32;
            loop {
                let thunk = u64_at(self.data, self.rva_to_off(lookup + i * 8)?)?;
                if thunk == 0 {
                    break;
                }
                let name = if thunk >> 63 == 1 {
                    alloc::format!("#{}", thunk & 0xFFFF)
                } else {
                    cstr(self.data, self.rva_to_off(thunk as u32 & 0x7FFF_FFFF)? + 2)?
                };
                out.push(Import { dll: dll.clone(), name, iat_rva: iat + i * 8 });
                i += 1;
            }
            d += 20;
        }
        Ok(out)
    }

    /// Lay the image out in `image` (length >= SizeOfImage), relocate it to
    /// `base` and bind imports with `resolve(dll, function) -> address`.
    /// Returns the absolute entry-point address.
    pub fn load_into(&self, image: &mut [u8], base: u64, mut resolve: impl FnMut(&str, &str) -> Option<u64>) -> Result<u64, PeError> {
        if image.len() < self.size_of_image as usize {
            return Err(PeError::BufferTooSmall);
        }
        image[..self.size_of_image as usize].fill(0);
        let hdr = self.size_of_headers as usize;
        image[..hdr].copy_from_slice(self.data.get(..hdr).ok_or(PeError::OutOfBounds)?);
        for s in &self.sections {
            let n = s.raw_size.min(s.vsize.max(s.raw_size)) as usize;
            let src = self.data.get(s.raw_off as usize..s.raw_off as usize + n).ok_or(PeError::OutOfBounds)?;
            let dst = image.get_mut(s.va as usize..s.va as usize + n).ok_or(PeError::OutOfBounds)?;
            dst.copy_from_slice(src);
        }
        // Base relocations (DIR64 only, which is all x86-64 images use).
        let delta = base.wrapping_sub(self.image_base);
        if delta != 0 && self.reloc_dir.0 != 0 {
            let mut off = self.rva_to_off(self.reloc_dir.0)?;
            let end = off + self.reloc_dir.1 as usize;
            while off + 8 <= end {
                let page = u32_at(self.data, off)?;
                let size = u32_at(self.data, off + 4)? as usize;
                if size < 8 {
                    break;
                }
                for i in 0..(size - 8) / 2 {
                    let e = u16_at(self.data, off + 8 + i * 2)?;
                    if e >> 12 == 10 {
                        let at = (page + (e & 0xFFF) as u32) as usize;
                        let cur = u64_at(image, at)?;
                        image.get_mut(at..at + 8).ok_or(PeError::OutOfBounds)?.copy_from_slice(&cur.wrapping_add(delta).to_le_bytes());
                    }
                }
                off += size;
            }
        }
        for imp in self.imports()? {
            let addr = resolve(&imp.dll, &imp.name).ok_or(PeError::Unresolved)?;
            let at = imp.iat_rva as usize;
            image.get_mut(at..at + 8).ok_or(PeError::OutOfBounds)?.copy_from_slice(&addr.to_le_bytes());
        }
        Ok(base + self.entry_rva as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HELLO: &[u8] = include_bytes!("../tests/hello.exe");

    #[test]
    fn parses_real_windows_exe() {
        let pe = Pe::parse(HELLO).expect("parse");
        assert!(pe.is_console());
        assert!(pe.size_of_image >= 0x1000);
        assert!(pe.sections.iter().any(|s| s.executable()));
        let imps = pe.imports().expect("imports");
        let names: Vec<_> = imps.iter().map(|i| (i.dll.to_lowercase(), i.name.as_str())).collect();
        for f in ["GetStdHandle", "WriteFile", "ExitProcess"] {
            assert!(names.iter().any(|(d, n)| d == "kernel32.dll" && *n == f), "missing {f}: {names:?}");
        }
    }

    #[test]
    fn loads_relocates_and_binds_imports() {
        let pe = Pe::parse(HELLO).unwrap();
        let mut mem = alloc::vec![0u8; pe.size_of_image as usize];
        let base = 0x0000_7FF6_1234_0000u64;
        let mut seen = Vec::new();
        let entry = pe
            .load_into(&mut mem, base, |dll, f| {
                seen.push((dll.to_string(), f.to_string()));
                Some(0xDEAD_0000 + seen.len() as u64)
            })
            .expect("load");
        assert_eq!(entry, base + pe.entry_rva as u64);
        assert_eq!(seen.len(), 3);
        for (i, imp) in pe.imports().unwrap().iter().enumerate() {
            let v = u64::from_le_bytes(mem[imp.iat_rva as usize..imp.iat_rva as usize + 8].try_into().unwrap());
            assert_eq!(v, 0xDEAD_0001 + i as u64);
        }
        // the entry point holds real x86-64 code (not zeros)
        assert!(mem[pe.entry_rva as usize..pe.entry_rva as usize + 8].iter().any(|&b| b != 0));
    }

    #[test]
    fn rejects_garbage_and_missing_imports() {
        assert_eq!(Pe::parse(b"not a pe").err(), Some(PeError::Format));
        let pe = Pe::parse(HELLO).unwrap();
        let mut mem = alloc::vec![0u8; pe.size_of_image as usize];
        assert_eq!(pe.load_into(&mut mem, pe.image_base, |_, _| None).err(), Some(PeError::Unresolved));
        let mut small = alloc::vec![0u8; 16];
        assert_eq!(pe.load_into(&mut small, pe.image_base, |_, _| Some(1)).err(), Some(PeError::BufferTooSmall));
    }
}
