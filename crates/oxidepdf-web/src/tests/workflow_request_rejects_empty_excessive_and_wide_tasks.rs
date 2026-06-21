
    #[test]
    fn workflow_request_rejects_empty_excessive_and_wide_tasks() {
        assert!(validate_workflow_request(&WorkflowReq { tasks: Vec::new() }).is_err());

        let too_many_tasks = WorkflowReq {
            tasks: (0..=MAX_WORKFLOW_TASKS)
                .map(|_| WfTask {
                    family: "PdfInspect".to_owned(),
                    op: "Metadata".to_owned(),
                    options_json: "{}".to_owned(),
                    inputs: vec![WfInput::File {
                        id: "file".to_owned(),
                    }],
                })
                .collect(),
        };
        assert!(validate_workflow_request(&too_many_tasks).is_err());

        let too_many_inputs = WorkflowReq {
            tasks: vec![WfTask {
                family: "PdfEdit".to_owned(),
                op: "Merge".to_owned(),
                options_json: "{}".to_owned(),
                inputs: (0..=MAX_TASK_INPUTS)
                    .map(|index| WfInput::File {
                        id: format!("file{index}"),
                    })
                    .collect(),
            }],
        };
        assert!(validate_workflow_request(&too_many_inputs).is_err());
    }

    #[test]
    fn sweep_expired_drops_only_stale_entries() {
        // Skip if the monotonic clock is younger than the TTL (e.g. CI booted
        // moments ago) — can't construct a timestamp older than the origin.
        let Some(stale_ts) = Instant::now().checked_sub(ARTIFACT_TTL + Duration::from_secs(1))
        else {
            return;
        };
        let mut store = Store::default();
        let mut fresh = artifact(10, 0);
        fresh.last_access = Instant::now();
        let mut stale = artifact(20, 1);
        stale.last_access = stale_ts;
        store.insert("fresh".into(), fresh);
        store.insert("stale".into(), stale);

        store.sweep_expired();

        assert!(store.artifacts.contains_key("fresh"));
        assert!(!store.artifacts.contains_key("stale"));
        assert_eq!(store.total_bytes, 10);
    }
