//! Java `DocBookDialect`.

use crate::xml_dialect::{ConstraintKind, DefaultXmlDialect, XmlDialect};

pub struct DocBookDialect {
    inner: DefaultXmlDialect,
}

impl DocBookDialect {
    pub fn new() -> Self {
        let mut inner = DefaultXmlDialect::new();
        inner.define_constraint(ConstraintKind::PublicDoctype, r"-//OASIS//DTD DocBook.*");
        inner.define_paragraph_tags(&[
            "book",
            "bookinfo",
            "title",
            "subtitle",
            "authorgroup",
            "author",
            "firstname",
            "surname",
            "affiliation",
            "orgname",
            "address",
            "email",
            "edition",
            "pubdate",
            "copyright",
            "year",
            "holder",
            "isbn",
            "keywordset",
            "keyword",
            "preface",
            "simpara",
            "para",
            "chapter",
            "table",
            "tgroup",
            "thead",
            "tbody",
            "row",
            "entry",
            "revhistory",
            "revision",
            "revnumber",
            "date",
            "authorinitials",
            "revremark",
            "itemizedlist",
            "listitem",
            "member",
            "releaseinfo",
            "bibliomixed",
            "bibliomset",
            "bridgehead",
            "glossseealso",
            "primaryie",
            "refentrytitle",
            "secondaryie",
            "seealsoie",
            "seeie",
            "synopfragmentref",
            "term",
            "tertiaryie",
            "tocentry",
            "glosssee",
            "section",
        ]);
        inner.define_out_of_turn_tags(&["indexterm"]);
        inner.define_preformat_tags(&[
            "screen",
            "programlisting",
            "synopsis",
            "literallayout",
            "address",
        ]);
        inner.define_translatable_attributes(&["url", "lang", "xml:lang"]);
        Self { inner }
    }
}

impl Default for DocBookDialect {
    fn default() -> Self {
        Self::new()
    }
}

impl XmlDialect for DocBookDialect {
    fn base(&self) -> &DefaultXmlDialect {
        &self.inner
    }
}
