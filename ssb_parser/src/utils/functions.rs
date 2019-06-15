// Imports
use super::constants::*;
use std::collections::{HashMap,HashSet};


// Functions
pub fn parse_timestamp(timestamp: &str) -> Result<u32,()> {
    // Milliseconds factors
    const MS_2_MS: u32 = 1;
    const S_2_MS: u32 = MS_2_MS * 1000;
    const M_2_MS: u32 = S_2_MS * 60;
    const H_2_MS: u32 = M_2_MS * 60;
    // Calculate time in milliseconds
    let mut ms = 0u32;
    let captures = TIMESTAMP_PATTERN.captures(timestamp).ok_or_else(|| ())?;
    for (unit, factor) in &[("MS", MS_2_MS), ("S", S_2_MS), ("M", M_2_MS), ("HM", M_2_MS), ("H", H_2_MS)] {
        if let Some(unit_value) = captures.name(unit) {
            if unit_value.start() != unit_value.end() { // Not empty
                ms += unit_value.as_str().parse::<u32>().map_err(|_| ())? * factor;
            }
        }
    }
    // Return time
    Ok(ms)
}

pub fn bool_from_str(text: &str) -> Result<bool,()> {
    match text {
        "y" => Ok(true),
        "n" => Ok(false),
        _ => Err(())
    }
}

pub fn alpha_from_str(text: &str) -> Result<u8,()> {
    match text.len() {
        1...2 => u8::from_str_radix(text, 16).map_err(|_| () ),
        _ => Err(())
    }
}
pub fn rgb_from_str(text: &str) -> Result<[u8;3],()> {
    match text.len() {
        1...6 => u32::from_str_radix(text, 16).map(|value| {let bytes = value.to_le_bytes(); [bytes[2], bytes[1], bytes[0]]} ).map_err(|_| () ),
        _ => Err(()),
    }
}

pub fn flatten_macro<'a>(macro_name: &str, history: &mut HashSet<&'a str>, macros: &'a HashMap<String, String>, flat_macros: &mut HashMap<&'a str, String>) -> Result<(), MacroError> {
    // Macro already flattened?
    if flat_macros.contains_key(macro_name) {
        return Ok(());
    }
    // Macro exists?
    let (macro_name, mut flat_macro_value) = get_key_value(macros, macro_name)
        .map(|key_value| (key_value.0.as_str(), key_value.1.to_owned()))
        .ok_or_else(|| MacroError::NotFound(macro_name.to_owned()))?;
    // Macro already in history (avoid infinite loop!)
    if history.contains(macro_name) {
        return Err(MacroError::InfiniteLoop(macro_name.to_owned()));
    } else {
        history.insert(macro_name);
    }
    // Process macro value
    while let Some(found) = MACRO_PATTERN.find(&flat_macro_value) {
        // Insert sub-macro
        let sub_macro_name = &flat_macro_value[found.start()+MACRO_INLINE_START.len()..found.end()-MACRO_INLINE_END.len()];
        if !flat_macros.contains_key(sub_macro_name) {
            flatten_macro(sub_macro_name, history, macros, flat_macros)?;
        }
        let sub_macro_location = found.start()..found.end();
        let sub_macro_value = flat_macros.get(sub_macro_name).ok_or_else(|| MacroError::NotFound(sub_macro_name.to_owned()))?;
        flat_macro_value.replace_range(sub_macro_location, sub_macro_value);
    }
    // Register flat macro
    flat_macros.insert(
        macro_name,
        flat_macro_value
    );
    // Everything alright
    Ok(())
}
// On stable: <https://doc.rust-lang.org/std/collections/struct.HashMap.html#method.get_key_value>
fn get_key_value<'a,K,V,Q: ?Sized>(map: &'a HashMap<K,V>, k: &Q) -> Option<(&'a K, &'a V)>
    where K: std::borrow::Borrow<Q> + std::hash::Hash + std::cmp::Eq,
        Q: std::hash::Hash + Eq {
    let key = map.keys().find(|key| key.borrow() == k)?;
    Some((key, map.get(key.borrow())?))
}

pub fn map_or_err_str<T,F,U,E>(option: Option<& T>, op: F) -> Result<U,&str>
    where T: AsRef<str> + ?Sized,
        F: FnOnce(&str) -> Result<U,E> {
    option.map_or(Err(""), |value| op(value.as_ref()).map_err(|_| value.as_ref() ))
}
pub fn map_else_err_str<T,F,U>(option: Option<&T>, op: F) -> Result<U,&str>
    where T: AsRef<str> + ?Sized,
        F: FnOnce(&str) -> Option<U> {
    option.map_or(Err(""), |value| op(value.as_ref()).ok_or_else(|| value.as_ref() ))
}


