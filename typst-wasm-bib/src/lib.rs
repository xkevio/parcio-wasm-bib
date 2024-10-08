use core::str;

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
/// - `full` represents whether to include all works from the given bibliography files.
/// - `style` may either represent the raw text of the given CSL style or its `ArchivedName`.
/// - `style_format` should be `csl | text` to tell the function what to do with `style`.
/// - `lang` represents a RFC 1766 language code.
/// - `cited` should contain all used citations when `full: false` or None when `full: true`.
pub(crate) fn generate_bibliography(
    bib: &str,
    format: &str,
    full: bool,
    style: &str,
    style_format: &str,
    lang: &str,
    cited: Option<&[&str]>,
) -> Result<Rendered, String> {
    let bib = if format == "yaml" {
        from_yaml_str(bib).unwrap()
    } else if format == "bibtex" {
        from_biblatex_str(bib).unwrap()
    } else {
        return Err("Invalid format!".to_string());
    };

    // If `style_format` is "csl", we expect Typst to pass the raw file contents for us,
    // as we cannot read from the filesystem as a WASM application. Otherwise, use `archive`.
    let style = if style_format == "csl" {
        IndependentStyle::from_xml(style).unwrap()
    } else {
        let Style::Independent(indep) = ArchivedStyle::by_name(style).unwrap().get() else {
            return Err("invalid independent style!".to_string());
        };

        indep
    };

    let locales = locales();
    let locale_code = Some(LocaleCode(String::from(lang)));
    let mut driver = BibliographyDriver::new();

    // If sort is none, we manually sort by order of appearance within the Typst document.
    // The parameter `cited` should represent this order, as such we iterate over it.
    if style
        .bibliography
        .as_ref()
        .is_some_and(|b| b.sort.is_none() && !full)
    {
        for key in cited.unwrap() {
            let entry = bib.get(key);
            if let Some(entry) = entry {
                let items = vec![CitationItem::with_entry(entry)];
                driver.citation(CitationRequest::new(
                    items,
                    &style,
                    locale_code.clone(),
                    &locales,
                    None,
                ));
            } else {
                return Err(format!("Cannot find {} in bibliography file", key));
            }
        }
    } else {
        for entry in bib
            .iter()
            .filter(|e| full || cited.unwrap().contains(&e.key()))
        {
            let items = vec![CitationItem::with_entry(entry)];
            driver.citation(CitationRequest::new(
                items,
                &style,
                locale_code.clone(),
                &locales,
                None,
            ));
        }
    }

    Ok(driver.finish(BibliographyRequest {
        style: &style,
        locale: locale_code,
        locale_files: &locales,
    }))
}

#[derive(Serialize)]
pub struct BibItem {
    key: String,
    prefix: Option<String>,
    content: String,
}

#[wasm_func]
pub fn get_bib_keys(bib: &[u8], format: &[u8]) -> Result<Vec<u8>, String> {
    let bib = str::from_utf8(bib).unwrap();
    let format = str::from_utf8(format).unwrap();

    let bib = if format == "yaml" {
        from_yaml_str(bib).unwrap()
    } else if format == "bibtex" {
        from_biblatex_str(bib).unwrap()
    } else {
        return Err(String::from("Invalid bibliography file format!"));
    };

    let bib_keys = bib.keys().collect::<Vec<_>>();
    Ok(bib_keys.join("%%%").as_bytes().to_vec())
}

#[wasm_func]
pub fn parcio_bib(
    bib: &[u8],
    format: &[u8],
    full: &[u8],
    style: &[u8],
    style_format: &[u8],
    lang: &[u8],
    cited: &[u8],
) -> Result<Vec<u8>, String> {
    let cited_str = str::from_utf8(cited).unwrap();
    let cited = cited_str.split(',').collect::<Vec<_>>();
    let full = str::from_utf8(full).is_ok_and(|f| f == "true");

    let rendered_bib = generate_bibliography(
        str::from_utf8(bib).unwrap(),
        str::from_utf8(format).unwrap(),
        full,
        str::from_utf8(style).unwrap(),
        str::from_utf8(style_format).unwrap(),
        str::from_utf8(lang).unwrap(),
        if full { None } else { Some(&cited) },
    )?;

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

    // Append hanging-indent stringified boolean value at the end.
    citation_strings.push(hanging_indent.to_string());
    // Separate each item in `citation_strings` with "%%%" and turn into byte vector.
    Ok(citation_strings.join("%%%").as_bytes().to_vec())
}
