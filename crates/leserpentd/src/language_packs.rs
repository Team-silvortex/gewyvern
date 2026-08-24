#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LanguagePackAsset {
    pub(crate) payload: &'static [u8],
}

macro_rules! asset {
    ($name:literal) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/leserpent/src/Leserpent/wwwroot/language-packs/",
            $name
        ))
    };
}

pub(crate) fn find(path: &str) -> Option<LanguagePackAsset> {
    let payload: &'static [u8] = match path {
        "/language-packs/catalog.json" => asset!("catalog.json"),
        "/language-packs/ar.json" => asset!("ar.json"),
        "/language-packs/bn.json" => asset!("bn.json"),
        "/language-packs/cs.json" => asset!("cs.json"),
        "/language-packs/da.json" => asset!("da.json"),
        "/language-packs/el.json" => asset!("el.json"),
        "/language-packs/fa.json" => asset!("fa.json"),
        "/language-packs/fi.json" => asset!("fi.json"),
        "/language-packs/he.json" => asset!("he.json"),
        "/language-packs/hi.json" => asset!("hi.json"),
        "/language-packs/id.json" => asset!("id.json"),
        "/language-packs/it.json" => asset!("it.json"),
        "/language-packs/ms.json" => asset!("ms.json"),
        "/language-packs/nl.json" => asset!("nl.json"),
        "/language-packs/no.json" => asset!("no.json"),
        "/language-packs/pl.json" => asset!("pl.json"),
        "/language-packs/pt-BR.json" => asset!("pt-BR.json"),
        "/language-packs/ru.json" => asset!("ru.json"),
        "/language-packs/sv.json" => asset!("sv.json"),
        "/language-packs/th.json" => asset!("th.json"),
        "/language-packs/tr.json" => asset!("tr.json"),
        "/language-packs/uk.json" => asset!("uk.json"),
        "/language-packs/vi.json" => asset!("vi.json"),
        _ => return None,
    };
    Some(LanguagePackAsset { payload })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_roster_is_exact_and_json_only() {
        let catalog = find("/language-packs/catalog.json").unwrap();
        assert!(catalog.payload.starts_with(b"{"));
        assert!(catalog.payload.ends_with(b"\n"));

        for locale in [
            "ar", "bn", "cs", "da", "el", "fa", "fi", "he", "hi", "id", "it", "ms", "nl", "no",
            "pl", "pt-BR", "ru", "sv", "th", "tr", "uk", "vi",
        ] {
            let path = format!("/language-packs/{locale}.json");
            let pack = find(&path).expect("official language pack must be embedded");
            assert!(pack.payload.starts_with(b"{"));
            assert!(pack.payload.ends_with(b"\n"));
        }
        assert!(find("/language-packs/en.json").is_none());
        assert!(find("/language-packs/../catalog.json").is_none());
        assert!(find("/language-packs/catalog.json?cache=false").is_none());
    }
}
