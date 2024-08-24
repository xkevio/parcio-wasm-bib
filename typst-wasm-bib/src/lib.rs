use core::str;
use std::{fmt::Write, fs};

use hayagriva::{
    archive::{locales, ArchivedStyle},
    citationberg::{IndependentStyle, LocaleCode, Style},
    io::{from_biblatex_str, from_yaml_str},
    BibliographyDriver, BibliographyRequest, BufWriteFormat, CitationItem, CitationRequest,
    Rendered,
};
use wasm_minimal_protocol::*;

initiate_protocol!();

// TODO: include locale/language as argument
fn generate_bibliography(bib: &str, format: &str, style: &str, cited: &[&str]) -> (Rendered, bool) {
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

fn hanging_indent(bib: &Rendered) -> bool {
    if let Some(bibliography) = &bib.bibliography {
        bibliography.hanging_indent
    } else {
        false
    }
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

    let hanging_indent = hanging_indent(&rendered_bib);
    let mut citation_strings = rendered_bib
        .bibliography
        .unwrap()
        .items
        .iter()
        .map(|b| {
            // Write 3-tuple of (key, prefix, content) separated by '%' into buffer.
            let mut buffer = String::new();

            // Write citation key and separator.
            buffer.write_str(&b.key).unwrap();
            buffer.write_char('%').unwrap();

            // Write first field if it exists or "None" if it doesn't.
            if let Some(first_field) = &b.first_field {
                first_field
                    .write_buf(&mut buffer, BufWriteFormat::Plain)
                    .unwrap();
            } else {
                buffer.write_str("None").unwrap();
            }
            buffer.write_char('%').unwrap();

            // Write citation content with typst markup.
            b.content
                .write_buf(&mut buffer, BufWriteFormat::Typst)
                .unwrap();

            buffer
        })
        .collect::<Vec<_>>();

    citation_strings.push(hanging_indent.to_string());
    citation_strings.push(manual_sort.to_string());
    Ok(citation_strings.join("%%%").as_bytes().to_vec())
}
