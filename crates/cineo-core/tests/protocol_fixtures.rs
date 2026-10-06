//! Protocol behavior pinned against the fixtures in `tests/fixtures/addons`.
//! Each test names the documented behavior it protects
//! (`docs/ADDON_PROTOCOL.md`).

// Test helpers panic on purpose: a panic is a failed assertion.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use cineo_core::addon::{
    ContentType, ExtraError, ExtraValue, IdFilter, ManifestError, PosterShape, ResourceName,
    ResourcePath, ResponseError, StreamSource, parse_catalog_response, parse_manifest,
    parse_meta_response, parse_stream_response, parse_subtitles_response,
};

fn fixture(path: &str) -> Vec<u8> {
    let full = format!(
        "{}/../../tests/fixtures/addons/{path}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(&full).unwrap_or_else(|err| panic!("reading {full}: {err}"))
}

fn ty(s: &str) -> ContentType {
    ContentType::new(s).unwrap()
}

fn path(resource: &str, content_type: &str, id: &str) -> ResourcePath {
    ResourcePath {
        resource: ResourceName::parse(resource),
        content_type: ty(content_type),
        id: id.to_owned(),
        extra: Vec::new(),
    }
}

#[test]
fn basic_manifest_parses_without_warnings() {
    let parsed = parse_manifest(&fixture("basic/manifest.json")).unwrap();
    assert_eq!(parsed.warnings, vec![]);
    let m = parsed.value;
    assert_eq!(m.id, "org.cineo.fixture.basic");
    assert_eq!(m.version, semver::Version::new(1, 2, 0));
    assert_eq!(m.types, vec![ty("movie"), ty("series")]);
    assert_eq!(m.resources.len(), 4);
    assert_eq!(m.catalogs.len(), 3);
    assert!(m.behavior_hints.configurable);
    assert!(m.logo.is_some());
}

#[test]
fn short_resource_inherits_manifest_types_and_prefixes() {
    let m = parse_manifest(&fixture("basic/manifest.json"))
        .unwrap()
        .value;
    assert!(m.supports(&path("meta", "movie", "tt0000001")));
    assert!(m.supports(&path("meta", "series", "tt0000001")));
    assert!(
        !m.supports(&path("meta", "movie", "kitsu:1")),
        "manifest idPrefixes apply"
    );
    assert!(
        !m.supports(&path("meta", "channel", "tt1")),
        "manifest types apply"
    );
}

#[test]
fn full_resource_uses_only_its_own_filters() {
    let m = parse_manifest(&fixture("basic/manifest.json"))
        .unwrap()
        .value;
    // stream declares its own prefixes, overriding the manifest's ["tt"].
    assert!(m.supports(&path("stream", "series", "kitsu:123")));
    // subtitles declares types but no idPrefixes: every id matches.
    assert!(m.supports(&path("subtitles", "movie", "anything")));
    assert!(!m.supports(&path("subtitles", "series", "tt1")));
    // Undeclared resource.
    assert!(!m.supports(&path("addon_catalog", "movie", "top")));
}

#[test]
fn catalog_support_checks_type_id_and_extra() {
    let m = parse_manifest(&fixture("basic/manifest.json"))
        .unwrap()
        .value;
    let top = ResourcePath::catalog(ty("movie"), "top");
    assert!(m.supports(&top));
    assert!(
        m.supports(
            &top.clone()
                .with_extra(vec![ExtraValue::new("genre", "Action")])
        )
    );
    assert!(!m.supports(&top.with_extra(vec![ExtraValue::new("search", "x")])));
    assert!(!m.supports(&ResourcePath::catalog(ty("series"), "top")));

    let search = m.catalog(&ty("movie"), "search").unwrap();
    assert!(!search.is_browsable());
    assert_eq!(
        search.check_extra(&[]),
        Err(ExtraError::MissingRequired("search".into()))
    );
    assert_eq!(
        search.check_extra(&[ExtraValue::new("search", "matrix")]),
        Ok(())
    );
}

#[test]
fn options_limit_bounds_repeated_extra_values() {
    let m = parse_manifest(&fixture("basic/manifest.json"))
        .unwrap()
        .value;
    let multi = m.catalog(&ty("series"), "multi").unwrap();
    let two = [ExtraValue::new("genre", "A"), ExtraValue::new("genre", "B")];
    assert_eq!(multi.check_extra(&two), Ok(()));
    let three = [
        two[0].clone(),
        two[1].clone(),
        ExtraValue::new("genre", "C"),
    ];
    assert_eq!(
        multi.check_extra(&three),
        Err(ExtraError::TooManyValues {
            name: "genre".into(),
            limit: 2
        })
    );
}

#[test]
fn quirky_manifest_parses_with_expected_warnings() {
    let parsed = parse_manifest(&fixture("quirks/manifest.json")).unwrap();
    let mut locations: Vec<_> = parsed
        .warnings
        .iter()
        .map(|w| w.location.as_str())
        .collect();
    locations.sort_unstable();
    assert_eq!(
        locations,
        vec![
            "background",
            "behaviorHints.adult",
            "catalogs[0].extraRequired",
            "catalogs[1]",
            "catalogs[2]",
            "catalogs[3].extra",
            "catalogs[3].extra[3].optionsLimit",
            "catalogs[3].extra[4]",
            "resources[1].types",
            "resources[3]",
            "resources[4]",
            "types[1]",
            "types[2]",
        ]
    );

    let m = parsed.value;
    assert_eq!(m.description, None);
    assert_eq!(m.logo, None, "empty string means absent");
    assert_eq!(m.types, vec![ty("movie")]);
    assert!(!m.behavior_hints.adult);
    assert!(m.behavior_hints.p2p);

    // Full-form resource without `types` matches nothing.
    assert!(!m.supports(&path("stream", "movie", "tt1")));
    // Empty idPrefixes list matches every id.
    let meta = m
        .resources
        .iter()
        .find(|r| r.name == ResourceName::Meta)
        .unwrap();
    assert_eq!(meta.ids, IdFilter::Any);

    // Short-form extras: only `extraSupported` names exist; `year` is dropped.
    let legacy = m.catalog(&ty("movie"), "legacy").unwrap();
    let names: Vec<_> = legacy
        .extra
        .iter()
        .map(|e| (e.name.as_str(), e.is_required))
        .collect();
    assert_eq!(names, vec![("search", true), ("genre", false)]);
    assert_eq!(legacy.name, None, "duplicate catalog dropped, first kept");

    // `skip` is normalized; duplicates keep the first; optionsLimit 0 -> 1.
    let dupes = m.catalog(&ty("movie"), "dupes").unwrap();
    let skip = dupes.extra.iter().find(|e| e.name == "skip").unwrap();
    assert!(!skip.is_required && skip.options.is_empty());
    let genre = dupes.extra.iter().find(|e| e.name == "genre").unwrap();
    assert!(!genre.is_required);
    let limit = dupes.extra.iter().find(|e| e.name == "limit").unwrap();
    assert_eq!(limit.options_limit, 1);
}

#[test]
fn invalid_manifests_are_rejected_with_specific_errors() {
    assert!(matches!(
        parse_manifest(&fixture("invalid/manifest-bad-version.json")),
        Err(ManifestError::InvalidField {
            field: "version",
            ..
        })
    ));
    assert!(matches!(
        parse_manifest(&fixture("invalid/manifest-missing-types.json")),
        Err(ManifestError::MissingField("types"))
    ));
    assert!(matches!(
        parse_manifest(&fixture("invalid/manifest-empty-id.json")),
        Err(ManifestError::InvalidField { field: "id", .. })
    ));
    assert!(matches!(
        parse_manifest(&fixture("invalid/manifest-not-object.json")),
        Err(ManifestError::NotAnObject)
    ));
    assert!(matches!(parse_manifest(b"{"), Err(ManifestError::Json(_))));
}

#[test]
fn basic_catalog_parses() {
    let parsed = parse_catalog_response(&fixture("basic/catalog-movie-top.json")).unwrap();
    assert_eq!(parsed.warnings, vec![]);
    let metas = parsed.value.metas;
    assert_eq!(metas.len(), 2);
    assert_eq!(metas[0].name, "First Example Film");
    assert_eq!(metas[0].release_info.as_deref(), Some("1999"));
    assert_eq!(metas[0].poster_shape, PosterShape::Poster);
    assert_eq!(metas[1].poster_shape, PosterShape::Landscape);
}

#[test]
fn quirky_catalog_keeps_valid_entries_and_reports_the_rest() {
    let parsed = parse_catalog_response(&fixture("quirks/catalog-mixed.json")).unwrap();
    let ids: Vec<_> = parsed.value.metas.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, vec!["tt1", "tt2", "tt3", "tt5"]);

    let metas = &parsed.value.metas;
    assert_eq!(metas[0].release_info.as_deref(), Some("2010"));
    assert_eq!(metas[0].imdb_rating.as_deref(), Some("6.5"));
    assert_eq!(metas[1].name, "");
    assert_eq!(metas[1].poster, None);
    assert_eq!(metas[2].poster, None, "non-http(s) image URLs are dropped");
    assert_eq!(metas[3].genres, vec!["Drama"]);

    let mut locations: Vec<_> = parsed
        .warnings
        .iter()
        .map(|w| w.location.as_str())
        .collect();
    locations.sort_unstable();
    assert_eq!(
        locations,
        vec![
            "metas[1].posterShape",
            "metas[2].poster",
            "metas[3]",
            "metas[4]",
            "metas[5]",
            "metas[6].genres[1]",
        ]
    );
}

