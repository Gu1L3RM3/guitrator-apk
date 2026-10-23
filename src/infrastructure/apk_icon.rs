//! Resolves an app's launcher icon the way Android does: read the binary manifest for the
//! `application` icon reference, look that reference up in `resources.arsc`, and follow
//! adaptive-icon XML to a raster image when needed.
//!
//! Only the parts of the AXML and ARSC formats needed for this are read. Anything unexpected
//! makes the lookup return `None`, and the caller falls back to a name heuristic.

const CHUNK_RES_TABLE: u16 = 0x0002;
const CHUNK_XML: u16 = 0x0003;
const CHUNK_STRING_POOL: u16 = 0x0001;
const CHUNK_XML_START_ELEMENT: u16 = 0x0102;
const CHUNK_PACKAGE: u16 = 0x0200;
const CHUNK_TABLE_TYPE: u16 = 0x0201;

const TYPE_REFERENCE: u8 = 0x01;
const TYPE_STRING: u8 = 0x03;

/// Finds the zip entry of the raster (PNG or WebP) launcher icon of an APK.
///
/// `read` returns the bytes of a named entry.
pub fn find_icon_entry_with(mut read: impl FnMut(&str) -> Option<Vec<u8>>) -> Option<String> {
    let manifest = read("AndroidManifest.xml")?;
    let arsc = read("resources.arsc")?;
    let icon_id = manifest_icon_id(&manifest)?;

    let paths = resolve_file_paths(&arsc, icon_id);
    if let Some(raster) = best_raster(&paths) {
        return Some(raster);
    }

    // Adaptive icon: the reference points to an XML file that names its raster layers.
    let (_, xml_path) = paths
        .iter()
        .filter(|(_, p)| p.ends_with(".xml"))
        .max_by_key(|(density, _)| *density)?;
    let xml = read(xml_path)?;
    parse_xml(&xml)?
        .iter()
        .flat_map(|element| &element.attrs)
        .filter(|attr| attr.data_type == TYPE_REFERENCE)
        .find_map(|attr| best_raster(&resolve_file_paths(&arsc, attr.data)))
}

/// Resource id of the `icon` attribute of `<application>` in the binary manifest.
fn manifest_icon_id(manifest: &[u8]) -> Option<u32> {
    parse_xml(manifest)?
        .iter()
        .find(|element| element.name == "application")?
        .attrs
        .iter()
        .find(|attr| attr.name == "icon" && attr.data_type == TYPE_REFERENCE)
        .map(|attr| attr.data)
}

/// Among the paths of raster images, returns the one from the densest configuration.
fn best_raster(paths: &[(u16, String)]) -> Option<String> {
    paths
        .iter()
        .filter(|(_, p)| {
            let lower = p.to_lowercase();
            lower.ends_with(".png") || lower.ends_with(".webp")
        })
        .max_by_key(|(density, _)| *density)
        .map(|(_, p)| p.clone())
}

/// Every file path that resource `id` resolves to, with the density of its configuration.
/// Returns an empty list when the id is not found or the table is not understood.
fn resolve_file_paths(arsc: &[u8], id: u32) -> Vec<(u16, String)> {
    try_resolve(arsc, id).unwrap_or_default()
}

fn try_resolve(arsc: &[u8], id: u32) -> Option<Vec<(u16, String)>> {
    let package_id = id >> 24;
    let type_id = ((id >> 16) & 0xff) as u8;
    let entry_index = (id & 0xffff) as usize;
    if u16_at(arsc, 0)? != CHUNK_RES_TABLE {
        return None;
    }

    let mut global_strings = Vec::new();
    let mut paths = Vec::new();
    let mut off = u16_at(arsc, 2)? as usize;
    while off < arsc.len() {
        let kind = u16_at(arsc, off)?;
        let size = u32_at(arsc, off + 4)? as usize;
        if size == 0 {
            break;
        }
        match kind {
            CHUNK_STRING_POOL if global_strings.is_empty() => {
                global_strings = parse_string_pool(arsc, off)?;
            }
            CHUNK_PACKAGE if u32_at(arsc, off + 8)? == package_id => {
                let header = u16_at(arsc, off + 2)? as usize;
                let end = (off + size).min(arsc.len());
                let mut child = off + header;
                while child + 8 <= end {
                    let child_size = u32_at(arsc, child + 4)? as usize;
                    if child_size == 0 {
                        break;
                    }
                    if u16_at(arsc, child)? == CHUNK_TABLE_TYPE
                        && u8_at(arsc, child + 8)? == type_id
                    {
                        if let Some(path) = type_entry(arsc, child, entry_index, &global_strings) {
                            paths.push(path);
                        }
                    }
                    child += child_size;
                }
            }
            _ => {}
        }
        off += size;
    }
    Some(paths)
}

