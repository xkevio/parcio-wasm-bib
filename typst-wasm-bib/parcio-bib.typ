#let wasm-bib = plugin("parcio_wasm_bib.wasm")

// Links citation to correct bibliography entry.
// WARNING: hacky will only work with direct "@"-citations.
#show ref.where(element: none): r => {
  if not str(r.target).starts-with("_") {
    show regex(".+"): x => link(label("_" + str(r.target)), x)
    r
  } else {
    r
  }
}

// Query through citations and collect all pages where citation has been used.
// Then, formats it accordingly (Cited on page x, cited on pages x and y, cited on pages x, y and z).
#let _cite-pages(cc) = context {
  let cite-group = (:)
  let citations = query(ref.where(element: none))
    
  for c in citations {
    if str(c.target) not in cite-group.keys() {
      cite-group.insert(str(c.target), (c.location(),))
    } else {
      cite-group.at(str(c.target)).push(c.location())
    }
  }
  
  let locs = cite-group.at(cc)
    .map(l => link(l, str(counter(page).at(l).first())))
    .dedup(key: l => l.body)
  
  text(rgb("#606060"))[
    #if locs.len() == 1 [
      (Cited on page #locs.first())
    ] else if locs.len() == 2 [
      (Cited on pages #locs.at(0) and #locs.at(1))
    ] else [
      #let loc-str = locs.join(", ", last: " and ")
      (Cited on pages #loc-str)
    ]
  ]
}

/* 
Create "fake" bibliography based on modified hayagriva output (Typst markup) with
more customization possibilities. This calls a WASM Rust plugin which in turn calls 
Hayagriva directly to generate the bibliography and its formatting.

Then, it sends over the bibliography information as JSON, including keys, prefix and so on.
This allows for introspection code to query through all citations to generate backrefs.
*/
#let parcio-bib(path, title: [Bibliography], full: false, style: "ieee", enable-backrefs: false) = context {
  show bibliography: none
  bibliography(path, title: title, full: full, style: style)

  let bibliography-file = read(path)
  let used-citations = query(ref.where(element: none)).dedup().map(c => str(c.target))

  let rendered-bibliography = wasm-bib.parcio_bib(
    bytes(bibliography-file), 
    bytes(if path.ends-with(regex("yml|yaml")) { "yaml" } else { "bibtex" }), 
    bytes(style),
    bytes(text.lang), 
    bytes(used-citations.join(","))
  )

  /* WASM plugin returns `Rendered` as a list of JSON representations of 
    (key, prefix, content) separated by "%%%" with `hanging-indent` 
    and `sort` at the end.
  */
  let rendered-bibliography-str = str(rendered-bibliography).split("%%%");
  let hanging-indent = eval(rendered-bibliography-str.at(-2))
  let manual-sort = eval(rendered-bibliography-str.last())

  let is-grid = json.decode(rendered-bibliography-str.first()).prefix != none

  // CSL did not specify sorting order, hence order equals citation order.
  // Meaning, for now we query for all citations and sort accordingly.
  // FIXME: prefix numbers are off after sorting obviously
  let sorted-bib = if manual-sort {
    rendered-bibliography-str.slice(0, -2).enumerate().sorted(key: ((idx, x)) => {
      let used-idx = used-citations.position(s => s == json.decode(x).key)
      used-idx - idx
    }).map(((.., x)) => x)
  } else {
    rendered-bibliography-str.slice(0, -2)
  }

  heading(title) + v(0.5em)
  if is-grid {
    grid(columns: 2, column-gutter: 0.65em, row-gutter: 1em,
      ..for citation in sorted-bib {
        let (key, prefix, content) = json.decode(citation)
        let backref = if enable-backrefs { _cite-pages(key) } else { none }
        let cite-location = query(ref.where(element: none)).filter(r => r.citation.key == label(key))
        
        (
          link(cite-location.first().location(), [#prefix#label("_" + key)]), 
          eval(content, mode: "markup") + backref
        )
      }
    )
  } else {
    set par(hanging-indent: 1.5em) if hanging-indent
    for citation in sorted-bib {
      let (key, prefix, content) = json.decode(citation)
      let backref = if enable-backrefs { _cite-pages(key) } else { none }
      [#eval(content, mode: "markup")#label("_" + key)#backref]
      v(1em, weak: true)
    }
  }
}

@DuweLMSF0B020

@dataset

#pagebreak()

@DuweLMSF0B020

#set par(justify: true)
#parcio-bib("refs.bib", style: "ieee", title: [Faux Bibliography], enable-backrefs: true)
