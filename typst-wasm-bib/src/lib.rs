use core::str;
use std::fs;

use hayagriva::{
    archive::{locales, ArchivedStyle},
    citationberg::{IndependentStyle, LocaleCode, Style},
    io::{from_biblatex_str, from_yaml_str},
    BibliographyDriver, BibliographyItem, BibliographyRequest, BufWriteFormat, CitationItem,
    CitationRequest, Rendered,
};
use serde::Serialize;
use wasm_minimal_protocol::*;

initiate_protocol!();

/// Generates a `Rendered` hayagriva bibliography object (and whether it is sorted) based on the given arguments.
/// - `bib` represents the contents of either a BibTeX file or a hayagriva YAML file.
/// - `format` should be `yaml | bibtex` in order to parse the file contents correctly.
/// - `style` may either represent a file path to the given CSL style or its `ArchivedName`.
/// - `lang` represents a RFC 1766 language code.
/// - `cited` should contain all used citations when `full: false`.
pub(crate) fn generate_bibliography(
    bib: &str,
    format: &str,
    style: &str,
    lang: &str,
    cited: &[&str],
) -> (Rendered, bool) {
    let bib = if format == "yaml" {
        from_yaml_str(bib).unwrap()
    } else if format == "bibtex" {
        from_biblatex_str(bib).unwrap()
    } else {
        panic!("Invalid format!")
    };

    let style = if style.ends_with(".csl") {
        IndependentStyle::from_xml(&fs::read_to_string(style).unwrap()).unwrap()
    } else {
        let Style::Independent(indep) = ArchivedStyle::by_name(style).unwrap().get() else {
            panic!("invalid independent style!")
        };

        indep
    };

    let locales = locales();
    let locale_code = Some(LocaleCode(String::from(lang)));
    let mut driver = BibliographyDriver::new();

    for entry in bib.iter().filter(|e| cited.contains(&e.key())) {
        let items = vec![CitationItem::with_entry(entry)];
        driver.citation(CitationRequest::new(
            items,
            &style,
            locale_code.clone(),
            &locales,
            None,
        ));
    }

    let manual_sort = style.bibliography.clone().unwrap().sort.is_none();
    let result = driver.finish(BibliographyRequest {
        style: &style,
        locale: locale_code,
        locale_files: &locales,
    });

    (result, manual_sort)
}

#[derive(Serialize)]
pub struct BibItem {
    key: String,
    prefix: Option<String>,
    content: String,
}

#[wasm_func]
pub fn parcio_bib(
    bib: &[u8],
    format: &[u8],
    style: &[u8],
    lang: &[u8],
    cited: &[u8],
) -> Result<Vec<u8>, String> {
    let cited_str = str::from_utf8(cited).unwrap();
    let cited = cited_str.split(",").collect::<Vec<_>>();

    let (rendered_bib, manual_sort) = generate_bibliography(
        str::from_utf8(bib).unwrap(),
        str::from_utf8(format).unwrap(),
        str::from_utf8(style).unwrap(),
        str::from_utf8(lang).unwrap(),
        &cited,
    );

    // Check whether the style specifies hanging-indent.
    // Will enable a hanging-indent of 1.5em in Typst markup.
    let hanging_indent = rendered_bib
        .bibliography
        .as_ref()
        .is_some_and(|x| x.hanging_indent);

    /*
    Gather all references and stringify the key, prefix and content.
    Content will be transformed into Typst markup via `BufWriteFormat`.
    Then, serialize into JSON and collect.
    */
    let Some(bibliography) = rendered_bib.bibliography else {
        return Err("invalid bibliography".to_string());
    };
    let mut citation_strings = bibliography
        .items
        .iter()
        .map(
            |BibliographyItem {
                 key,
                 first_field,
                 content,
             }| {
                let stringified_bib_item = BibItem {
                    key: key.clone(),
                    prefix: first_field.as_ref().map(|p| format!("{:#}", p)),
                    content: {
                        let mut buffer = String::new();
                        content
                            .write_buf(&mut buffer, BufWriteFormat::Typst)
                            .unwrap();
                        buffer
                    },
                };

                serde_json::to_string(&stringified_bib_item).unwrap()
            },
        )
        .collect::<Vec<_>>();

    // Append hanging-indent and manual-sort stringified boolean values at the end.
    citation_strings.push(hanging_indent.to_string());
    citation_strings.push(manual_sort.to_string());
    // Separate each item in `citation_strings` with "%%%" and turn into byte vector.
    Ok(citation_strings.join("%%%").as_bytes().to_vec())
}