/// Reads entry `entry_index` of one type chunk, if it holds a string value.
fn type_entry(
    arsc: &[u8],
    chunk: usize,
    entry_index: usize,
    global_strings: &[String],
) -> Option<(u16, String)> {
    let header = u16_at(arsc, chunk + 2)? as usize;
    let entry_count = u32_at(arsc, chunk + 12)? as usize;
    let entries_start = u32_at(arsc, chunk + 16)? as usize;
    let flags = u8_at(arsc, chunk + 9)?;
    if entry_index >= entry_count {
        return None;
    }

    let offset = entry_offset(arsc, chunk + header, flags, entry_count, entry_index)?;
    let entry = chunk + entries_start + offset;
    let entry_size = u16_at(arsc, entry)? as usize;
    let entry_flags = u16_at(arsc, entry + 2)?;
    if entry_flags & 0x0001 != 0 {
        return None; // bag entries (styles, arrays) are not file references
    }

    let value = entry + entry_size;
    if u8_at(arsc, value + 3)? != TYPE_STRING {
        return None;
    }
    let path = global_strings
        .get(u32_at(arsc, value + 4)? as usize)?
        .clone();
    let density = u16_at(arsc, chunk + 20 + 14)?;
    Some((density, path))
}

/// Offset of entry `index` from the start of the entries area, or `None` if the type has no
/// such entry. Three layouts exist: dense (u32 per entry), sparse (idx/offset pairs), and
/// offset16 (u16 per entry). Offsets in the last two are stored divided by 4.
fn entry_offset(arsc: &[u8], table: usize, flags: u8, count: usize, index: usize) -> Option<usize> {
    const FLAG_SPARSE: u8 = 0x01;
    const FLAG_OFFSET16: u8 = 0x02;
    if flags & FLAG_SPARSE != 0 {
        for i in 0..count {
            let at = table + 4 * i;
            if u16_at(arsc, at)? as usize == index {
                return Some(u16_at(arsc, at + 2)? as usize * 4);
            }
        }
        None
    } else if flags & FLAG_OFFSET16 != 0 {
        match u16_at(arsc, table + 2 * index)? {
            u16::MAX => None,
            offset => Some(offset as usize * 4),
        }
    } else {
        match u32_at(arsc, table + 4 * index)? {
            u32::MAX => None,
            offset => Some(offset as usize),
        }
    }
}

struct XmlAttr {
    name: String,
    data_type: u8,
    data: u32,
}

struct XmlElement {
    name: String,
    attrs: Vec<XmlAttr>,
}

/// Parses the start elements of a binary XML document, in document order.
fn parse_xml(doc: &[u8]) -> Option<Vec<XmlElement>> {
    if u16_at(doc, 0)? != CHUNK_XML {
        return None;
    }
    let mut strings = Vec::new();
    let mut elements = Vec::new();
    let mut off = u16_at(doc, 2)? as usize;
    while off < doc.len() {
        let kind = u16_at(doc, off)?;
        let size = u32_at(doc, off + 4)? as usize;
        if size == 0 {
            break;
        }
        match kind {
            CHUNK_STRING_POOL => strings = parse_string_pool(doc, off)?,
            CHUNK_XML_START_ELEMENT => elements.push(parse_element(doc, off, &strings)?),
            _ => {}
        }
        off += size;
    }
    Some(elements)
}

fn parse_element(doc: &[u8], off: usize, strings: &[String]) -> Option<XmlElement> {
    // Chunk header (8) + line number and comment (8), then the element's attribute block.
    let block = off + 16;
    let name = strings.get(u32_at(doc, block + 4)? as usize)?.clone();
    let attr_start = u16_at(doc, block + 8)? as usize;
    let attr_size = u16_at(doc, block + 10)? as usize;
    let count = u16_at(doc, block + 12)? as usize;

    let mut attrs = Vec::with_capacity(count);
    for i in 0..count {
        let a = block + attr_start + i * attr_size;
        attrs.push(XmlAttr {
            name: strings
                .get(u32_at(doc, a + 4)? as usize)
                .cloned()
                .unwrap_or_default(),
            data_type: u8_at(doc, a + 15)?,
            data: u32_at(doc, a + 16)?,
        });
    }
    Some(XmlElement { name, attrs })
}

