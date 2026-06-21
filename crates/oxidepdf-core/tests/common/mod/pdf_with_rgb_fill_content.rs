
pub fn pdf_with_rgb_fill_content() -> Vec<u8> {
    let mut document = lopdf::Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let page_id = document.new_object_id();
    let content_id = document.new_object_id();
    let catalog_id = document.new_object_id();
    let content = lopdf::content::Content {
        operations: vec![
            lopdf::content::Operation::new(
                "rg",
                vec![Object::Real(1.0), Object::Real(0.0), Object::Real(0.0)],
            ),
            lopdf::content::Operation::new(
                "re",
                vec![
                    Object::Integer(0),
                    Object::Integer(0),
                    Object::Integer(100),
                    Object::Integer(100),
                ],
            ),
            lopdf::content::Operation::new("f", Vec::new()),
        ],
    }
    .encode()
    .unwrap();
    document.objects.insert(
        content_id,
        Object::Stream(lopdf::Stream::new(lopdf::Dictionary::new(), content)),
    );
    document.objects.insert(
        page_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => Object::Array(vec![0.into(), 0.into(), 100.into(), 100.into()]),
            "Contents" => content_id,
        }),
    );
    document.objects.insert(
        pages_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Pages",
            "Kids" => Object::Array(vec![page_id.into()]),
            "Count" => 1,
        }),
    );
    document.objects.insert(
        catalog_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        }),
    );
    document.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn pdf_with_xfa_form() -> Vec<u8> {
    let mut document = lopdf::Document::load_mem(&pdf_with_text_form_field(false)).unwrap();
    let catalog = document.catalog().unwrap();
    let acroform_id = catalog.get(b"AcroForm").unwrap().as_reference().unwrap();
    document
        .get_object_mut(acroform_id)
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("XFA", Object::string_literal("xfa packet"));
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn page_resources(document: &lopdf::Document, page_number: u32) -> Dictionary {
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    let resources = document
        .get_dictionary(page_id)
        .unwrap()
        .get(b"Resources")
        .unwrap();
    match resources {
        Object::Dictionary(dictionary) => dictionary.clone(),
        Object::Reference(id) => document.get_dictionary(*id).unwrap().clone(),
        other => panic!("unexpected resources object: {other:?}"),
    }
}

pub fn page_content_contains_operator(
    document: &lopdf::Document,
    page_number: u32,
    operator: &str,
) -> bool {
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    document
        .get_page_contents(page_id)
        .into_iter()
        .filter_map(|content_id| document.get_object(content_id).ok())
        .filter_map(|object| object.as_stream().ok())
        .filter_map(|stream| lopdf::content::Content::decode(&stream.content).ok())
        .flat_map(|content| content.operations)
        .any(|operation| operation.operator == operator)
}

pub fn page_xobject_subtypes(document: &lopdf::Document, page_number: u32) -> Vec<Vec<u8>> {
    let resources = page_resources(document, page_number);
    let Ok(xobjects) = resources.get(b"XObject").and_then(Object::as_dict) else {
        return Vec::new();
    };
    xobjects
        .iter()
        .filter_map(|(_, object)| object.as_reference().ok())
        .filter_map(|id| document.get_object(id).ok())
        .filter_map(|object| object.as_stream().ok())
        .filter_map(|stream| stream.dict.get(b"Subtype").and_then(Object::as_name).ok())
        .map(|name| name.to_vec())
        .collect()
}

pub fn page_xobject_count(document: &lopdf::Document, page_number: u32) -> usize {
    let resources = page_resources(document, page_number);
    resources
        .get(b"XObject")
        .and_then(Object::as_dict)
        .map(|dictionary| dictionary.len())
        .unwrap_or(0)
}

pub fn page_content_text_contains(
    document: &lopdf::Document,
    page_number: u32,
    expected: &str,
) -> bool {
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    String::from_utf8_lossy(&document.get_page_content(page_id).unwrap()).contains(expected)
}

pub fn page_form_xobject_operators(document: &lopdf::Document, page_number: u32) -> Vec<String> {
    let resources = page_resources(document, page_number);
    let Ok(xobjects) = resources.get(b"XObject").and_then(Object::as_dict) else {
        return Vec::new();
    };
    let mut operators = Vec::new();
    let mut seen = BTreeSet::new();
    for (_, object) in xobjects.iter() {
        if let Ok(id) = object.as_reference() {
            collect_form_xobject_operators(document, id, &mut seen, &mut operators);
        }
    }
    operators
}

fn collect_form_xobject_operators(
    document: &lopdf::Document,
    object_id: lopdf::ObjectId,
    seen: &mut BTreeSet<lopdf::ObjectId>,
    operators: &mut Vec<String>,
) {
    if !seen.insert(object_id) {
        return;
    }
    let Ok(stream) = document
        .get_object(object_id)
        .and_then(lopdf::Object::as_stream)
    else {
        return;
    };
    if stream
        .dict
        .get(b"Subtype")
        .and_then(lopdf::Object::as_name)
        .ok()
        != Some(b"Form".as_slice())
    {
        return;
    }
    if let Ok(content) = stream.get_plain_content()
        && let Ok(content) = lopdf::content::Content::decode(&content)
    {
        operators.extend(
            content
                .operations
                .into_iter()
                .map(|operation| operation.operator),
        );
    }
    let Ok(resources) = stream.dict.get(b"Resources").and_then(Object::as_dict) else {
        return;
    };
    let Ok(xobjects) = resources.get(b"XObject").and_then(Object::as_dict) else {
        return;
    };
    for (_, object) in xobjects.iter() {
        if let Ok(id) = object.as_reference() {
            collect_form_xobject_operators(document, id, seen, operators);
        }
    }
}

pub fn simple_svg() -> &'static [u8] {
    br##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80">
            <rect x="10" y="10" width="100" height="60" fill="#16a34a"/>
        </svg>"##
}

#[derive(Clone)]
pub struct RecordingRunner {
    executed: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    fail_on: Option<&'static str>,
    error: std::sync::Arc<std::sync::Mutex<Option<OxideError>>>,
}

impl RecordingRunner {
    pub fn with_failure(fail_on: &'static str, error: OxideError) -> Self {
        Self {
            executed: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            fail_on: Some(fail_on),
            error: std::sync::Arc::new(std::sync::Mutex::new(Some(error))),
        }
    }

    pub fn executed(&self) -> Vec<String> {
        self.executed.lock().unwrap().clone()
    }
}

impl Default for RecordingRunner {
    fn default() -> Self {
        Self {
            executed: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            fail_on: None,
            error: std::sync::Arc::new(std::sync::Mutex::new(None)),
        }
    }
}

impl OperatorRunner for RecordingRunner {
    fn run(&self, task: TaskSpec, _inputs: Vec<Artifact>) -> oxidepdf_core::OperatorFuture {
        let runner = self.clone();
        Box::pin(async move {
            runner
                .executed
                .lock()
                .unwrap()
                .push(task.id.as_str().to_owned());
            if runner.fail_on == Some(task.id.as_str()) {
                return Err(runner.error.lock().unwrap().take().unwrap());
            }

            Artifact::bytes(task.id.as_str().as_bytes())
        })
    }
}

#[derive(Clone)]
pub struct SlowRunner;

impl OperatorRunner for SlowRunner {
    fn run(&self, _task: TaskSpec, _inputs: Vec<Artifact>) -> oxidepdf_core::OperatorFuture {
        Box::pin(async move {
            std::thread::sleep(std::time::Duration::from_millis(5));
            Ok(Artifact::bytes(b"finished").unwrap())
        })
    }
}
