//! Parser for Delphi binary form streams ("TPF0"), as stored in RCDATA resources.

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Nil,
    Int(i64),
    /// Extended / Single / Double, converted to f64.
    Float(f64),
    Str(String),
    Ident(String),
    Bool(bool),
    Set(Vec<String>),
    List(Vec<Value>),
    Binary(Vec<u8>),
    Collection(Vec<Vec<(String, Value)>>),
}

impl Value {
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) | Value::Ident(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Component {
    pub class: String,
    pub name: String,
    pub props: Vec<(String, Value)>,
    pub children: Vec<Component>,
}

impl Component {
    pub fn prop(&self, name: &str) -> Option<&Value> {
        self.props.iter().find(|(k, _)| k == name).map(|(_, v)| v)
    }
    pub fn int(&self, name: &str) -> Option<i64> {
        self.prop(name).and_then(Value::as_int)
    }
    pub fn str(&self, name: &str) -> Option<&str> {
        self.prop(name).and_then(Value::as_str)
    }
    pub fn bool(&self, name: &str) -> Option<bool> {
        self.prop(name).and_then(Value::as_bool)
    }
    /// Depth-first search by component name.
    pub fn find(&self, name: &str) -> Option<&Component> {
        if self.name.eq_ignore_ascii_case(name) {
            return Some(self);
        }
        self.children.iter().find_map(|c| c.find(name))
    }
    pub fn walk<'a>(&'a self, out: &mut Vec<&'a Component>) {
        out.push(self);
        for c in &self.children {
            c.walk(out);
        }
    }
}

/// Delphi strings in this program are Latin-1 (ANSI codepage 1252 subset).
pub fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| c as char).collect()
}

/// 80-bit x87 extended -> f64.
pub fn extended_to_f64(b: &[u8; 10]) -> f64 {
    let mant = u64::from_le_bytes(b[0..8].try_into().unwrap());
    let se = u16::from_le_bytes([b[8], b[9]]);
    if mant == 0 {
        return 0.0;
    }
    let sign = if se & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exp = (se & 0x7fff) as i32 - 16383;
    sign * (mant as f64 / (1u64 << 63) as f64) * 2f64.powi(exp)
}

struct Reader<'a> {
    b: &'a [u8],
    p: usize,
}

