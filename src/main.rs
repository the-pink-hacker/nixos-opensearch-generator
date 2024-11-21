use std::cell::OnceCell;

use clap::Parser;
use reqwest::Url;
use scraper::Html;
use serde::Deserialize;

const META_TAG_REL: &str = "search";
const META_TAG_TYPE: &str = "application/opensearchdescription+xml";

#[derive(Debug, Deserialize, Clone)]
#[serde(from = "OpenSearchDescriptionXml")]
struct OpenSearchDescription {
    short_name: String,
    description: String,
    images: Vec<OpenSearchImage>,
    urls: Vec<OpenSearchUrl>,
}

#[derive(Debug, Deserialize)]
enum OpenSearchDescriptionXmlValue {
    ShortName(String),
    Description(String),
    Image(OpenSearchImage),
    Url(OpenSearchUrl),

    // serde_xml_rs fails to deserialize when this isn't parsed.
    #[allow(unused)]
    SearchForm(Url),
}

#[derive(Debug, Deserialize)]
#[serde(rename = "OpenSearchDescription")]
struct OpenSearchDescriptionXml {
    #[serde(rename = "$value")]
    values: Vec<OpenSearchDescriptionXmlValue>,
}

impl From<OpenSearchDescriptionXml> for OpenSearchDescription {
    fn from(value: OpenSearchDescriptionXml) -> Self {
        let mut images = Vec::new();
        let mut urls = Vec::new();
        let short_name = OnceCell::new();
        let description = OnceCell::new();

        for xml_value in value.values {
            match xml_value {
                OpenSearchDescriptionXmlValue::Url(url) => urls.push(url),
                OpenSearchDescriptionXmlValue::Image(image) => images.push(image),
                OpenSearchDescriptionXmlValue::ShortName(provided_name) => short_name
                    .set(provided_name)
                    .expect("Multiple short name values were provided"),
                OpenSearchDescriptionXmlValue::Description(provided_description) => description
                    .set(provided_description)
                    .expect("Multiple descriptions were provided"),
                OpenSearchDescriptionXmlValue::SearchForm(_) => (),
            }
        }

        Self {
            short_name: short_name.into_inner().unwrap_or_default(),
            description: description.into_inner().unwrap_or_default(),
            images,
            urls,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
struct OpenSearchUrl {
    r#type: String,
    template: Url,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
struct OpenSearchImage {
    r#type: String,
    width: Option<u16>,
    height: Option<u16>,
    #[serde(rename = "$value")]
    url: Url,
}

/// Fetches a html webpage and extracts the open-search protocol information.
#[derive(Debug, Parser)]
#[command(version)]
struct Args {
    /// The website url to convert.
    website: Url,

    #[arg(long, short, action)]
    verbose: bool,
}

async fn get_webpage_raw(url: Url) -> String {
    reqwest::get(url)
        .await
        .expect("Failed to send get request to webpage")
        .text()
        .await
        .expect("Failed to get text response from webpage")
}

fn parse_webpage(webpage_raw: impl AsRef<str>) -> Html {
    Html::parse_document(webpage_raw.as_ref())
}

fn select_opensearch_url(document: &Html, current_url: &Url) -> Url {
    let root = document.root_element();

    let mut url = None;

    'root: for root_child in root.child_elements() {
        if root_child.value().name() == "head" {
            for head_child in root_child.child_elements() {
                let head_child_element = head_child.value();
                if head_child_element
                    .attr("rel")
                    .map(|attr| attr == META_TAG_REL)
                    .unwrap_or_default()
                {
                    if head_child_element
                        .attr("type")
                        .map(|attr| attr == META_TAG_TYPE)
                        .unwrap_or_default()
                    {
                        let url_raw = head_child_element
                            .attr("href")
                            .expect("Failed to get opensearch url from meta tag");
                        url = Some(
                            current_url
                                .join(url_raw)
                                .expect("Incorrectly formatted opensearch url"),
                        );
                        break 'root;
                    }
                }
            }
        }
    }

    url.expect("Failed to locate opensearch meta tag in webpage")
}

async fn get_opensearch_raw(url: Url) -> String {
    reqwest::get(url)
        .await
        .expect("Failed to send opensearch get request")
        .text()
        .await
        .expect("Failed to get opensearch file")
}

fn deserialize_opensearch_xml(xml: impl AsRef<str>) -> OpenSearchDescription {
    serde_xml_rs::from_str(xml.as_ref()).expect("Failed to deserialize opensearch xml data")
}

// Single threaded since multithreading would have no gain.
#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args = Args::parse();

    if args.verbose {
        println!("Fetching HTML page: {}", args.website);
    }

    let webpage_raw = get_webpage_raw(args.website.clone()).await;

    if args.verbose {
        println!("Received webpage; parsing...");
    }

    let webpage = parse_webpage(webpage_raw);
    let opensearch_url = select_opensearch_url(&webpage, &args.website);

    if args.verbose {
        println!("Found opensearch url: {}", opensearch_url);
    }

    let opensearch_raw = get_opensearch_raw(opensearch_url).await;

    if args.verbose {
        println!("Received opensearch file; parsing...");
    }

    let opensearch = deserialize_opensearch_xml(opensearch_raw);

    println!("{:#?}", opensearch);
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn deserialize_xml() {
        let raw = r#"<?xml version="1.0"?>
            <OpenSearchDescription>
                <ShortName>Test</ShortName>
                <Image height="16" width ="16" type="image/x-icon">https://example.com/image.ico</Image>
                <Image height="32" width ="32" type="image/x-icon">https://example.com/image.ico</Image>
                <Url type="text/html" template="https://example.com/search?q={searchTerms}" />
                <Description>Hi there</Description>
                <Url type="application/x-suggestions+json" template="https://example.com/json?q={searchTerms}" />
                <Url type="application/x-suggestions+xml" template="https://example.com/xml?q={searchTerms}" />
            </OpenSearchDescription>
        "#;

        let parsed = serde_xml_rs::from_str::<OpenSearchDescription>(raw).unwrap();
        panic!("{:#?}", parsed);
    }
}
