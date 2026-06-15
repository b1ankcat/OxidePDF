use lopdf::dictionary;
use std::fs;
use std::hint::black_box;
use std::time::Instant;

const ITERS: usize = 10;

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

fn bench(name: &str, mut run: impl FnMut()) {
    let started = Instant::now();
    for _ in 0..ITERS {
        run();
    }
    let elapsed = started.elapsed();
    let per_iter = elapsed / ITERS as u32;
    println!("{name}: {per_iter:?}/iter over {ITERS} iterations");
}

fn run_cli<'a>(runtime: &tokio::runtime::Runtime, args: impl IntoIterator<Item = &'a str>) {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = runtime.block_on(oxidepdf_cli::run_with_io(
        args,
        [],
        &mut stdout,
        &mut stderr,
    ));
    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&stderr));
    black_box(stdout);
}

fn main() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let rotate_dir = bench_dir("pdf_edit_rotate");
    let rotate_input = rotate_dir.join("input.pdf");
    let rotate_output = rotate_dir.join("output.pdf");
    fs::write(&rotate_input, fixture_pdf()).unwrap();
    bench("pdf_edit_rotate_command_path", || {
        run_cli(
            &runtime,
            [
                "oxidepdf",
                "pdf_edit",
                "rotate-pages",
                rotate_input.to_str().unwrap(),
                "--pages",
                "1-4",
                "--degrees",
                "90",
                "-o",
                rotate_output.to_str().unwrap(),
                "--force",
            ],
        );
    });

    let workflow_dir = bench_dir("run_workflow");
    let workflow_input = workflow_dir.join("input.pdf");
    let workflow_output = workflow_dir.join("output.pdf");
    let workflow = workflow_dir.join("workflow.yaml");
    fs::write(&workflow_input, fixture_pdf()).unwrap();
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
            workflow_input.display(),
            workflow_output.display()
        ),
    )
    .unwrap();
    bench("run_workflow_path", || {
        run_cli(
            &runtime,
            [
                "oxidepdf",
                "run",
                "--workflow",
                workflow.to_str().unwrap(),
                "--force",
            ],
        );
    });
}