impl<'a> Reader<'a> {
    fn u8(&mut self) -> Result<u8, String> {
        let v = *self.b.get(self.p).ok_or("unexpected end of form stream")?;
        self.p += 1;
        Ok(v)
    }
    fn peek(&self) -> Result<u8, String> {
        self.b.get(self.p).copied().ok_or_else(|| "unexpected end of form stream".into())
    }
    fn raw(&mut self, n: usize) -> Result<&'a [u8], String> {
        let s = self.b.get(self.p..self.p + n).ok_or("unexpected end of form stream")?;
        self.p += n;
        Ok(s)
    }
    fn i32(&mut self) -> Result<i32, String> {
        Ok(i32::from_le_bytes(self.raw(4)?.try_into().unwrap()))
    }
    fn sstr(&mut self) -> Result<String, String> {
        let n = self.u8()? as usize;
        Ok(latin1(self.raw(n)?))
    }

    fn value(&mut self) -> Result<Value, String> {
        let t = self.u8()?;
        Ok(match t {
            0 => Value::Nil,
            1 => {
                let mut items = Vec::new();
                while self.peek()? != 0 {
                    items.push(self.value()?);
                }
                self.p += 1;
                Value::List(items)
            }
            2 => Value::Int(self.u8()? as i8 as i64),
            3 => Value::Int(i16::from_le_bytes(self.raw(2)?.try_into().unwrap()) as i64),
            4 => Value::Int(self.i32()? as i64),
            5 => Value::Float(extended_to_f64(self.raw(10)?.try_into().unwrap())),
            6 => Value::Str(self.sstr()?),
            7 => Value::Ident(self.sstr()?),
            8 => Value::Bool(false),
            9 => Value::Bool(true),
            10 => {
                let n = self.i32()? as usize;
                Value::Binary(self.raw(n)?.to_vec())
            }
            11 => {
                let mut items = Vec::new();
                loop {
                    let s = self.sstr()?;
                    if s.is_empty() {
                        break;
                    }
                    items.push(s);
                }
                Value::Set(items)
            }
            12 => {
                let n = self.i32()? as usize;
                Value::Str(latin1(self.raw(n)?))
            }
            13 => Value::Nil,
            14 => {
                if matches!(self.peek()?, 2 | 3 | 4) {
                    self.value()?;
                }
                let mut items = Vec::new();
                while self.peek()? != 0 {
                    self.p += 1; // vaList marker
                    let mut props = Vec::new();
                    while self.peek()? != 0 {
                        let k = self.sstr()?;
                        props.push((k, self.value()?));
                    }
                    self.p += 1;
                    items.push(props);
                }
                self.p += 1;
                Value::Collection(items)
            }
            15 => Value::Float(f32::from_le_bytes(self.raw(4)?.try_into().unwrap()) as f64),
            16 => Value::Float(i64::from_le_bytes(self.raw(8)?.try_into().unwrap()) as f64 / 10000.0),
            17 => Value::Float(f64::from_le_bytes(self.raw(8)?.try_into().unwrap())),
            18 => {
                let n = self.i32()? as usize;
                let units: Vec<u16> =
                    self.raw(n * 2)?.chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
                Value::Str(String::from_utf16_lossy(&units))
            }
            19 => Value::Int(i64::from_le_bytes(self.raw(8)?.try_into().unwrap())),
            20 => {
                let n = self.i32()? as usize;
                Value::Str(String::from_utf8_lossy(self.raw(n)?).into_owned())
            }
            _ => return Err(format!("unknown DFM value type {t} at {:#x}", self.p - 1)),
        })
    }

    fn component(&mut self) -> Result<Component, String> {
        if self.peek()? & 0xF0 == 0xF0 {
            let flags = self.u8()? & 0x0F;
            if flags & 2 != 0 {
                self.value()?; // child position
            }
        }
        let class = self.sstr()?;
        let name = self.sstr()?;
        let mut props = Vec::new();
        while self.peek()? != 0 {
            let k = self.sstr()?;
            props.push((k, self.value()?));
        }
        self.p += 1;
        let mut children = Vec::new();
        while self.peek()? != 0 {
            children.push(self.component()?);
        }
        self.p += 1;
        Ok(Component { class, name, props, children })
    }
}

pub fn parse(data: &[u8]) -> Result<Component, String> {
    if data.get(0..4) != Some(b"TPF0") {
        return Err("not a TPF0 form stream".into());
    }
    Reader { b: data, p: 4 }.component()
}

/// One item of a `TListView.Items.Data` stream (Delphi 7 TListItems.ReadData format).
#[derive(Debug, Clone)]
pub struct ListItemData {
    pub caption: String,
    pub image_index: i32,
    pub state_index: i32,
    pub overlay_index: i32,
    pub sub_items: Vec<String>,
}

pub fn parse_list_items(data: &[u8]) -> Result<Vec<ListItemData>, String> {
    let mut r = Reader { b: data, p: 0 };
    let _size = r.i32()?;
    let count = r.i32()?;
    let mut items = Vec::new();
    for _ in 0..count {
        let image_index = r.i32()?;
        let state_index = r.i32()?;
        let overlay_index = r.i32()?;
        let sub_count = r.i32()?;
        let _data = r.i32()?;
        let caption = r.sstr()?;
        let mut sub_items = Vec::new();
        for _ in 0..sub_count {
            sub_items.push(r.sstr()?);
        }
        items.push(ListItemData { caption, image_index, state_index, overlay_index, sub_items });
    }
    Ok(items)
}