/// Reads a string pool chunk, in either UTF-8 or UTF-16 encoding.
fn parse_string_pool(data: &[u8], base: usize) -> Option<Vec<String>> {
    let header = u16_at(data, base + 2)? as usize;
    let count = u32_at(data, base + 8)? as usize;
    let utf8 = u32_at(data, base + 16)? & 0x100 != 0;
    let strings_start = u32_at(data, base + 20)? as usize;

    (0..count)
        .map(|i| {
            let offset = u32_at(data, base + header + 4 * i)? as usize;
            let at = base + strings_start + offset;
            if utf8 {
                read_utf8(data, at)
            } else {
                read_utf16(data, at)
            }
        })
        .collect()
}

fn read_utf8(data: &[u8], at: usize) -> Option<String> {
    // Each length is one byte, or two bytes when the high bit of the first one is set.
    let mut p = at;
    if u8_at(data, p)? & 0x80 != 0 {
        p += 1;
    }
    p += 1;
    let mut len = u8_at(data, p)? as usize;
    p += 1;
    if len & 0x80 != 0 {
        len = ((len & 0x7f) << 8) | u8_at(data, p)? as usize;
        p += 1;
    }
    Some(String::from_utf8_lossy(data.get(p..p + len)?).into_owned())
}

fn read_utf16(data: &[u8], at: usize) -> Option<String> {
    let mut len = u16_at(data, at)? as usize;
    let mut p = at + 2;
    if len & 0x8000 != 0 {
        len = ((len & 0x7fff) << 16) | u16_at(data, p)? as usize;
        p += 2;
    }
    let units = (0..len)
        .map(|i| u16_at(data, p + 2 * i))
        .collect::<Option<Vec<u16>>>()?;
    Some(String::from_utf16_lossy(&units))
}

fn u8_at(d: &[u8], o: usize) -> Option<u8> {
    d.get(o).copied()
}

fn u16_at(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(o..o + 2)?.try_into().ok()?))
}

