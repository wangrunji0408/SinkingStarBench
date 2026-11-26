// Decode the shipping "enty" container, using the published r68307 field descriptors.
use super::init::json;
use serde_json::{Map, Value as J};

struct Reader<'a>(&'a [u8]);
impl<'a> Reader<'a> {
    fn bytes(&mut self, n: usize) -> &'a [u8] {
        let (v, tail) = self.0.split_at(n);
        self.0 = tail;
        v
    }
    fn uint(&mut self, n: usize) -> u64 {
        let mut b = [0u8; 8];
        b[..n].copy_from_slice(self.bytes(n));
        u64::from_le_bytes(b)
    }
    fn string(&mut self) -> &'a str {
        let n = self.uint(8) as usize;
        std::str::from_utf8(self.bytes(n)).unwrap()
    }
}
fn ptr(j: &J) -> String {
    u64::from_str_radix(j.as_str().unwrap().trim_start_matches("0x"), 16)
        .unwrap()
        .to_string()
}
pub struct Decoder {
    schema: J,
    types: J,
}
impl Decoder {
    pub fn load() -> Self {
        Self {
            schema: json("meta/fields"),
            types: json("meta/types"),
        }
    }

    fn value(&self, r: &mut Reader, d: &J) -> J {
        let size = d["size"].as_u64().unwrap() as usize;
        match d["tag"].as_u64().unwrap() {
            0 => {
                let n = r.uint(size);
                if d["signed"] == true {
                    let shift = 64 - size * 8;
                    J::from(((n << shift) as i64) >> shift)
                } else {
                    J::from(n)
                }
            }
            1 => J::from(match size {
                4 => f32::from_bits(r.uint(4) as u32) as f64,
                8 => f64::from_bits(r.uint(8)),
                _ => panic!("bad float width"),
            }),
            2 => {
                let v = r.uint(size);
                assert!(v <= 1);
                J::from(v == 1)
            }
            3 | 4 => J::from(r.string()),
            11 | 18 => self.value(r, &self.types[d["underlying_addr"].to_string()]),
            8 => {
                let n = if d["arrayKind"] == 0 {
                    d["count"].as_u64().unwrap()
                } else {
                    r.uint(8)
                };
                assert!(n <= 1_000_000);
                let el = &self.types[d["element_addr"].to_string()];
                J::Array((0..n).map(|_| self.value(r, el)).collect())
            }
            7 => {
                let mut m = Map::new();
                if d["name"] != "Allocator" {
                    for field in d["members"].as_array().unwrap() {
                        m.insert(
                            field["name"].as_str().unwrap().into(),
                            self.value(r, &self.types[ptr(&field["typeaddr"])]),
                        );
                    }
                }
                J::Object(m)
            }
            t => panic!("unsupported serialized field type {t}"),
        }
    }
    fn read(&self, key: &str, keep: impl Fn(&str) -> bool) -> Vec<J> {
        let bytes = crate::assets::read(&format!("scene/{key}"));
        let mut r = Reader(&bytes);
        assert_eq!(r.bytes(4), b"enty");
        r.bytes(10);
        let mut types = vec![];
        for _ in 0..r.uint(8) {
            types.push(r.string());
            r.bytes(8);
        }
        let mut ents = vec![];
        for _ in 0..r.uint(4) {
            let id = r.uint(4);
            let k = types[r.uint(2) as usize];
            let n = r.uint(4) as usize;
            let mut payload = Reader(r.bytes(n));
            if !keep(k) {
                continue;
            }
            let mut m = Map::new();
            let fields = &self.schema[k]["fields"];
            while !payload.0.is_empty() {
                let i = payload.uint(2);
                let f = &fields[i.to_string()];
                let d = &self.types[ptr(&f["type"]["addr"])];
                self.value(&mut payload, d);
                let v = self.value(&mut payload, d);
                assert_eq!(payload.uint(1), 253, "{k} field {i}");
                m.insert(f["name"].as_str().unwrap().into(), v);
            }
            ents.push(serde_json::json!([id, k, m]));
        }
        assert!(r.0.is_empty());
        ents
    }
}
pub fn read(key: &str) -> (Vec<J>, J, Decoder, u8) {
    let d = Decoder::load();
    let mut ents = d.read(key, |k| {
        matches!(k, "Camera_Control" | "Camera_Marker") || crate::scene_filter::gameplay(k)
    });
    let view = screen_view(&ents);
    // The original layer setup uses overview markers to choose where rays
    // wrap onto the fast-travel grid. Other camera markers are visual only.
    ents.retain(|e| {
        e[1] != "Camera_Control" && (e[1] != "Camera_Marker" || e[2]["overhead_map_type"] == 1)
    });
    let fx = d
        .read("ingame_particles", |k| k == "Global_Effects_Settings")
        .into_iter()
        .next()
        .unwrap();
    (ents, fx, d, view)
}

