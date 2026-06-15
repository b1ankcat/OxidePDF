use criterion::{Criterion, criterion_group, criterion_main};
use lopdf::dictionary;
use oxidepdf_core::{
    Artifact, ArtifactBytes, ArtifactRef, OperatorSpec, PdfEditOptions, PdfOperatorRunner,
    ResourceLimits, RotateOptions, TaskId, TaskSpec, Workflow, WorkflowMetadata, WorkflowVersion,
    execute_workflow,
};

fn fixture_pdf() -> Vec<u8> {
    let mut document = lopdf::Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let catalog_id = document.new_object_id();
    let font_id = document.add_object(lopdf::dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });
    let mut kids = Vec::new();
    for page_number in 1..=6 {
        let page_id = document.new_object_id();
        let content_id = document.add_object(lopdf::Stream::new(
            lopdf::Dictionary::new(),
            format!("BT /F1 12 Tf 72 720 Td (page {page_number}) Tj ET").into_bytes(),
        ));
        document.objects.insert(
            page_id,
            lopdf::Object::Dictionary(lopdf::dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => lopdf::Object::Array(vec![0.into(), 0.into(), 595.into(), 842.into()]),
                "Resources" => lopdf::Object::Dictionary(lopdf::dictionary! {
                    "Font" => lopdf::Object::Dictionary(lopdf::dictionary! {
                        "F1" => font_id,
                    }),
                }),
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
            "Count" => 6,
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

fn input_ref() -> ArtifactRef {
    ArtifactRef::new("source")
}

fn workflow_with_tasks(tasks: Vec<TaskSpec>) -> Workflow {
    Workflow {
        version: WorkflowVersion::V1,
        inputs: vec![oxidepdf_core::InputSpec {
            id: input_ref(),
            path: "input.pdf".into(),
        }],
        tasks,
        outputs: vec![oxidepdf_core::OutputSpec {
            id: ArtifactRef::new("final"),
            from: ArtifactRef::new("rotate3"),
            path: "out.pdf".into(),
        }],
        limits: ResourceLimits::default(),
        metadata: WorkflowMetadata::default(),
    }
}

fn rotate_task(id: &str, input: &str, degrees: i16) -> TaskSpec {
    TaskSpec {
        id: TaskId::new(id),
        op: OperatorSpec::PdfEdit(PdfEditOptions::RotatePages(RotateOptions {
            pages: "1-6".to_owned(),
            degrees,
        })),
        inputs: vec![ArtifactRef::new(input)],
    }
}

fn benches(c: &mut Criterion) {
    let pdf = fixture_pdf();
    let runner = PdfOperatorRunner::default();

    c.bench_function("workflow_edit_chain", |b| {
        let workflow = workflow_with_tasks(vec![
            rotate_task("rotate1", "source", 90),
            rotate_task("rotate2", "rotate1", 180),
            rotate_task("rotate3", "rotate2", 270),
        ]);
        b.iter(|| {
            let mut store = oxidepdf_core::ArtifactStore::new();
            store.insert(input_ref(), Artifact::pdf(&pdf).unwrap());
            execute_workflow(&workflow, store, &runner).unwrap()
        });
    });

    c.bench_function("merge_rotate_keep_pages", |b| {
        b.iter(|| {
            let merged = oxidepdf_core::merge_pdf_artifacts(&[
                Artifact::pdf(&pdf).unwrap(),
                Artifact::pdf(&pdf).unwrap(),
            ])
            .unwrap();
            let rotated = oxidepdf_core::rotate_pdf(&merged.bytes, "1-3", 90).unwrap();
            oxidepdf_core::split_pdf(&rotated.bytes, "1-4").unwrap()
        });
    });

    c.bench_function("large_artifact_spill", |b| {
        b.iter(|| ArtifactBytes::from_vec(vec![7u8; 64 * 1024 * 1024 + 4096]).unwrap());
    });

    c.bench_function("object_output_materialization", |b| {
        let document = lopdf::Document::load_mem(&pdf).unwrap();
        let artifact = Artifact::pdf_object(document);
        b.iter(|| artifact.output_bytes().unwrap().into_owned());
    });
}

criterion_group! {
    name = core_hot_paths;
    config = Criterion::default().sample_size(10);
    targets = benches
}
criterion_main!(core_hot_paths);
