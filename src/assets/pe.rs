//! Minimal PE32 reader: just enough to pull resources out of the user's own PokeGame.exe.

use std::collections::HashMap;

#[derive(Debug)]
pub struct Section {
    pub name: String,
    pub va: u32,
    pub vsize: u32,
    pub raw_ptr: u32,
    pub raw_size: u32,
}

pub struct Pe {
    pub data: Vec<u8>,
    pub image_base: u32,
    pub sections: Vec<Section>,
}

/// Resource identifier: either a numeric id or a (upper-case) name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ResId {
    Id(u16),
    Name(String),
}

pub const RT_BITMAP: u16 = 2;
pub const RT_RCDATA: u16 = 10;

fn u16_at(d: &[u8], o: usize) -> Result<u16, String> {
    d.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or_else(|| format!("read past end at {o:#x}"))
}
fn u32_at(d: &[u8], o: usize) -> Result<u32, String> {
    d.get(o..o + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or_else(|| format!("read past end at {o:#x}"))
}

impl Pe {
    pub fn parse(data: Vec<u8>) -> Result<Pe, String> {
        if data.get(0..2) != Some(b"MZ") {
            return Err("not an MZ executable".into());
        }
        let pe = u32_at(&data, 0x3c)? as usize;
        if data.get(pe..pe + 4) != Some(b"PE\0\0") {
            return Err("missing PE signature".into());
        }
        let nsec = u16_at(&data, pe + 6)? as usize;
        let opt_size = u16_at(&data, pe + 20)? as usize;
        let opt = pe + 24;
        let image_base = u32_at(&data, opt + 28)?;
        let mut sections = Vec::new();
        for i in 0..nsec {
            let o = opt + opt_size + 40 * i;
            let raw_name = data.get(o..o + 8).ok_or("truncated section table")?;
            let name = String::from_utf8_lossy(raw_name).trim_end_matches('\0').to_string();
            sections.push(Section {
                name,
                vsize: u32_at(&data, o + 8)?,
                va: u32_at(&data, o + 12)?,
                raw_size: u32_at(&data, o + 16)?,
                raw_ptr: u32_at(&data, o + 20)?,
            });
        }
        Ok(Pe { data, image_base, sections })
    }

    /// File offset for a relative virtual address.
    pub fn rva_to_off(&self, rva: u32) -> Option<usize> {
        self.sections
            .iter()
            .find(|s| rva >= s.va && rva < s.va + s.raw_size.max(s.vsize))
            .map(|s| (rva - s.va + s.raw_ptr) as usize)
    }

    /// All resources: (type, name) -> data (first language).
    pub fn resources(&self) -> Result<HashMap<(ResId, ResId), Vec<u8>>, String> {
        let rsrc = self.sections.iter().find(|s| s.name == ".rsrc").ok_or("no .rsrc section")?;
        let base = rsrc.raw_ptr as usize;
        let d = &self.data;
        let read_name = |off: u32| -> Result<ResId, String> {
            if off & 0x8000_0000 != 0 {
                let p = base + (off & 0x7fff_ffff) as usize;
                let len = u16_at(d, p)? as usize;
                let units: Vec<u16> = (0..len).map(|i| u16_at(d, p + 2 + 2 * i)).collect::<Result<_, _>>()?;
                Ok(ResId::Name(String::from_utf16_lossy(&units).to_uppercase()))
            } else {
                Ok(ResId::Id(off as u16))
            }
        };
        let entries = |dir: usize| -> Result<Vec<(u32, u32)>, String> {
            let named = u16_at(d, dir + 12)? as usize;
            let ids = u16_at(d, dir + 14)? as usize;
            (0..named + ids).map(|i| Ok((u32_at(d, dir + 16 + 8 * i)?, u32_at(d, dir + 20 + 8 * i)?))).collect()
        };
        let mut out = HashMap::new();
        for (tname, tdir) in entries(base)? {
            let tid = read_name(tname)?;
            for (ename, edir) in entries(base + (tdir & 0x7fff_ffff) as usize)? {
                let eid = read_name(ename)?;
                let langs = entries(base + (edir & 0x7fff_ffff) as usize)?;
                let Some(&(_, leaf)) = langs.first() else { continue };
                let leaf = base + leaf as usize;
                let rva = u32_at(d, leaf)?;
                let size = u32_at(d, leaf + 4)? as usize;
                let off = self.rva_to_off(rva).ok_or("resource rva outside sections")?;
                let bytes = d.get(off..off + size).ok_or("resource data truncated")?.to_vec();
                out.insert((tid.clone(), eid), bytes);
            }
        }
        Ok(out)
    }
}
