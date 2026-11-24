#[cfg(test)]
mod suite {
    use crate::compress::{self, Encoding};
    use crate::fsutil::{self, has_extension};
    use crate::logging::human_bytes;
    use crate::responder::guess_mime;
    use std::path::Path;

    // ---- path safety ------------------------------------------------------

    #[test]
    fn safe_segments_accepts_normal_paths() {
        assert_eq!(
            fsutil::safe_segments("/assets/app.js").unwrap(),
            vec!["assets", "app.js"]
        );
        assert!(fsutil::safe_segments("/").unwrap().is_empty());
        assert_eq!(
            fsutil::safe_segments("/a/b/c/").unwrap(),
            vec!["a", "b", "c"]
        );
        // percent-decoded spaces
        assert_eq!(
            fsutil::safe_segments("/my%20file.txt").unwrap(),
            vec!["my file.txt"]
        );
    }

    #[test]
    fn safe_segments_rejects_traversal() {
        assert!(fsutil::safe_segments("/../../etc/passwd").is_err());
        assert!(fsutil::safe_segments("/a/../../b").is_err());
        assert!(fsutil::safe_segments("/%2e%2e/%2e%2e/etc").is_err());
        assert!(fsutil::safe_segments("/a\\b").is_err());
        assert!(fsutil::safe_segments("/C:/windows").is_err());
        assert!(fsutil::safe_segments("/a%00b").is_err());
        assert!(fsutil::safe_segments("/x/%01").is_err());
    }

    #[test]
    fn extension_helpers() {
        assert!(has_extension(Path::new("/a/app.js")));
        assert!(!has_extension(Path::new("/a/about")));
        assert!(!has_extension(Path::new("/a/.hidden")));
        assert_eq!(
            fsutil::append_ext(Path::new("/a/about"), "html"),
            Path::new("/a/about.html")
        );
    }

    #[test]
    fn url_encoding_escapes_dangerous_chars() {
        assert_eq!(fsutil::url_encode_segment("a b.txt"), "a%20b.txt");
        assert_eq!(fsutil::url_encode_segment("q?x"), "q%3Fx");
        assert_eq!(
            fsutil::url_encode_segment("plain-name_1.js"),
            "plain-name_1.js"
        );
    }

    // ---- compression ------------------------------------------------------

    #[test]
    fn compressible_types() {
        assert!(compress::is_compressible("text/html", Path::new("a.html")));
        assert!(compress::is_compressible("text/css", Path::new("a.css")));
        assert!(compress::is_compressible(
            "application/javascript",
            Path::new("a.js")
        ));
        assert!(compress::is_compressible(
            "image/svg+xml",
            Path::new("a.svg")
        ));
        assert!(!compress::is_compressible("image/png", Path::new("a.png")));
        assert!(!compress::is_compressible(
            "font/woff2",
            Path::new("a.woff2")
        ));
        assert!(!compress::is_compressible("video/mp4", Path::new("a.mp4")));
        assert!(!compress::is_compressible("text/plain", Path::new("a.gz")));
    }

    #[test]
    fn encoding_preference() {
        let mk = |v: &str| {
            let mut m = hyper::HeaderMap::new();
            m.insert(
                hyper::header::ACCEPT_ENCODING,
                hyper::header::HeaderValue::from_str(v).unwrap(),
            );
            m
        };
        assert_eq!(
            compress::negotiate(&mk("gzip"), true, true),
            Some(Encoding::Gzip)
        );
        assert_eq!(
            compress::negotiate(&mk("br"), true, true),
            Some(Encoding::Brotli)
        );
        assert_eq!(
            compress::negotiate(&mk("gzip, deflate, br"), true, true),
            Some(Encoding::Brotli)
        );
        assert_eq!(
            compress::negotiate(&mk("gzip"), true, false),
            Some(Encoding::Gzip)
        );
        assert_eq!(
            compress::negotiate(&mk("br"), false, true),
            Some(Encoding::Brotli)
        );
        assert_eq!(compress::negotiate(&mk("gzip"), false, false), None);
        assert_eq!(compress::negotiate(&mk("identity"), true, true), None);
        // q=0 explicitly refuses
        assert_eq!(compress::negotiate(&mk("gzip;q=0"), true, true), None);
        // wildcard allows everything
        assert_eq!(
            compress::negotiate(&mk("*"), true, true),
            Some(Encoding::Brotli)
        );
        assert_eq!(
            compress::negotiate(&hyper::HeaderMap::new(), true, true),
            None
        );
    }

    // ---- mime -------------------------------------------------------------

    #[test]
    fn mime_overrides() {
        assert_eq!(guess_mime(Path::new("a.mjs")), "application/javascript");
        assert_eq!(guess_mime(Path::new("a.wasm")), "application/wasm");
        assert_eq!(
            guess_mime(Path::new("a.webmanifest")),
            "application/manifest+json"
        );
        assert_eq!(guess_mime(Path::new("a.woff2")), "font/woff2");
        assert_eq!(guess_mime(Path::new("index.html")), "text/html");
        assert_eq!(guess_mime(Path::new("a.css")), "text/css");
        assert_eq!(
            guess_mime(Path::new("unknown.zzz")),
            "application/octet-stream"
        );
    }

    // ---- auth -------------------------------------------------------------

    #[test]
    fn basic_auth() {
        let mut h = hyper::HeaderMap::new();
        // "admin:s3cret" -> YWRtaW46czNjcmV0
        h.insert(
            hyper::header::AUTHORIZATION,
            hyper::header::HeaderValue::from_static("Basic YWRtaW46czNjcmV0"),
        );
        assert!(crate::auth::authorized(&h, "admin", "s3cret"));
        assert!(!crate::auth::authorized(&h, "admin", "wrong"));
        assert!(!crate::auth::authorized(&h, "root", "s3cret"));

        let empty = hyper::HeaderMap::new();
        assert!(!crate::auth::authorized(&empty, "admin", "s3cret"));

        let mut bad = hyper::HeaderMap::new();
        bad.insert(
            hyper::header::AUTHORIZATION,
            hyper::header::HeaderValue::from_static("Bearer nope"),
        );
        assert!(!crate::auth::authorized(&bad, "admin", "s3cret"));
    }

    // ---- misc -------------------------------------------------------------

    #[test]
    fn human_bytes_formatting() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(1024), "1.0 kB");
        assert_eq!(human_bytes(1536), "1.5 kB");
        assert_eq!(human_bytes(1024 * 1024 * 3), "3.0 MB");
    }

    #[test]
    fn html_escaping() {
        assert_eq!(
            fsutil::html_escape("<script>\"x\"&'y'</script>"),
            "&lt;script&gt;&quot;x&quot;&amp;&#39;y&#39;&lt;/script&gt;"
        );
    }

    #[test]
    fn client_ip_uses_forwarded_header_when_trusted() {
        let mut h = hyper::HeaderMap::new();
        h.insert(
            hyper::header::HeaderName::from_static("x-forwarded-for"),
            hyper::header::HeaderValue::from_static("203.0.113.7, 10.0.0.1"),
        );
        assert_eq!(
            crate::logging::client_ip(&h, "127.0.0.1", true),
            "203.0.113.7"
        );
        assert_eq!(
            crate::logging::client_ip(&h, "127.0.0.1", false),
            "127.0.0.1"
        );
    }
}