#[test]
fn null_metas_is_an_empty_catalog() {
    let parsed = parse_catalog_response(&fixture("quirks/catalog-null-metas.json")).unwrap();
    assert!(parsed.value.metas.is_empty());
    assert!(parsed.warnings.is_empty());
}

#[test]
fn response_without_metas_is_rejected() {
    assert!(matches!(
        parse_catalog_response(&fixture("invalid/catalog-wrong-resource.json")),
        Err(ResponseError::MissingField("metas"))
    ));
}

#[test]
fn series_meta_parses_and_sorts_videos() {
    let parsed = parse_meta_response(&fixture("basic/meta-series.json")).unwrap();
    assert_eq!(parsed.warnings, vec![]);
    let meta = parsed.value;
    assert_eq!(meta.preview.name, "Example Series");
    assert_eq!(meta.runtime.as_deref(), Some("22 min"));
    let ids: Vec<_> = meta.videos.iter().map(|v| v.id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "tt0000010:0:1",
            "tt0000010:1:1",
            "tt0000010:1:2",
            "tt0000010:2:1"
        ]
    );
    // `name` is the reference-client alias of `title`; `description` backs `overview`.
    assert_eq!(meta.videos[3].title, "Second Season Opener");
    assert_eq!(meta.videos[1].overview.as_deref(), Some("It begins."));
    assert_eq!(meta.seasons(), vec![1, 2, 0], "specials last");
}