// Constructor fields are read in serialization order before authored overrides.
// Keep that order even for empty strings: their guest allocations are observable.
impl Decoder {
    pub unsafe fn defaults(&self, k: &str, p: usize) -> Map<String, J> {
        self.schema[k]["fields"]
            .as_object()
            .unwrap()
            .values()
            .map(|f| {
                let off = f["offset"].as_u64().unwrap() as usize;
                (
                    f["name"].as_str().unwrap().to_owned(),
                    self.native(p + off, &self.types[ptr(&f["type"]["addr"])]),
                )
            })
            .collect()
    }
    unsafe fn native(&self, p: usize, d: &J) -> J {
        use super::get;
        let n = d["size"].as_u64().unwrap() as usize;
        match d["tag"].as_u64().unwrap() {
            0 => {
                let mut bytes = [0u8; 8];
                bytes[..n].copy_from_slice(std::slice::from_raw_parts(p as *const u8, n));
                let bits = u64::from_le_bytes(bytes);
                if d["signed"] == true {
                    let shift = 64 - 8 * n;
                    J::from(((bits << shift) as i64) >> shift)
                } else {
                    J::from(bits)
                }
            }
            1 => {
                if n == 4 {
                    J::from(get::<f32>(p) as f64)
                } else {
                    J::from(get::<f64>(p))
                }
            }
            2 => J::from(get::<u8>(p) != 0),
            3 => {
                let n = get::<usize>(p);
                if n == 0 {
                    return J::from("");
                }
                assert!(n <= 1 << 20);
                J::from(
                    std::str::from_utf8(std::slice::from_raw_parts(
                        get::<usize>(p + 8) as *const u8,
                        n,
                    ))
                    .unwrap(),
                )
            }
            4 => J::from(""),
            11 | 18 => self.native(p, &self.types[d["underlying_addr"].to_string()]),
            8 => {
                let (n, a) = if d["arrayKind"] == 0 {
                    (d["count"].as_u64().unwrap() as usize, p)
                } else {
                    (get::<usize>(p), get::<usize>(p + 8))
                };
                assert!(n <= 1_000_000);
                let el = &self.types[d["element_addr"].to_string()];
                let stride = el["size"].as_u64().unwrap() as usize;
                J::Array((0..n).map(|i| self.native(a + i * stride, el)).collect())
            }
            7 => {
                let mut obj = Map::new();
                if d["name"] != "Allocator" {
                    for m in d["members"].as_array().unwrap() {
                        obj.insert(
                            m["name"].as_str().unwrap().into(),
                            self.native(
                                p + m["offset"].as_u64().unwrap() as usize,
                                &self.types[ptr(&m["typeaddr"])],
                            ),
                        );
                    }
                }
                J::Object(obj)
            }
            t => panic!("unsupported constructor field {t}"),
        }
    }
}

fn screen_view(entities: &[J]) -> u8 {
    if entities
        .iter()
        .any(|e| e[1] == "Settings" && e[2]["per_screen_camera"] == true)
    {
        return 0;
    }
    for e in entities {
        let f = &e[2];
        if e[1] != "Camera_Control"
            || f["is_for_map_overview"] == true
            || f["minus_one_if_not_primary"].as_i64() == Some(-1)
            || f["minus_one_if_not_primary"].as_u64() == Some(u64::MAX)
        {
            continue;
        }
        let [x, y, z, w] = ["x", "y", "z", "w"].map(|c| {
            f[format!("orientation.{c}")]
                .as_f64()
                .unwrap_or(if c == "w" { 1. } else { 0. })
        });
        let (rx, ry) = (2. * (z * w - x * y), 2. * (x * x + z * z) - 1.);
        return if rx.abs() >= ry.abs() {
            if rx >= 0. {
                0
            } else {
                2
            }
        } else if ry >= 0. {
            1
        } else {
            3
        };
    }
    0
}
