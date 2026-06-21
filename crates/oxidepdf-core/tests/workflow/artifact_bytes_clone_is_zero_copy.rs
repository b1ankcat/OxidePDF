
#[tokio::test]
async fn independent_tasks_in_a_layer_run_in_parallel() {
    // Eight independent tasks each sleep 50ms. Run serially that is 400ms; in
    // parallel it should finish in well under that. Use a generous bound to stay
    // robust on busy CI while still proving concurrency.
    let tasks = (0..8)
        .map(|i| {
            format!(
                r#"{{ "id": "t{i}", "op": {{ "pdf_edit": {{ "merge": {{}} }} }}, "inputs": ["source"] }}"#
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let outputs = (0..8)
        .map(|i| format!(r#"{{ "id": "o{i}", "from": "t{i}", "path": "out{i}.bin" }}"#))
        .collect::<Vec<_>>()
        .join(",");
    let workflow = workflow_from_json(&format!(
        r#"
            {{
              "version": 1,
              "inputs": [{{ "id": "source", "path": "input.bin" }}],
              "tasks": [{tasks}],
              "outputs": [{outputs}]
            }}
            "#,
    ));
    let mut store = ArtifactStore::new();
    store.insert(artifact_ref("source"), Artifact::bytes(b"input").unwrap());

    #[derive(Clone)]
    struct SleepRunner;
    impl OperatorRunner for SleepRunner {
        fn run(&self, task: TaskSpec, _inputs: Vec<Artifact>) -> oxidepdf_core::OperatorFuture {
            Box::pin(async move {
                std::thread::sleep(std::time::Duration::from_millis(50));
                Artifact::bytes(task.id.as_str().as_bytes())
            })
        }
    }

    let started = std::time::Instant::now();
    let result = execute_workflow(&workflow, store, SleepRunner)
        .await
        .unwrap();
    let elapsed = started.elapsed();

    assert_eq!(result.plan.task_order.len(), 8);
    // Serial would be ~400ms; require comfortably below that.
    assert!(
        elapsed < std::time::Duration::from_millis(300),
        "expected parallel execution, took {elapsed:?}"
    );
}

#[tokio::test]
async fn downstream_task_starts_before_unrelated_layer_peer_finishes() {
    let workflow = workflow_from_json(
        r#"
            {
              "version": 1,
              "inputs": [{ "id": "source", "path": "input.bin" }],
              "tasks": [
                {
                  "id": "fast",
                  "op": { "pdf_edit": { "merge": {} } },
                  "inputs": ["source"]
                },
                {
                  "id": "slow_peer",
                  "op": { "pdf_edit": { "merge": {} } },
                  "inputs": ["source"]
                },
                {
                  "id": "after_fast",
                  "op": { "pdf_edit": { "merge": {} } },
                  "inputs": ["fast"]
                }
              ],
              "outputs": [
                { "id": "final", "from": "after_fast", "path": "out.bin" },
                { "id": "peer", "from": "slow_peer", "path": "peer.bin" }
              ]
            }
            "#,
    );
    let mut store = ArtifactStore::new();
    store.insert(artifact_ref("source"), Artifact::bytes(b"input").unwrap());

    #[derive(Clone, Default)]
    struct EventRunner(std::sync::Arc<std::sync::Mutex<Vec<(String, std::time::Instant)>>>);
    impl EventRunner {
        fn event_time(&self, name: &str) -> std::time::Instant {
            self.0
                .lock()
                .unwrap()
                .iter()
                .find_map(|(event, at)| (event == name).then_some(*at))
                .unwrap()
        }
    }
    impl OperatorRunner for EventRunner {
        fn run(&self, task: TaskSpec, _inputs: Vec<Artifact>) -> oxidepdf_core::OperatorFuture {
            let events = self.0.clone();
            Box::pin(async move {
                events.lock().unwrap().push((
                    format!("{}:start", task.id.as_str()),
                    std::time::Instant::now(),
                ));
                if task.id.as_str() == "slow_peer" {
                    std::thread::sleep(std::time::Duration::from_millis(80));
                }
                events.lock().unwrap().push((
                    format!("{}:end", task.id.as_str()),
                    std::time::Instant::now(),
                ));
                Artifact::bytes(task.id.as_str().as_bytes())
            })
        }
    }

    let runner = EventRunner::default();
    execute_workflow(&workflow, store, runner.clone())
        .await
        .unwrap();

    assert!(
        runner.event_time("after_fast:start") < runner.event_time("slow_peer:end"),
        "downstream task waited for unrelated peer, indicating layer-barrier execution"
    );
}

#[tokio::test]
async fn execute_workflow_enforces_total_input_size_limit() {
    let workflow = workflow_from_json(
        r#"
            {
              "version": 1,
              "inputs": [
                { "id": "first", "path": "a.bin" },
                { "id": "second", "path": "b.bin" }
              ],
              "tasks": [],
              "outputs": [{ "id": "final", "from": "first", "path": "out.bin" }],
              "limits": { "max_total_input_bytes": 9 }
            }
            "#,
    );
    let mut store = ArtifactStore::new();
    store.insert(artifact_ref("first"), Artifact::bytes(b"12345").unwrap());
    store.insert(artifact_ref("second"), Artifact::bytes(b"67890").unwrap());
    let runner = RecordingRunner::default();

    let err = execute_workflow(&workflow, store, runner.clone())
        .await
        .unwrap_err();

    assert_eq!(
        err,
        OxideError::ResourceLimitExceeded {
            limit: "max_total_input_bytes".to_owned()
        }
    );
    assert!(runner.executed().is_empty());
}

#[test]
fn artifact_bytes_clone_is_zero_copy() {
    let payload = vec![7u8; 4096];
    let original = ArtifactBytes::from_vec(payload).unwrap();
    let cloned = original.clone();

    // Cloning an ArtifactBytes must share the same backing allocation rather
    // than copying the bytes, so the two views point at the same address.
    assert_eq!(original.as_ptr(), cloned.as_ptr());
    assert_eq!(original.as_slice(), cloned.as_slice());
}

#[test]
fn from_arc_is_zero_copy() {
    let arc: std::sync::Arc<[u8]> = std::sync::Arc::from(vec![5u8; 4096].into_boxed_slice());
    let arc_ptr = arc.as_ptr();

    let bytes = ArtifactBytes::from_arc(arc);

    // Adopting an existing Arc must not reallocate or copy; the payload points
    // at the same address the Arc already owned.
    assert_eq!(bytes.as_ptr(), arc_ptr);
    assert!(!bytes.is_spilled());
    assert_eq!(bytes.len(), 4096);
}

#[test]
fn from_arc_clone_shares_buffer() {
    let arc: std::sync::Arc<[u8]> = std::sync::Arc::from(vec![1u8; 1024].into_boxed_slice());
    let bytes = ArtifactBytes::from_arc(arc);
    let cloned = bytes.clone();

    assert_eq!(bytes.as_ptr(), cloned.as_ptr());
    assert_eq!(bytes.as_slice(), cloned.as_slice());
}

#[test]
fn artifact_clone_shares_pdf_payload() {
    let artifact = Artifact::pdf(vec![1u8; 1024]).unwrap();
    let cloned = artifact.clone();

    let (Artifact::Pdf(original), Artifact::Pdf(copy)) = (&artifact, &cloned) else {
        panic!("expected PDF artifacts");
    };
    assert_eq!(original.bytes.as_ptr(), copy.bytes.as_ptr());
}

#[test]
fn small_artifact_stays_inline() {
    let bytes = ArtifactBytes::from_vec(vec![9u8; 1024]).unwrap();
    assert!(!bytes.is_spilled());
    assert_eq!(bytes.len(), 1024);
}

#[test]
fn large_artifact_spills_to_disk_and_reads_back() {
    // Exceed the 64 MiB spill threshold by a little; the payload must move to a
    // memory-mapped temp file yet still expose identical bytes.
    let size = 64 * 1024 * 1024 + 4096;
    let mut payload = vec![0u8; size];
    payload[0] = 1;
    payload[size - 1] = 2;
    let bytes = ArtifactBytes::from_vec(payload).unwrap();

    assert!(bytes.is_spilled());
    assert_eq!(bytes.len(), size);
    assert_eq!(bytes.as_slice()[0], 1);
    assert_eq!(bytes.as_slice()[size - 1], 2);

    // Cloning a spilled payload shares the mapping; both views read the same data.
    let cloned = bytes.clone();
    assert_eq!(cloned.as_slice(), bytes.as_slice());
}

#[test]
fn spilled_payload_roundtrip_is_byte_exact() {
    // A spilled payload must read back byte-for-byte, not just at the endpoints.
    // Fill with a position-dependent pattern so any truncation, offset, or
    // partial write would surface.
    let size = 64 * 1024 * 1024 + 7919;
    let mut payload = vec![0u8; size];
    for (index, byte) in payload.iter_mut().enumerate() {
        *byte = (index % 251) as u8;
    }
    let bytes = ArtifactBytes::from_vec(payload.clone()).unwrap();

    assert!(bytes.is_spilled());
    assert_eq!(bytes.as_slice(), payload.as_slice());
}
