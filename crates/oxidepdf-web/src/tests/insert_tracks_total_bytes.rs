
    use super::*;

    /// A stored artifact of the given size and insertion order. The temp file
    /// is real but empty; these tests exercise the in-memory accounting only.
    fn artifact(size: u64, seq: u64) -> StoredArtifact {
        StoredArtifact {
            file: NamedTempFile::new().unwrap(),
            kind: Kind::Pdf,
            content_type: "application/pdf",
            size,
            seq,
            last_access: Instant::now(),
        }
    }

    fn state_with_upload_limit(max_total_bytes: u64, max_upload_bytes: u64) -> AppState {
        AppState {
            store: Arc::new(Mutex::new(Store {
                max_total_bytes,
                ..Store::default()
            })),
            seq: Arc::new(AtomicU64::new(0)),
            max_upload_bytes,
        }
    }

    #[test]
    fn insert_tracks_total_bytes() {
        let mut store = Store::default();
        store.insert("a".into(), artifact(100, 0));
        store.insert("b".into(), artifact(250, 1));
        assert_eq!(store.total_bytes, 350);
        assert_eq!(store.artifacts.len(), 2);
    }

    #[test]
    fn replacing_an_id_adjusts_total_bytes() {
        let mut store = Store::default();
        store.insert("a".into(), artifact(100, 0));
        store.insert("a".into(), artifact(40, 1));
        assert_eq!(store.artifacts.len(), 1);
        assert_eq!(store.total_bytes, 40);
    }

    #[test]
    fn remove_subtracts_and_never_underflows() {
        let mut store = Store::default();
        store.insert("a".into(), artifact(100, 0));
        assert!(store.remove("a").is_some());
        assert_eq!(store.total_bytes, 0);
        // Removing a missing id is a no-op and must not underflow.
        assert!(store.remove("missing").is_none());
        assert_eq!(store.total_bytes, 0);
    }

    #[test]
    fn count_cap_evicts_oldest_first() {
        let mut store = Store::default();
        for i in 0..(MAX_ARTIFACTS as u64 + 5) {
            store.insert(format!("k{i}"), artifact(1, i));
        }
        assert_eq!(store.artifacts.len(), MAX_ARTIFACTS);
        // The five lowest seqs (oldest) are gone; the newest survive.
        assert!(!store.artifacts.contains_key("k0"));
        assert!(!store.artifacts.contains_key("k4"));
        assert!(store.artifacts.contains_key("k5"));
        assert_eq!(store.total_bytes, MAX_ARTIFACTS as u64);
    }

    #[test]
    fn size_cap_evicts_until_under_limit() {
        let mut store = Store {
            max_total_bytes: 1000,
            ..Store::default()
        };
        store.insert("a".into(), artifact(500, 0));
        store.insert("b".into(), artifact(500, 1));
        // Both fit exactly at the cap.
        assert_eq!(store.artifacts.len(), 2);
        // One more byte over the cap evicts the oldest.
        store.insert("c".into(), artifact(2, 2));
        assert!(!store.artifacts.contains_key("a"));
        assert!(store.artifacts.contains_key("b"));
        assert!(store.artifacts.contains_key("c"));
        assert!(store.total_bytes <= store.max_total_bytes);
    }

    #[test]
    fn ct_eq_matches_only_identical_strings() {
        assert!(ct_eq("secret", "secret"));
        assert!(!ct_eq("secret", "secres"));
        assert!(!ct_eq("secret", "secre"));
        assert!(ct_eq("", ""));
    }

    #[test]
    fn core_invalid_input_is_returned_to_web_clients() {
        let error = core_error(OxideError::InvalidInput {
            reason: "merge requires at least two PDF inputs".to_owned(),
        });

        assert_eq!(error.0, StatusCode::BAD_REQUEST);
        assert_eq!(
            error.1,
            "invalid input: merge requires at least two PDF inputs"
        );
    }

    #[test]
    fn core_internal_errors_stay_generic() {
        let error = core_error(OxideError::Internal);

        assert_eq!(error.0, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(error.1, "internal server error");
    }

    #[test]
    fn web_resource_limits_are_stricter_than_core_defaults() {
        let limits = web_resource_limits(DEFAULT_MAX_UPLOAD_BYTES);

        assert_eq!(DEFAULT_MAX_UPLOAD_BYTES, 128 * 1024 * 1024);
        assert_eq!(limits.max_input_bytes, Some(DEFAULT_MAX_UPLOAD_BYTES));
        assert_eq!(limits.max_total_input_bytes, Some(DEFAULT_MAX_UPLOAD_BYTES));
        assert_eq!(limits.max_output_bytes, Some(DEFAULT_MAX_UPLOAD_BYTES));
        assert_eq!(limits.timeout_ms, Some(WEB_WORKFLOW_TIMEOUT_MS));
    }

    #[test]
    fn app_state_custom_upload_limit_flows_into_workflow_limits() {
        let state = state_with_upload_limit(1024, 200 * 1024 * 1024);
        let workflow = build_workflow(
            vec![InputSpec {
                id: ArtifactRef::new("input"),
                path: "input".into(),
            }],
            vec![TaskSpec {
                id: TaskId::new("task"),
                op: OperatorSpec::PdfInspect(PdfInspectOptions::Metadata(
                    MetadataInspectOptions::default(),
                )),
                inputs: vec![ArtifactRef::new("input")],
            }],
            ArtifactRef::new("task"),
            state.max_upload_bytes(),
        );

        assert_eq!(workflow.limits.max_input_bytes, Some(200 * 1024 * 1024));
        assert_eq!(
            workflow.limits.max_total_input_bytes,
            Some(200 * 1024 * 1024)
        );
        assert_eq!(workflow.limits.max_output_bytes, Some(200 * 1024 * 1024));
    }

    #[test]
    fn body_limit_allows_multipart_overhead_above_file_limit() {
        let state = state_with_upload_limit(1024, 100 * 1024 * 1024);

        assert_eq!(state.max_upload_bytes(), 100 * 1024 * 1024);
        assert_eq!(
            state.max_body_bytes().unwrap(),
            100 * 1024 * 1024 + MULTIPART_BODY_OVERHEAD_BYTES
        );
    }

    #[test]
    fn total_stored_size_rejects_combined_inputs_before_reading() {
        let state = state_with_upload_limit(10_000, 100);
        state
            .lock()
            .unwrap()
            .insert("a".into(), artifact(60, state.next_seq()));
        state
            .lock()
            .unwrap()
            .insert("b".into(), artifact(50, state.next_seq()));

        let error = enforce_total_stored_size(&state, ["a", "b"]).unwrap_err();

        assert_eq!(error.0, StatusCode::PAYLOAD_TOO_LARGE);
        assert!(error.1.contains("max_total_input_bytes"));
    }

    #[test]
    fn total_stored_size_counts_distinct_workflow_files_once() {
        let state = state_with_upload_limit(10_000, 100);
        state
            .lock()
            .unwrap()
            .insert("a".into(), artifact(60, state.next_seq()));

        assert!(enforce_total_stored_size(&state, ["a"]).is_ok());
    }

    #[test]
    fn same_origin_allows_missing_origin_for_non_browser_clients() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, "127.0.0.1:19898".parse().unwrap());

        assert!(same_origin(&headers));
    }

    #[test]
    fn same_origin_rejects_cross_site_origin() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, "127.0.0.1:19898".parse().unwrap());
        headers.insert(header::ORIGIN, "https://example.invalid".parse().unwrap());

        assert!(!same_origin(&headers));
    }

    #[test]
    fn same_origin_accepts_matching_origin() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, "127.0.0.1:19898".parse().unwrap());
        headers.insert(header::ORIGIN, "http://127.0.0.1:19898".parse().unwrap());

        assert!(same_origin(&headers));
    }

    #[test]
    fn same_origin_accepts_https_and_rejects_port_mismatch() {
        let mut https_headers = HeaderMap::new();
        https_headers.insert(header::HOST, "127.0.0.1:19898".parse().unwrap());
        https_headers.insert(header::ORIGIN, "https://127.0.0.1:19898".parse().unwrap());
        assert!(same_origin(&https_headers));

        let mut port_headers = HeaderMap::new();
        port_headers.insert(header::HOST, "127.0.0.1:19898".parse().unwrap());
        port_headers.insert(header::ORIGIN, "http://127.0.0.1:19899".parse().unwrap());
        assert!(!same_origin(&port_headers));
    }

    #[test]
    fn upload_dir_is_created_under_current_working_directory() {
        let base = tempfile::tempdir().unwrap();

        let dir = ensure_upload_dir_under(base.path()).unwrap();

        assert_eq!(dir, base.path().join(UPLOAD_DIR));
        assert!(dir.is_dir());
    }

    #[test]
    fn upload_temp_file_uses_upload_directory() {
        let base = tempfile::tempdir().unwrap();
        let dir = ensure_upload_dir_under(base.path()).unwrap();

        let file = tempfile::Builder::new()
            .prefix(".tmp")
            .tempfile_in(&dir)
            .unwrap();

        assert_eq!(
            file.path().parent().unwrap(),
            dir.as_path(),
            "tempfile should be created under the upload directory"
        );
    }

    #[test]
    fn single_request_rejects_empty_and_excessive_inputs() {
        let empty = SingleReq {
            artifact_ids: Vec::new(),
            family: "PdfInspect".to_owned(),
            op: "Metadata".to_owned(),
            options_json: "{}".to_owned(),
        };
        assert!(validate_single_request(&empty).is_err());

        let excessive = SingleReq {
            artifact_ids: vec!["id".to_owned(); MAX_SINGLE_INPUTS + 1],
            family: "PdfEdit".to_owned(),
            op: "Merge".to_owned(),
            options_json: "{}".to_owned(),
        };
        assert!(validate_single_request(&excessive).is_err());
    }