// Structures
#[derive(Debug, PartialEq)]
pub enum MacroError {
    NotFound(String),
    InfiniteLoop(String)
}

pub struct EscapedText {
    text: String,
    tag_starts_ends: Vec<(usize,char)>
}
impl EscapedText {
    pub fn new(source: &str) -> Self {
        let text = source.replace("\\\\", "\x1B").replace(&("\\".to_owned() + TAG_START), "\x02").replace(&("\\".to_owned() + TAG_END), "\x03").replace("\\n", "\n").replace('\x1B', "\\");
        Self {
            text: text.replace('\x02', TAG_START).replace('\x03', TAG_END),
            tag_starts_ends: text.char_indices().filter(|c| c.1 == TAG_START_CHAR || c.1 == TAG_END_CHAR).collect()
        }
    }
    pub fn iter(&self) -> TagGeometryIterator {
        TagGeometryIterator {
            source: self,
            pos: 0
        }
    }
}
pub struct TagGeometryIterator<'src> {
    source: &'src EscapedText,
    pos: usize
}
impl<'src> Iterator for TagGeometryIterator<'src> {
    type Item = (bool, &'src str);
    fn next(&mut self) -> Option<Self::Item> {
        // End of source reached?
        if self.pos == self.source.text.len() {
            return None;
        }
        // Find next tag start
        let tag_start = self.source.tag_starts_ends.iter().find(|c| c.0 >= self.pos && c.1 == TAG_START_CHAR).map(|c| c.0);
        // Match tag or geometry
        let is_tag;
        let text_chunk;
        if tag_start.filter(|pos| *pos == self.pos).is_some() {
            is_tag = true;
            // Till tag end (considers nested tags)
            let mut tag_open_count = 0usize;
            if let Some(end_pos) = self.source.tag_starts_ends.iter().find(|c| match c.1 {
                _ if c.0 < self.pos + TAG_START.len() => false,
                TAG_START_CHAR => {tag_open_count+=1; false}
                TAG_END_CHAR => if tag_open_count == 0 {true} else {tag_open_count-=1; false}
                _ => false
            }).map(|c| c.0) {
                text_chunk = &self.source.text[self.pos + TAG_START.len()..end_pos];
                self.pos = end_pos + TAG_END.len();
            // Till end
            } else {
                text_chunk = &self.source.text[self.pos + TAG_START.len()..];
                self.pos = self.source.text.len();
            }
        } else {
            is_tag = false;
            // Till tag start
            if let Some(tag_start) = tag_start {
                text_chunk = &self.source.text[self.pos..tag_start];
                self.pos = tag_start;
            // Till end
            } else {
                text_chunk = &self.source.text[self.pos..];
                self.pos = self.source.text.len();
            }
        }
        // Return tag or geometry
        Some((is_tag, text_chunk))
    }
}

pub struct TagsIterator<'src> {
    text: &'src str,
    pos: usize
}
impl<'src> TagsIterator<'src> {
    pub fn new(text: &'src str) -> Self {
        Self {
            text,
            pos: 0
        }
    }
}
impl<'src> Iterator for TagsIterator<'src> {
    type Item = (&'src str, Option<&'src str>);
    fn next(&mut self) -> Option<Self::Item> {
        // End of source reached?
        if self.pos == self.text.len() {
            return None;
        }
        // Find next tag separator (considers nested tags)
        let mut tag_open_count = 0usize;
        let tag_sep = self.text.char_indices().skip(self.pos).find(|c| match c.1 {
            TAG_START_CHAR => {tag_open_count+=1; false}
            TAG_END_CHAR => {if tag_open_count > 0 {tag_open_count-=1} false}
            TAG_SEPARATOR if tag_open_count == 0 => true,
            _ => false
        }).map(|c| c.0);
        // Match till separator or end
        let tag_token;
        if let Some(tag_sep) = tag_sep {
            tag_token = &self.text[self.pos..tag_sep];
            self.pos = tag_sep + 1 /* TAG_SEPARATOR */;
        } else {
            tag_token = &self.text[self.pos..];
            self.pos = self.text.len();
        }
        // Split into name+value and return
        if let Some(tag_assign) = tag_token.find(TAG_ASSIGN) {
            Some((&tag_token[..tag_assign], Some(&tag_token[tag_assign + 1 /* TAG_ASSIGN */..])))
        } else {
            Some((tag_token, None))
        }
    }
}


