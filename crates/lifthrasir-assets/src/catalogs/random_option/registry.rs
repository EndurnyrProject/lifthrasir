use bevy::prelude::*;
use lifthrasir_data::RandomOptionData;
use std::collections::BTreeMap;

/// Random option display templates: option id -> printf-style template.
#[derive(Resource, Default)]
pub struct RandomOptionDb {
    names: BTreeMap<u32, String>,
}

impl RandomOptionDb {
    pub fn from_random_option_data(data: RandomOptionData) -> Self {
        Self { names: data.names }
    }

    pub fn template(&self, id: i32) -> Option<&str> {
        let id = u32::try_from(id).ok()?;
        self.names.get(&id).map(String::as_str)
    }

    /// Renders the option's display line, substituting the rolled `value` into
    /// the template's `%d` (`%%` becomes `%`). `None` when the id has no template.
    pub fn line(&self, id: i32, value: i32) -> Option<String> {
        self.template(id).map(|template| render(template, value))
    }

    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

fn render(template: &str, value: i32) -> String {
    let mut out = String::with_capacity(template.len() + 8);
    let mut chars = template.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('d') => out.push_str(&value.to_string()),
            Some('%') => out.push('%'),
            Some(other) => {
                out.push('%');
                out.push(other);
            }
            None => out.push('%'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> RandomOptionDb {
        let mut data = RandomOptionData::default();
        data.names.insert(3, "STR + %d".to_string());
        data.names.insert(9, "MHP + %d%%".to_string());
        data.names.insert(11, "Taeguk %d %s".to_string());
        RandomOptionDb::from_random_option_data(data)
    }

    #[test]
    fn renders_value_into_the_template() {
        assert_eq!(fixture().line(3, 5), Some("STR + 5".to_string()));
    }

    #[test]
    fn renders_percent_templates() {
        assert_eq!(fixture().line(9, 4), Some("MHP + 4%".to_string()));
    }

    #[test]
    fn unknown_filters_are_left_verbatim() {
        assert_eq!(fixture().line(11, 7), Some("Taeguk 7 %s".to_string()));
    }

    #[test]
    fn absent_or_negative_ids_have_no_line() {
        assert_eq!(fixture().line(9999, 1), None);
        assert_eq!(fixture().line(-1, 1), None);
    }

    #[test]
    fn negative_values_render() {
        assert_eq!(fixture().line(3, -2), Some("STR + -2".to_string()));
    }
}
