use super::*;
use format::R;
#[derive(Clone)]
pub struct E {
    pub id: usize,
    pub kind: String,
    pub ver: usize,
    pub fields: BTreeMap<usize, (Vec<u8>, Vec<u8>)>,
    pub tail: Vec<u8>,
    pub source: Option<Source>,
}
// Untouched scenery can be handed straight back to the native renderer.
// Offsets cover the original and current value of each serialized field.
#[derive(Clone)]
pub struct Source {
    bytes: Vec<u8>,
    fields: Vec<(u16, u32, u32, u32)>,
}
pub struct World {
    pub base: usize,
    pub ents: Vec<E>,
    pub versions: BTreeMap<String, usize>,
}
fn skip(r: &mut R, i: usize) -> Res<()> {
    match i {
        11 | 31 => {
            let len = r.q()?;
            r.take(len)?;
        }
        32 => {
            for _ in 0..r.q()? {
                let len = r.q()?;
                r.take(len)?;
            }
        }
        10 => {
            r.take(8)?;
        }
        19 => {
            r.take(2)?;
        }
        _ => {
            r.take(4)?;
        }
    }
    Ok(())
}
impl World {
    pub fn load(p: &format::Package, name: &str) -> Res<Self> {
        let b = p.get(
            p.by_filename(&format!("{name}.entities"))
                .ok_or("missing level manifest")?,
        )?;
        let mut r = R(&b);
        if r.take(4)? != b"enty" || r.h()? != 7 {
            return Err("unsupported entity manifest".into());
        }
        let base = r.q()?;
        let mut types = vec![];
        for _ in 0..r.q()? {
            types.push((r.long()?, r.q()?));
        }
        let mut ents = vec![];
        for _ in 0..r.w()? {
            let id = r.w()?;
            let (kind, ver) = types.get(r.h()?).ok_or("invalid entity type")?;
            let len = r.w()?;
            let raw = r.take(len)?;
            let mut s = R(raw);
            let mut spans = Vec::new();
            while s.0.len() >= 2 && u16::from_le_bytes(s.0[..2].try_into()?) < 0x1000 {
                let i = s.h()?;
                let a = len - s.0.len();
                skip(&mut s, i)?;
                let v = len - s.0.len();
                skip(&mut s, i)?;
                spans.push((i as u16, a as u32, v as u32, (len - s.0.len()) as u32));
                if s.byte()? != 253 {
                    return Err(format!("bad entity field {i}").into());
                }
            }
            let source = Source {
                bytes: raw[..len - s.0.len()].to_vec(),
                fields: spans,
            };
            let (source, fields) = if source.fields.windows(2).all(|w| w[0].0 < w[1].0) {
                (Some(source), BTreeMap::new())
            } else {
                (None, source.materialize())
            };
            ents.push(E {
                id,
                kind: kind.clone(),
                ver: *ver,
                fields,
                source,
                tail: s.0.to_vec(),
            });
        }
        Ok(Self {
            base,
            ents,
            versions: types.into_iter().collect(),
        })
    }
}
pub fn indices() -> impl Iterator<Item = usize> {
    (0..10).chain(22..30)
}
pub fn strings(v: impl IntoIterator<Item = String>) -> Vec<u8> {
    let v: Vec<_> = v.into_iter().collect();
    let mut b = v.len().to_le_bytes().to_vec();
    for x in v {
        b.extend(string(&x));
    }
    b
}
pub fn string(s: &str) -> Vec<u8> {
    let mut b = s.len().to_le_bytes().to_vec();
    b.extend(s.as_bytes());
    b
}
impl Source {
    fn materialize(self) -> BTreeMap<usize, (Vec<u8>, Vec<u8>)> {
        self.fields
            .into_iter()
            .map(|(i, a, b, c)| {
                (
                    usize::from(i),
                    (
                        self.bytes[a as usize..b as usize].to_vec(),
                        self.bytes[b as usize..c as usize].to_vec(),
                    ),
                )
            })
            .collect()
    }
}
impl E {
    pub fn editable(&mut self) -> &mut BTreeMap<usize, (Vec<u8>, Vec<u8>)> {
        if let Some(source) = self.source.take() {
            self.fields = source.materialize();
        }
        &mut self.fields
    }
    pub fn set(&mut self, i: usize, v: Vec<u8>) {
        self.editable()
            .entry(i)
            .or_insert_with(|| {
                (
                    vec![0; if [10, 11, 31, 32].contains(&i) { 8 } else { 4 }],
                    vec![],
                )
            })
            .1 = v;
    }
    pub fn value(&self, i: usize) -> &[u8] {
        if let Some(s) = &self.source {
            return s
                .fields
                .binary_search_by_key(&(i as u16), |v| v.0)
                .ok()
                .map(|at| {
                    let (_, _, b, c) = s.fields[at];
                    &s.bytes[b as usize..c as usize]
                })
                .unwrap_or(&[]);
        }
        self.fields.get(&i).map(|v| v.1.as_slice()).unwrap_or(&[])
    }
    pub fn float(&self, i: usize) -> f32 {
        self.value(i).try_into().map(f32::from_le_bytes).unwrap_or(
            if matches!(i,3..=5|9|22..=24|26..=28) {
                1.
            } else {
                0.
            },
        )
    }
    pub fn name(&self, i: usize) -> String {
        R(self.value(i)).long().unwrap_or_default()
    }
    pub fn record(&self, b: &mut Vec<u8>, types: &J) -> Res<()> {
        word(b, self.id);
        b.extend(
            types[&self.kind]
                .as_u64()
                .ok_or_else(|| format!("missing reflection for {}", self.kind))?
                .to_le_bytes(),
        );
        b.extend(self.ver.to_le_bytes());
        if let Some(s) = &self.source {
            word(b, s.bytes.len() + self.tail.len());
            b.extend_from_slice(&s.bytes);
        } else {
            let size: usize = self
                .fields
                .values()
                .map(|(a, v)| 3 + a.len() + v.len())
                .sum();
            word(b, size + self.tail.len());
            for (i, (a, v)) in &self.fields {
                b.extend((*i as u16).to_le_bytes());
                b.extend(a);
                b.extend(v);
                b.push(253);
            }
        }
        b.extend_from_slice(&self.tail);
        Ok(())
    }
    pub fn apply(&mut self, s: &J, before: &J) {
        let f = &s["f"];
        for (j, i) in indices().enumerate() {
            let k = super::super::snapshot::FIELDS[j];
            if before.is_null() || f[k] != before[k] {
                self.set(i, (val(&f[k]) as f32).to_le_bytes().into());
            }
        }
        for (i, k) in [(13, "theta_current"), (14, "theta_target")] {
            if !before.is_null() && f[k] != before[k] {
                self.set(i, (val(&f[k]) as f32).to_le_bytes().into());
            }
        }
        if self.kind == "Guy" && !before.is_null() && f["theta_current"] != before["theta_current"]
        {
            let a = val(&s["theta"]).to_radians() * 0.5;
            for (i, x) in [0., 0., a.sin(), a.cos()].into_iter().enumerate() {
                self.set(i + 6, (x as f32).to_le_bytes().into());
            }
        }
        let flags = num(&f["entity_flags"]);
        if flags != u64::from_le_bytes(self.value(10).try_into().unwrap_or([0; 8])) {
            self.set(10, flags.to_le_bytes().into());
        }
        if before.is_null() || f["mesh"] != before["mesh"] || f["materials"] != before["materials"]
        {
            self.set(31, string(f["mesh"].as_str().unwrap_or("")));
            self.set(
                32,
                strings(
                    vecj(&f["materials"])
                        .iter()
                        .filter_map(|v| v.as_str().map(str::to_owned)),
                ),
            );
        }
    }
}