// Tests
#[cfg(test)]
mod tests {
    use super::{
        parse_timestamp,
        bool_from_str,
        alpha_from_str,
        rgb_from_str,
        flatten_macro,
        map_or_err_str,
        map_else_err_str,
        MacroError,
        EscapedText,
        TagsIterator,
        HashMap,
        HashSet
    };

    #[test]
    fn parse_timestamp_various() {
        assert_eq!(parse_timestamp(""), Ok(0));
        assert_eq!(parse_timestamp("1:2.3"), Ok(62_003));
        assert_eq!(parse_timestamp("59:59.999"), Ok(3_599_999));
        assert_eq!(parse_timestamp("1::.1"), Ok(3_600_001));
    }

    #[test]
    fn parse_bool() {
        assert_eq!(bool_from_str("y"), Ok(true));
        assert_eq!(bool_from_str("n"), Ok(false));
        assert_eq!(bool_from_str("no"), Err(()));
    }

    #[test]
    fn parse_rgb_alpha() {
        assert_eq!(alpha_from_str(""), Err(()));
        assert_eq!(alpha_from_str("A"), Ok(10));
        assert_eq!(alpha_from_str("C1"), Ok(193));
        assert_eq!(alpha_from_str("1FF"), Err(()));
        assert_eq!(rgb_from_str(""), Err(()));
        assert_eq!(rgb_from_str("1FF"), Ok([0, 1, 255]));
        assert_eq!(rgb_from_str("808080"), Ok([128, 128, 128]));
        assert_eq!(rgb_from_str("FFFF01"), Ok([255, 255, 1]));
        assert_eq!(rgb_from_str("1FFFFFF"), Err(()));
    }

    #[test]
    fn flatten_macro_success() {
        // Test data
        let mut macros = HashMap::new();
        macros.insert("a".to_owned(), "Hello ${b} test!".to_owned());
        macros.insert("b".to_owned(), "fr${c}".to_owned());
        macros.insert("c".to_owned(), "om".to_owned());
        let mut flat_macros = HashMap::new();
        // Test execution
        flatten_macro("a", &mut HashSet::new(), &macros, &mut flat_macros).unwrap();
        assert_eq!(flat_macros.get("a").unwrap(), "Hello from test!");
    }
    #[test]
    fn flatten_macro_infinite() {
        // Test data
        let mut macros = HashMap::new();
        macros.insert("a".to_owned(), "foo ${b}".to_owned());
        macros.insert("b".to_owned(), "${a} bar".to_owned());
        // Test execution
        assert_eq!(flatten_macro("a", &mut HashSet::new(), &macros, &mut HashMap::new()).unwrap_err(), MacroError::InfiniteLoop("a".to_owned()));
    }
    #[test]
    fn flatten_macro_notfound() {
        assert_eq!(flatten_macro("x", &mut HashSet::new(), &HashMap::new(), &mut HashMap::new()).unwrap_err(), MacroError::NotFound("x".to_owned()));
    }

    #[test]
    fn map_err_str() {
        assert_eq!(map_or_err_str(Some("123"), |value| value.parse()), Ok(123));
        assert_eq!(map_else_err_str(Some("987a"), |value| value.parse::<i32>().ok()), Err("987a"));
    }

    #[test]
    fn compare_macro_errors() {
        assert_ne!(MacroError::InfiniteLoop("".to_owned()), MacroError::NotFound("zzz".to_owned()));
    }
    
    #[test]
    fn tag_geometry_iter() {
        let text = EscapedText::new("[tag1][tag2=[inner_tag]]geometry1\\[geometry1_continue\\\\[tag3]geometry2\\n[tag4");
        let mut iter = text.iter();
        assert_eq!(iter.next(), Some((true, "tag1")));
        assert_eq!(iter.next(), Some((true, "tag2=[inner_tag]")));
        assert_eq!(iter.next(), Some((false, "geometry1[geometry1_continue\\")));
        assert_eq!(iter.next(), Some((true, "tag3")));
        assert_eq!(iter.next(), Some((false, "geometry2\n")));
        assert_eq!(iter.next(), Some((true, "tag4")));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn tags_iter() {
        let mut iter = TagsIterator::new("mode=points;reset;animate=0,-500,[position=200,100.5];color=ff00ff;mask-clear");
        assert_eq!(iter.next(), Some(("mode", Some("points"))));
        assert_eq!(iter.next(), Some(("reset", None)));
        assert_eq!(iter.next(), Some(("animate", Some("0,-500,[position=200,100.5]"))));
        assert_eq!(iter.next(), Some(("color", Some("ff00ff"))));
        assert_eq!(iter.next(), Some(("mask-clear", None)));
        assert_eq!(iter.next(), None);
    }
}