use core::str;
use std::fs;

use hayagriva::{
    archive::{locales, ArchivedStyle},
    citationberg::{IndependentStyle, LocaleCode, Style},
    io::{from_biblatex_str, from_yaml_str},
    BibliographyDriver, BibliographyRequest, BufWriteFormat, CitationItem, CitationRequest,
    Rendered,
};
use serde::Serialize;
use wasm_minimal_protocol::*;

initiate_protocol!();

// TODO: include locale/language as argument
/// Generates a `Rendered` hayagriva bibliography object (and whether it is sorted) based on the given arguments.
/// - `bib` represents the contents of either a BibTeX file or a hayagriva YAML file.
/// - `format` should be `yaml | bibtex` in order to parse the file contents correctly.
/// - `style` may either represent a file path to the given CSL style or its `ArchivedName`.
/// - `cited` should contain all used citations when `full: false`.
pub(crate) fn generate_bibliography(
    bib: &str,
    format: &str,
    style: &str,
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

    let mut driver = BibliographyDriver::new();
    let locales = locales();

    for entry in bib.iter().filter(|e| cited.contains(&e.key())) {
        let items = vec![CitationItem::with_entry(entry)];
        driver.citation(CitationRequest::new(
            items,
            &style,
            Some(LocaleCode::en_us()),
            &locales,
            None,
        ));
    }

    let manual_sort = style.bibliography.clone().unwrap().sort.is_none();
    let result = driver.finish(BibliographyRequest {
        style: &style,
        locale: Some(LocaleCode::en_us()),
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
    cited: &[u8],
) -> Result<Vec<u8>, String> {
    let cited_str = str::from_utf8(cited).unwrap();
    let cited = cited_str.split(",").collect::<Vec<_>>();

    let (rendered_bib, manual_sort) = generate_bibliography(
        str::from_utf8(bib).unwrap(),
        str::from_utf8(format).unwrap(),
        str::from_utf8(style).unwrap(),
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
    let mut citation_strings = rendered_bib
        .bibliography
        .unwrap()
        .items
        .iter()
        .map(|b| {
            let stringified_bib_item = BibItem {
                key: b.key.clone(),
                prefix: b.first_field.as_ref().map(|p| format!("{:#}", p)),
                content: {
                    let mut buffer = String::new();
                    b.content
                        .write_buf(&mut buffer, BufWriteFormat::Typst)
                        .unwrap();
                    buffer
                },
            };

            serde_json::to_string(&stringified_bib_item).unwrap()
        })
        .collect::<Vec<_>>();

    // Append hanging-indent and manual-sort stringified boolean values at the end.
    citation_strings.push(hanging_indent.to_string());
    citation_strings.push(manual_sort.to_string());
    // Separate each item in `citation_strings` with "%%%" and turn into byte vector.
    Ok(citation_strings.join("%%%").as_bytes().to_vec())
}