#[test]
fn movie_meta_without_videos_has_its_own_id_as_video() {
    let meta = parse_meta_response(&fixture("basic/meta-movie.json"))
        .unwrap()
        .value;
    assert_eq!(meta.video_ids(), vec!["tt0000001"]);
    assert_eq!(meta.runtime.as_deref(), Some("120"));
    assert_eq!(meta.default_video_id.as_deref(), Some("tt0000001"));
}

#[test]
fn stream_sources_are_recognized_in_reference_order() {
    let parsed = parse_stream_response(&fixture("basic/streams-movie.json")).unwrap();
    assert_eq!(parsed.warnings, vec![]);
    let streams = parsed.value;
    let kinds: Vec<_> = streams.iter().map(|s| s.source.kind_label()).collect();
    assert_eq!(
        kinds,
        vec![
            "HTTP", "Torrent", "YouTube", "External", "Archive", "Usenet"
        ]
    );
    assert!(streams[0].source.is_playable());
    assert!(streams[1..].iter().all(|s| !s.source.is_playable()));

    let http = &streams[0];
    assert_eq!(http.binge_group.as_deref(), Some("example-1080p"));
    assert_eq!(http.video_size, Some(1_234_567));
    assert_eq!(http.subtitles.len(), 1);
    assert_eq!(http.request_headers.len(), 2);

    let StreamSource::Torrent {
        info_hash,
        file_idx,
        ..
    } = &streams[1].source
    else {
        panic!("expected torrent");
    };
    assert_eq!(info_hash, "0123456789abcdef0123456789abcdef01234567");
    assert_eq!(*file_idx, Some(2));
    assert_eq!(
        streams[1].description.as_deref(),
        Some("Deprecated title field")
    );
}

#[test]
fn quirky_streams_drop_bad_sources_and_unsafe_headers() {
    let parsed = parse_stream_response(&fixture("quirks/streams-mixed.json")).unwrap();
    let streams = &parsed.value;
    assert_eq!(streams.len(), 2);
    assert!(
        !streams[0].source.is_playable(),
        "rtmp is parsed but not playable"
    );
    assert_eq!(
        streams[1].request_headers,
        vec![("X-Ok".to_owned(), "1".to_owned())],
        "invalid names and CR/LF values are dropped"
    );
    let mut locations: Vec<_> = parsed
        .warnings
        .iter()
        .map(|w| w.location.as_str())
        .collect();
    locations.sort_unstable();
    assert_eq!(
        locations,
        vec![
            "streams[1]",
            "streams[1].url",
            "streams[2]",
            "streams[2].infoHash",
            "streams[3].behaviorHints.proxyHeaders.request.Bad Header",
            "streams[3].behaviorHints.proxyHeaders.request.X-Inject",
            "streams[4]",
            "streams[5]",
        ]
    );
}

#[test]
fn subtitles_require_http_urls() {
    let parsed = parse_subtitles_response(&fixture("basic/subtitles.json")).unwrap();
    assert_eq!(parsed.value.len(), 1);
    assert_eq!(parsed.value[0].label.as_deref(), Some("English [CC]"));
    assert_eq!(parsed.warnings.len(), 2, "ignored url + skipped item");
}
