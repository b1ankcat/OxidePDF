use criterion::{Criterion, criterion_group, criterion_main};
use lopdf::dictionary;
use std::fs;

fn fixture_pdf() -> Vec<u8> {
    let mut document = lopdf::Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let catalog_id = document.new_object_id();
    let mut kids = Vec::new();
    for page_number in 1..=4 {
        let page_id = document.new_object_id();
        let content_id = document.add_object(lopdf::Stream::new(
            lopdf::Dictionary::new(),
            format!("BT 72 720 Td (page {page_number}) Tj ET").into_bytes(),
        ));
        document.objects.insert(
            page_id,
            lopdf::Object::Dictionary(lopdf::dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => lopdf::Object::Array(vec![0.into(), 0.into(), 595.into(), 842.into()]),
                "Contents" => content_id,
            }),
        );
        kids.push(page_id.into());
    }
    document.objects.insert(
        pages_id,
        lopdf::Object::Dictionary(lopdf::dictionary! {
            "Type" => "Pages",
            "Kids" => lopdf::Object::Array(kids),
            "Count" => 4,
        }),
    );
    document.objects.insert(
        catalog_id,
        lopdf::Object::Dictionary(lopdf::dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        }),
    );
    document.trailer.set("Root", catalog_id);
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

fn bench_dir(name: &str) -> std::path::PathBuf {
    let dir =
        std::env::temp_dir().join(format!("oxidepdf_cli_bench_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn benches(c: &mut Criterion) {
    c.bench_function("pdf_edit_rotate_command_path", |b| {
        let dir = bench_dir("pdf_edit_rotate");
        let input = dir.join("input.pdf");
        let output = dir.join("output.pdf");
        fs::write(&input, fixture_pdf()).unwrap();
        b.iter(|| {
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let code = oxidepdf_cli::run_with_io(
                [
                    "oxidepdf",
                    "pdf_edit",
                    "rotate-pages",
                    input.to_str().unwrap(),
                    "--pages",
                    "1-4",
                    "--degrees",
                    "90",
                    "-o",
                    output.to_str().unwrap(),
                    "--force",
                ],
                [],
                &mut stdout,
                &mut stderr,
            );
            assert_eq!(code, 0, "{}", String::from_utf8_lossy(&stderr));
        });
    });

    c.bench_function("run_workflow_path", |b| {
        let dir = bench_dir("run_workflow");
        let input = dir.join("input.pdf");
        let output = dir.join("output.pdf");
        let workflow = dir.join("workflow.yaml");
        fs::write(&input, fixture_pdf()).unwrap();
        fs::write(
            &workflow,
            format!(
                r#"
version: 1
inputs:
  - id: source
    path: {}
tasks:
  - id: rotate
    op:
      pdf_edit:
        rotate_pages:
          pages: "1-4"
          degrees: 90
    inputs: [source]
outputs:
  - id: final
    from: rotate
    path: {}
"#,
                input.display(),
                output.display()
            ),
        )
        .unwrap();
        b.iter(|| {
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let code = oxidepdf_cli::run_with_io(
                [
                    "oxidepdf",
                    "run",
                    "--workflow",
                    workflow.to_str().unwrap(),
                    "--force",
                ],
                [],
                &mut stdout,
                &mut stderr,
            );
            assert_eq!(code, 0, "{}", String::from_utf8_lossy(&stderr));
        });
    });
}

criterion_group! {
    name = cli_paths;
    config = Criterion::default().sample_size(10);
    targets = benches
}
criterion_main!(cli_paths);