fn u32_at(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push_u16(out: &mut Vec<u8>, v: u16) {
        out.extend_from_slice(&v.to_le_bytes());
    }

    fn push_u32(out: &mut Vec<u8>, v: u32) {
        out.extend_from_slice(&v.to_le_bytes());
    }

    /// UTF-8 string pool with the given strings.
    fn utf8_pool(strings: &[&str]) -> Vec<u8> {
        let mut encoded = Vec::new();
        let mut offsets = Vec::new();
        for s in strings {
            offsets.push(encoded.len() as u32);
            encoded.extend_from_slice(&[s.len() as u8, s.len() as u8]);
            encoded.extend_from_slice(s.as_bytes());
            encoded.push(0);
        }
        let strings_start = 28 + 4 * strings.len() as u32;
        let size = strings_start + encoded.len() as u32;
        let mut out = Vec::new();
        push_u16(&mut out, CHUNK_STRING_POOL);
        push_u16(&mut out, 28);
        push_u32(&mut out, size);
        push_u32(&mut out, strings.len() as u32);
        push_u32(&mut out, 0);
        push_u32(&mut out, 0x100);
        push_u32(&mut out, strings_start);
        push_u32(&mut out, 0);
        for o in offsets {
            push_u32(&mut out, o);
        }
        out.extend_from_slice(&encoded);
        out
    }

    /// Binary manifest whose `<application>` has `android:icon` = `icon_id`.
    fn manifest_with_icon(icon_id: u32) -> Vec<u8> {
        let pool = utf8_pool(&["application", "icon"]);
        let mut element = Vec::new();
        push_u16(&mut element, CHUNK_XML_START_ELEMENT);
        push_u16(&mut element, 16);
        push_u32(&mut element, 56);
        push_u32(&mut element, 0); // line number
        push_u32(&mut element, u32::MAX); // comment
        push_u32(&mut element, u32::MAX); // namespace
        push_u32(&mut element, 0); // element name: "application"
        push_u16(&mut element, 20); // attribute start
        push_u16(&mut element, 20); // attribute size
        push_u16(&mut element, 1); // attribute count
        push_u16(&mut element, 0);
        push_u16(&mut element, 0);
        push_u16(&mut element, 0);
        push_u32(&mut element, u32::MAX); // namespace
        push_u32(&mut element, 1); // attribute name: "icon"
        push_u32(&mut element, u32::MAX); // raw value
        push_u16(&mut element, 8); // typed value size
        element.push(0);
        element.push(TYPE_REFERENCE);
        push_u32(&mut element, icon_id);

        let mut doc = Vec::new();
        push_u16(&mut doc, CHUNK_XML);
        push_u16(&mut doc, 8);
        push_u32(&mut doc, (8 + pool.len() + element.len()) as u32);
        doc.extend_from_slice(&pool);
        doc.extend_from_slice(&element);
        doc
    }

    /// Resource table with one package (0x7f), one type (id 2) and one entry (index 0)
    /// whose xxhdpi configuration is a string pointing at `path`.
    fn table_with_png(path: &str) -> Vec<u8> {
        let global = utf8_pool(&[path]);

        let mut type_chunk = Vec::new();
        push_u16(&mut type_chunk, CHUNK_TABLE_TYPE);
        push_u16(&mut type_chunk, 84); // header with a 64-byte config
        push_u32(&mut type_chunk, 0); // size, filled in below
        type_chunk.push(2); // type id
        type_chunk.push(0); // flags
        push_u16(&mut type_chunk, 0);
        push_u32(&mut type_chunk, 1); // entry count
        push_u32(&mut type_chunk, 88); // entries start
        let mut config = vec![0u8; 64];
        config[0..4].copy_from_slice(&64u32.to_le_bytes());
        config[14..16].copy_from_slice(&480u16.to_le_bytes()); // xxhdpi
        type_chunk.extend_from_slice(&config);
        push_u32(&mut type_chunk, 0); // entry 0 at offset 0
        push_u16(&mut type_chunk, 8); // entry header size
        push_u16(&mut type_chunk, 0); // entry flags
        push_u32(&mut type_chunk, 0); // key
        push_u16(&mut type_chunk, 8); // Res_value size
        type_chunk.push(0);
        type_chunk.push(TYPE_STRING);
        push_u32(&mut type_chunk, 0); // global string index 0
        let len = type_chunk.len() as u32;
        type_chunk[4..8].copy_from_slice(&len.to_le_bytes());

        let mut package = vec![0u8; 288];
        package[0..2].copy_from_slice(&CHUNK_PACKAGE.to_le_bytes());
        package[2..4].copy_from_slice(&288u16.to_le_bytes());
        let package_size = (288 + type_chunk.len()) as u32;
        package[4..8].copy_from_slice(&package_size.to_le_bytes());
        package[8..12].copy_from_slice(&0x7fu32.to_le_bytes());
        package.extend_from_slice(&type_chunk);

        let mut table = Vec::new();
        push_u16(&mut table, CHUNK_RES_TABLE);
        push_u16(&mut table, 12);
        push_u32(&mut table, (12 + global.len() + package.len()) as u32);
        push_u32(&mut table, 1);
        table.extend_from_slice(&global);
        table.extend_from_slice(&package);
        table
    }

    #[test]
    fn reads_icon_reference_from_manifest() {
        let manifest = manifest_with_icon(0x7f02_0000);
        assert_eq!(manifest_icon_id(&manifest), Some(0x7f02_0000));
    }

    #[test]
    fn resolves_reference_to_png_path_in_densest_config() {
        let table = table_with_png("res/mipmap-xxhdpi/ic_launcher.png");
        let paths = resolve_file_paths(&table, 0x7f02_0000);
        assert_eq!(
            paths,
            vec![(480, "res/mipmap-xxhdpi/ic_launcher.png".to_string())]
        );
        assert_eq!(
            best_raster(&paths).as_deref(),
            Some("res/mipmap-xxhdpi/ic_launcher.png")
        );
    }

    #[test]
    fn unknown_resource_resolves_to_nothing() {
        let table = table_with_png("res/mipmap-xxhdpi/ic_launcher.png");
        assert!(resolve_file_paths(&table, 0x7f02_0005).is_empty());
        assert!(resolve_file_paths(b"garbage", 0x7f02_0000).is_empty());
    }
}
