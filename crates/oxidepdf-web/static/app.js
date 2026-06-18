let schema = [];
let selFamily = null, selOp = null, selMeta = null;
let mode = 'single';            // 'single' | 'workflow'
let wfSteps = [];               // [{family,op,options,inputs:[{kind,..}]}]
let currentRead = null;
let lastResultId = null;
let lang = localStorage.getItem('oxidepdf.lang') || 'en';
let formWorkflowMode = false;

// File pool: {id, name, virtual, stepIndex, selected}. Virtual entries are
// placeholder outputs of earlier workflow steps (cannot be previewed/deleted).
let pool = [];
let dragFromIdx = null;          // drag-to-reorder source index

const $ = id => document.getElementById(id);
const i18n = {
  en: {
    family: 'Family',
    workflowMode: 'Workflow',
    operation: 'Operation',
    workflow: 'Workflow',
    addStep: 'Add step',
    executeWorkflow: 'Execute Workflow',
    upload: 'Upload',
    files: 'Files',
    downloadOutput: 'Download output',
    execute: 'Execute',
    addToWorkflow: 'Add to workflow',
    emptyTitle: 'Drop files here or use Upload',
    emptyCopy: 'PDF, PNG, JPEG, and SVG files appear here after selection.',
    addStepTitle: 'Add a step',
    cancel: 'Cancel',
    noFiles: 'No files. Upload to begin.',
    dragToReorder: 'Drag to reorder',
    requiresMultiple: 'Requires multiple inputs',
    inputNode: 'input',
    outputNode: 'output',
    stepOutput: index => `output of step ${index}`,
    selectInputsForStep: 'Select input file(s) for this step first',
    selectInputs: 'Select input file(s)',
    selectOperation: 'Select an operation',
    running: 'Running...',
    noParameters: 'This operation has no parameters.',
    required: 'required',
    optional: 'optional',
    none: 'none',
    defaultValue: 'default',
    commaSeparated: 'comma,separated',
    addAtLeastOneStep: 'Add at least one step',
    uploadFailed: 'Upload failed',
    previewFailed: 'Preview failed',
    schemaFailed: 'Failed to load operation schema',
    families: {
      PdfEdit: 'PDF Edit',
      PdfInspect: 'PDF Inspect',
      PdfSecurity: 'PDF Security',
      PdfCompare: 'PDF Compare',
      PdfSign: 'PDF Sign',
    },
    ops: {
      Merge: 'Merge',
      KeepPages: 'Keep pages',
      ExtractPages: 'Extract pages',
      ReorderPages: 'Reorder pages',
      RotatePages: 'Rotate pages',
      DeletePages: 'Delete pages',
      DeleteBlankPages: 'Delete blank pages',
      CropPages: 'Crop pages',
      ScalePages: 'Scale pages',
      SinglePage: 'Single page',
      NUp: 'N-up',
      Booklet: 'Booklet',
      PageNumbers: 'Page numbers',
      ImageToPdf: 'Image to PDF',
      SvgToPdf: 'SVG to PDF',
      Watermark: 'Watermark',
      Overlay: 'Overlay',
      ImageEdit: 'Image edit',
      Color: 'Color',
      Metadata: 'Metadata',
      Outline: 'Outline',
      Attachment: 'Attachment',
      Annotation: 'Annotation',
      FormFill: 'Fill form',
      FormUnlockReadonly: 'Unlock read-only form',
      FormRemove: 'Remove form',
      InteractiveRemove: 'Remove interactive content',
      Compression: 'Compression',
      Render: 'Render',
      ExtractText: 'Extract text',
      Attachments: 'Attachments',
      AttachmentExtract: 'Extract attachment',
      Annotations: 'Annotations',
      Forms: 'Forms',
      Images: 'Images',
      ImageExtract: 'Extract image',
      Encrypt: 'Encrypt',
      Decrypt: 'Decrypt',
      PermissionsGet: 'Get permissions',
      PermissionsSet: 'Set permissions',
      Report: 'Report',
      VisualDiff: 'Visual diff',
      List: 'List',
      Verify: 'Verify',
      DeleteField: 'Delete field',
    },
    fields: {
      accessibility: 'Accessibility',
      action: 'Action',
      actions: 'Actions',
      algorithm: 'Algorithm',
      annotate: 'Annotate',
      annotations: 'Annotations',
      assemble: 'Assemble',
      bottom: 'Bottom',
      children: 'Children',
      columns: 'Columns',
      copy: 'Copy',
      degrees: 'Degrees',
      description: 'Description',
      destructive: 'Destructive',
      embedded_files: 'Embedded files',
      entries: 'Entries',
      factor: 'Factor',
      field_name: 'Field name',
      fields: 'Fields',
      fill_forms: 'Fill forms',
      font: 'Font',
      font_size: 'Font size',
      format: 'Format',
      forms: 'Forms',
      from: 'From',
      high_quality_print: 'High quality print',
      id: 'ID',
      images: 'Images',
      include_text: 'Include text',
      items: 'Items',
      javascript: 'JavaScript',
      key: 'Key',
      keys: 'Keys',
      kind: 'Kind',
      layout: 'Layout',
      left: 'Left',
      max_height: 'Max height',
      max_width: 'Max width',
      mode: 'Mode',
      modify: 'Modify',
      name: 'Name',
      opacity: 'Opacity',
      owner_password: 'Owner password',
      page: 'Page',
      pages: 'Pages',
      password: 'Password',
      permissions: 'Permissions',
      position: 'Position',
      prefix: 'Prefix',
      print: 'Print',
      quality: 'Quality',
      rasterize: 'Rasterize',
      rasterize_pages: 'Rasterize pages',
      right: 'Right',
      rotation: 'Rotation',
      rows: 'Rows',
      scale: 'Scale',
      source_page: 'Source page',
      start: 'Start',
      suffix: 'Suffix',
      text: 'Text',
      text_max_chars: 'Text max chars',
      title: 'Title',
      to: 'To',
      top: 'Top',
      tree: 'Tree',
      user_password: 'User password',
      value: 'Value',
    },
    descriptions: {
      'AES-128 Standard Security Handler, revision 4.': 'AES-128 Standard Security Handler, revision 4.',
      'AES-256 Standard Security Handler, revision 6.': 'AES-256 Standard Security Handler, revision 6.',
      'Allow deleting a field that contains signature value material.': 'Allow deleting a field that contains signature value material.',
      'Bottom coordinate of the new CropBox.': 'Bottom coordinate of the new CropBox.',
      'Bottom-center edge.': 'Bottom-center edge.',
      'Bottom-left corner.': 'Bottom-left corner.',
      'Bottom-right corner.': 'Bottom-right corner.',
      'Color content stream edit action.': 'Color content stream edit action.',
      'Color content stream edit options.': 'Color content stream edit options.',
      'Existing watermark semantics.': 'Existing watermark semantics.',
      'Explicit document permission policy.': 'Explicit document permission policy.',
      'Explicit page sequence, for example `3,1,2`.': 'Explicit page sequence, for example `3,1,2`.',
      'First number written on the first selected page.': 'First number written on the first selected page.',
      'Font family name discovered via fontdb.': 'Font family name discovered via fontdb.',
      'Font size in PDF points.': 'Font size in PDF points.',
      'Image extraction options.': 'Image extraction options.',
      'Image overlay.': 'Image overlay.',
      'Image resource edit action.': 'Image resource edit action.',
      'Image resource edit options.': 'Image resource edit options.',
      'Image resource inspection options.': 'Image resource inspection options.',
      'Image watermark.': 'Image watermark.',
      'Include extractable text summaries in the comparison.': 'Include extractable text summaries in the comparison.',
      'Layout mode such as `fit`, `fill`, or `original_size`.': 'Layout mode such as `fit`, `fill`, or `original_size`.',
      'Left coordinate of the new CropBox.': 'Left coordinate of the new CropBox.',
      'Legacy RC4 Standard Security Handler. Explicitly unsupported until fully tested.': 'Legacy RC4 Standard Security Handler. Explicitly unsupported until fully tested.',
      'List PDF signatures without performing trust validation.': 'List PDF signatures without performing trust validation.',
      'Maximum text characters retained per side in summaries.': 'Maximum text characters retained per side in summaries.',
      'Number of columns on each output page.': 'Number of columns on each output page.',
      'Number of rows on each output page.': 'Number of rows on each output page.',
      'One-based page number to render from both documents.': 'One-based page number to render from both documents.',
      'One-based page number.': 'One-based page number.',
      'One-based source page for PDF page overlays.': 'One-based source page for PDF page overlays.',
      'Opacity from 0.0 to 1.0.': 'Opacity from 0.0 to 1.0.',
      'Optional output format such as `png`.': 'Optional output format such as `png`.',
      'Optional render scale.': 'Optional render scale.',
      'Options for N-up page layout.': 'Options for N-up page layout.',
      'Options for SVG-to-PDF conversion.': 'Options for SVG-to-PDF conversion.',
      'Options for adding page numbers.': 'Options for adding page numbers.',
      'Options for booklet imposition.': 'Options for booklet imposition.',
      'Options for combining pages into one tall page.': 'Options for combining pages into one tall page.',
      'Options for cropping pages.': 'Options for cropping pages.',
      'Options for decrypting a PDF.': 'Options for decrypting a PDF.',
      'Options for deleting a signature field.': 'Options for deleting a signature field.',
      'Options for deleting structurally blank pages.': 'Options for deleting structurally blank pages.',
      'Options for encrypting a PDF.': 'Options for encrypting a PDF.',
      'Options for image-to-PDF conversion.': 'Options for image-to-PDF conversion.',
      'Options for inspecting permissions.': 'Options for inspecting permissions.',
      'Options for merge.': 'Options for merge.',
      'Options for page-selection edits.': 'Options for page-selection edits.',
      'Options for rendered page visual differences.': 'Options for rendered page visual differences.',
      'Options for rendering.': 'Options for rendering.',
      'Options for reorder.': 'Options for reorder.',
      'Options for replacing permissions.': 'Options for replacing permissions.',
      'Options for rotate.': 'Options for rotate.',
      'Options for scaling pages.': 'Options for scaling pages.',
      'Options for signature and certificate operations.': 'Options for signature and certificate operations.',
      'Options for split.': 'Options for split.',
      'Options for structured PDF comparison reports.': 'Options for structured PDF comparison reports.',
      'Options for text extraction.': 'Options for text extraction.',
      'Options for the unified overlay engine.': 'Options for the unified overlay engine.',
      'Options for watermarking.': 'Options for watermarking.',
      'Output format, initially `plain`.': 'Output format, initially `plain`.',
      'Overlay a page from a second PDF.': 'Overlay a page from a second PDF.',
      'Overlay kind.': 'Overlay kind.',
      'Page number placement.': 'Page number placement.',
      'Page range, for example `1,3-5`.': 'Page range, for example `1,3-5`.',
      'Page range, for example `1,3-5`. Defaults to all pages.': 'Page range, for example `1,3-5`. Defaults to all pages.',
      'Position such as `center`.': 'Position such as `center`.',
      'Rasterize SVG before overlaying. Defaults to vector output when false.': 'Rasterize SVG before overlaying. Defaults to vector output when false.',
      'Rasterize SVG before watermarking. Defaults to vector output when false.': 'Rasterize SVG before watermarking. Defaults to vector output when false.',
      'Render scale applied to both documents.': 'Render scale applied to both documents.',
      'Requested signature operation.': 'Requested signature operation.',
      'Right coordinate of the new CropBox.': 'Right coordinate of the new CropBox.',
      'Rotation in degrees.': 'Rotation in degrees.',
      'Rotation in degrees. Validation happens in the workflow validator.': 'Rotation in degrees. Validation happens in the workflow validator.',
      'SVG overlay.': 'SVG overlay.',
      'SVG watermark.': 'SVG watermark.',
      'Scale factor applied to page boxes and page contents.': 'Scale factor applied to page boxes and page contents.',
      'Scale for image and SVG watermarks.': 'Scale for image and SVG watermarks.',
      'Scale for image, SVG, and PDF page overlays.': 'Scale for image, SVG, and PDF page overlays.',
      'Signature field name to delete.': 'Signature field name to delete.',
      'Stamp text or image appearance.': 'Stamp text or image appearance.',
      'Supported encryption algorithms for newly written PDFs.': 'Supported encryption algorithms for newly written PDFs.',
      'Text after the number.': 'Text after the number.',
      'Text before the number.': 'Text before the number.',
      'Text for text watermarks.': 'Text for text watermarks.',
      'Text for text, stamp, signature appearance, or watermark overlays.': 'Text for text, stamp, signature appearance, or watermark overlays.',
      'Text overlay.': 'Text overlay.',
      'Text watermark.': 'Text watermark.',
      'Top coordinate of the new CropBox.': 'Top coordinate of the new CropBox.',
      'Top-center edge.': 'Top-center edge.',
      'Top-left corner.': 'Top-left corner.',
      'Top-right corner.': 'Top-right corner.',
      'Unified overlay content kind.': 'Unified overlay content kind.',
      'User-selected rasterization mode. Defaults to vector output when false.': 'User-selected rasterization mode. Defaults to vector output when false.',
      'Verify PDF signatures and embedded certificate material.': 'Verify PDF signatures and embedded certificate material.',
      'Visual signature appearance only; does not create a digital signature.': 'Visual signature appearance only; does not create a digital signature.',
      'Watermark content kind.': 'Watermark content kind.',
      'Watermark kind.': 'Watermark kind.',
    },
  },
  zh: {
    family: '分类',
    workflowMode: '工作流',
    operation: '操作',
    workflow: '工作流',
    addStep: '添加步骤',
    executeWorkflow: '执行工作流',
    upload: '上传',
    files: '文件',
    downloadOutput: '下载输出',
    execute: '执行',
    addToWorkflow: '加入工作流',
    emptyTitle: '拖拽文件到这里，或点击上传',
    emptyCopy: '选择后可预览 PDF、PNG、JPEG 和 SVG 文件。',
    addStepTitle: '添加步骤',
    cancel: '取消',
    noFiles: '暂无文件。请先上传。',
    dragToReorder: '拖拽排序',
    requiresMultiple: '需要多个输入',
    inputNode: '输入',
    outputNode: '输出',
    stepOutput: index => `第 ${index} 步输出`,
    selectInputsForStep: '请先为此步骤选择输入文件',
    selectInputs: '请选择输入文件',
    selectOperation: '请选择操作',
    running: '执行中...',
    noParameters: '此操作没有参数。',
    required: '必填',
    optional: '可选',
    none: '无',
    defaultValue: '默认',
    commaSeparated: '逗号分隔',
    addAtLeastOneStep: '请至少添加一个步骤',
    uploadFailed: '上传失败',
    previewFailed: '预览失败',
    schemaFailed: '无法加载操作 schema',
    families: {
      PdfEdit: 'PDF 编辑',
      PdfInspect: 'PDF 检查',
      PdfSecurity: 'PDF 安全',
      PdfCompare: 'PDF 对比',
      PdfSign: 'PDF 签名',
    },
    ops: {
      Merge: '合并',
      KeepPages: '保留页面',
      ExtractPages: '提取页面',
      ReorderPages: '重排页面',
      RotatePages: '旋转页面',
      DeletePages: '删除页面',
      DeleteBlankPages: '删除空白页',
      CropPages: '裁剪页面',
      ScalePages: '缩放页面',
      SinglePage: '合成长页',
      NUp: '多页合一',
      Booklet: '小册子',
      PageNumbers: '页码',
      ImageToPdf: '图片转 PDF',
      SvgToPdf: 'SVG 转 PDF',
      Watermark: '水印',
      Overlay: '叠加',
      ImageEdit: '图片编辑',
      Color: '颜色',
      Metadata: '元数据',
      Outline: '书签大纲',
      Attachment: '附件',
      Annotation: '注释',
      FormFill: '填写表单',
      FormUnlockReadonly: '解除只读表单',
      FormRemove: '移除表单',
      InteractiveRemove: '移除交互内容',
      Compression: '压缩',
      Render: '渲染',
      ExtractText: '提取文本',
      Attachments: '附件列表',
      AttachmentExtract: '提取附件',
      Annotations: '注释列表',
      Forms: '表单列表',
      Images: '图片列表',
      ImageExtract: '提取图片',
      Encrypt: '加密',
      Decrypt: '解密',
      PermissionsGet: '读取权限',
      PermissionsSet: '设置权限',
      Report: '报告',
      VisualDiff: '视觉差异',
      List: '列出',
      Verify: '验证',
      DeleteField: '删除字段',
    },
    fields: {
      accessibility: '无障碍访问',
      action: '动作',
      actions: '动作',
      algorithm: '算法',
      annotate: '注释权限',
      annotations: '注释',
      assemble: '页面组装',
      bottom: '下边界',
      children: '子项',
      columns: '列数',
      copy: '复制',
      degrees: '角度',
      description: '说明',
      destructive: '破坏性操作',
      embedded_files: '嵌入文件',
      entries: '条目',
      factor: '系数',
      field_name: '字段名',
      fields: '字段',
      fill_forms: '填写表单',
      font: '字体',
      font_size: '字号',
      format: '格式',
      forms: '表单',
      from: '原颜色',
      high_quality_print: '高质量打印',
      id: 'ID',
      images: '图片',
      include_text: '包含文本',
      items: '项目',
      javascript: 'JavaScript',
      key: '键',
      keys: '键列表',
      kind: '类型',
      layout: '布局',
      left: '左边界',
      max_height: '最大高度',
      max_width: '最大宽度',
      mode: '模式',
      modify: '修改',
      name: '名称',
      opacity: '不透明度',
      owner_password: '所有者密码',
      page: '页面',
      pages: '页面范围',
      password: '密码',
      permissions: '权限',
      position: '位置',
      prefix: '前缀',
      print: '打印',
      quality: '质量',
      rasterize: '栅格化',
      rasterize_pages: '栅格化页面',
      right: '右边界',
      rotation: '旋转',
      rows: '行数',
      scale: '缩放',
      source_page: '源页面',
      start: '起始编号',
      suffix: '后缀',
      text: '文本',
      text_max_chars: '文本最大字符数',
      title: '标题',
      to: '目标颜色',
      top: '上边界',
      tree: '大纲树',
      user_password: '用户密码',
      value: '值',
    },
    descriptions: {
      'AES-128 Standard Security Handler, revision 4.': 'AES-128 标准安全处理器，修订版 4。',
      'AES-256 Standard Security Handler, revision 6.': 'AES-256 标准安全处理器，修订版 6。',
      'Allow deleting a field that contains signature value material.': '允许删除包含签名值数据的字段。',
      'Bottom coordinate of the new CropBox.': '新 CropBox 的下边界坐标。',
      'Bottom-center edge.': '底部居中。',
      'Bottom-left corner.': '左下角。',
      'Bottom-right corner.': '右下角。',
      'Color content stream edit action.': '颜色内容流编辑动作。',
      'Color content stream edit options.': '颜色内容流编辑选项。',
      'Existing watermark semantics.': '使用现有水印语义。',
      'Explicit document permission policy.': '显式文档权限策略。',
      'Explicit page sequence, for example `3,1,2`.': '显式页面顺序，例如 `3,1,2`。',
      'First number written on the first selected page.': '写入第一个选中页面的起始页码。',
      'Font family name discovered via fontdb.': '通过 fontdb 查找的字体族名称。',
      'Font size in PDF points.': 'PDF 点数单位的字号。',
      'Image extraction options.': '图片提取选项。',
      'Image overlay.': '图片叠加。',
      'Image resource edit action.': '图片资源编辑动作。',
      'Image resource edit options.': '图片资源编辑选项。',
      'Image resource inspection options.': '图片资源检查选项。',
      'Image watermark.': '图片水印。',
      'Include extractable text summaries in the comparison.': '在对比中包含可提取文本摘要。',
      'Layout mode such as `fit`, `fill`, or `original_size`.': '布局模式，例如 `fit`、`fill` 或 `original_size`。',
      'Left coordinate of the new CropBox.': '新 CropBox 的左边界坐标。',
      'Legacy RC4 Standard Security Handler. Explicitly unsupported until fully tested.': '旧版 RC4 标准安全处理器；完整测试前明确不支持。',
      'List PDF signatures without performing trust validation.': '列出 PDF 签名，不执行信任验证。',
      'Maximum text characters retained per side in summaries.': '摘要中每一侧保留的最大文本字符数。',
      'Number of columns on each output page.': '每个输出页面上的列数。',
      'Number of rows on each output page.': '每个输出页面上的行数。',
      'One-based page number to render from both documents.': '从两个文档渲染的页码，从 1 开始。',
      'One-based page number.': '页码，从 1 开始。',
      'One-based source page for PDF page overlays.': 'PDF 页面叠加使用的源页码，从 1 开始。',
      'Opacity from 0.0 to 1.0.': '不透明度，范围 0.0 到 1.0。',
      'Optional output format such as `png`.': '可选输出格式，例如 `png`。',
      'Optional render scale.': '可选渲染缩放比例。',
      'Options for N-up page layout.': '多页合一布局选项。',
      'Options for SVG-to-PDF conversion.': 'SVG 转 PDF 选项。',
      'Options for adding page numbers.': '添加页码选项。',
      'Options for booklet imposition.': '小册子拼版选项。',
      'Options for combining pages into one tall page.': '将页面合成为一个长页的选项。',
      'Options for cropping pages.': '裁剪页面选项。',
      'Options for decrypting a PDF.': 'PDF 解密选项。',
      'Options for deleting a signature field.': '删除签名字段选项。',
      'Options for deleting structurally blank pages.': '删除结构性空白页选项。',
      'Options for encrypting a PDF.': 'PDF 加密选项。',
      'Options for image-to-PDF conversion.': '图片转 PDF 选项。',
      'Options for inspecting permissions.': '检查权限选项。',
      'Options for merge.': '合并选项。',
      'Options for page-selection edits.': '页面选择编辑选项。',
      'Options for rendered page visual differences.': '渲染页面视觉差异选项。',
      'Options for rendering.': '渲染选项。',
      'Options for reorder.': '重排选项。',
      'Options for replacing permissions.': '替换权限选项。',
      'Options for rotate.': '旋转选项。',
      'Options for scaling pages.': '缩放页面选项。',
      'Options for signature and certificate operations.': '签名和证书操作选项。',
      'Options for split.': '拆分选项。',
      'Options for structured PDF comparison reports.': '结构化 PDF 对比报告选项。',
      'Options for text extraction.': '文本提取选项。',
      'Options for the unified overlay engine.': '统一叠加引擎选项。',
      'Options for watermarking.': '水印选项。',
      'Output format, initially `plain`.': '输出格式，目前为 `plain`。',
      'Overlay a page from a second PDF.': '叠加第二个 PDF 中的页面。',
      'Overlay kind.': '叠加类型。',
      'Page number placement.': '页码位置。',
      'Page range, for example `1,3-5`.': '页面范围，例如 `1,3-5`。',
      'Page range, for example `1,3-5`. Defaults to all pages.': '页面范围，例如 `1,3-5`。默认所有页面。',
      'Position such as `center`.': '位置，例如 `center`。',
      'Rasterize SVG before overlaying. Defaults to vector output when false.': '叠加前栅格化 SVG；为 false 时默认输出矢量内容。',
      'Rasterize SVG before watermarking. Defaults to vector output when false.': '加水印前栅格化 SVG；为 false 时默认输出矢量内容。',
      'Render scale applied to both documents.': '应用到两个文档的渲染缩放比例。',
      'Requested signature operation.': '请求的签名操作。',
      'Right coordinate of the new CropBox.': '新 CropBox 的右边界坐标。',
      'Rotation in degrees.': '旋转角度。',
      'Rotation in degrees. Validation happens in the workflow validator.': '旋转角度。由工作流验证器进行校验。',
      'SVG overlay.': 'SVG 叠加。',
      'SVG watermark.': 'SVG 水印。',
      'Scale factor applied to page boxes and page contents.': '应用到页面边界框和页面内容的缩放系数。',
      'Scale for image and SVG watermarks.': '图片和 SVG 水印的缩放比例。',
      'Scale for image, SVG, and PDF page overlays.': '图片、SVG 和 PDF 页面叠加的缩放比例。',
      'Signature field name to delete.': '要删除的签名字段名称。',
      'Stamp text or image appearance.': '印章文本或图片外观。',
      'Supported encryption algorithms for newly written PDFs.': '新写入 PDF 支持的加密算法。',
      'Text after the number.': '页码后的文本。',
      'Text before the number.': '页码前的文本。',
      'Text for text watermarks.': '文本水印内容。',
      'Text for text, stamp, signature appearance, or watermark overlays.': '文本、印章、签名外观或水印叠加使用的文本。',
      'Text overlay.': '文本叠加。',
      'Text watermark.': '文本水印。',
      'Top coordinate of the new CropBox.': '新 CropBox 的上边界坐标。',
      'Top-center edge.': '顶部居中。',
      'Top-left corner.': '左上角。',
      'Top-right corner.': '右上角。',
      'Unified overlay content kind.': '统一叠加内容类型。',
      'User-selected rasterization mode. Defaults to vector output when false.': '用户选择的栅格化模式；为 false 时默认输出矢量内容。',
      'Verify PDF signatures and embedded certificate material.': '验证 PDF 签名和嵌入的证书材料。',
      'Visual signature appearance only; does not create a digital signature.': '仅生成可视签名外观；不会创建数字签名。',
      'Watermark content kind.': '水印内容类型。',
      'Watermark kind.': '水印类型。',
    },
  },
};

function t(key, ...args) {
  const value = i18n[lang][key];
  if (value === undefined) throw new Error(`Missing i18n key: ${key}`);
  return typeof value === 'function' ? value(...args) : value;
}

function labelFrom(kind, name) {
  const group = i18n[lang][kind];
  if (!group || group[name] === undefined) throw new Error(`Missing ${kind} label: ${name}`);
  return group[name];
}

function familyLabel(name) { return labelFrom('families', name); }
function opLabel(name) { return labelFrom('ops', name); }
function fieldLabel(name) { return labelFrom('fields', name); }

function fieldDescription(description) {
  const group = i18n[lang].descriptions;
  if (!group || group[description] === undefined) {
    throw new Error(`Missing field description label: ${description}`);
  }
  return group[description];
}

function apiUrl(path) {
  return `api/${path}`;
}

async function init() {
  if (!i18n[lang]) throw new Error(`Unsupported stored language: ${lang}`);
  applyLanguage();
  try {
    const r = await fetch(apiUrl('schema'));
    if (!r.ok) throw new Error(await r.text());
    schema = await r.json();
    renderFamilies();
  } catch (e) {
    showError(`${t('schemaFailed')}: ${e.message}`);
    throw e;
  }
  $('wf-toggle').onclick = toggleWorkflowMode;
  $('btn-add-step').onclick = openPicker;
  $('picker-close').onclick = () => $('op-picker').classList.remove('show');
  $('btn-single').onclick = execSingle;
  $('btn-exec-wf').onclick = execWorkflow;
  $('btn-add-to-wf').onclick = addStepFromForm;
  $('btn-upload').onclick = () => $('file-input').click();
  $('file-input').onchange = onUpload;
  $('file-dropdown-btn').onclick = toggleDropdown;
  setupDropZone();
  for (const btn of document.querySelectorAll('.lang-btn')) {
    btn.onclick = () => setLanguage(btn.dataset.lang);
  }
  document.addEventListener('click', e => {
    if (!$('file-select').contains(e.target)) $('file-list').classList.remove('show');
  });
}

function setLanguage(nextLang) {
  if (!i18n[nextLang]) throw new Error(`Unsupported language: ${nextLang}`);
  lang = nextLang;
  localStorage.setItem('oxidepdf.lang', lang);
  applyLanguage();
  renderFamilies();
  renderOps();
  renderFileList();
  renderChain();
  if (selMeta && currentRead) showOpForm(selFamily, selMeta, formWorkflowMode);
}

function applyLanguage() {
  document.documentElement.lang = lang === 'zh' ? 'zh-CN' : 'en';
  for (const el of document.querySelectorAll('[data-i18n]')) el.textContent = t(el.dataset.i18n);
  for (const btn of document.querySelectorAll('.lang-btn')) {
    btn.classList.toggle('active', btn.dataset.lang === lang);
    btn.setAttribute('aria-pressed', String(btn.dataset.lang === lang));
  }
}

function renderFamilies() {
  const el = $('families');
  el.replaceChildren();
  for (const f of schema) {
    const d = document.createElement('div');
    d.className = 'item' + (selFamily === f.name && mode === 'single' ? ' active' : '');
    d.textContent = familyLabel(f.name);
    d.title = f.name;
    d.onclick = () => { if (mode === 'workflow') return; selectFamily(f.name); };
    el.appendChild(d);
  }
}

function selectFamily(name) {
  selFamily = name; selOp = null;
  renderFamilies();
  renderOps();
}

function renderOps() {
  const el = $('ops');
  el.replaceChildren();
  const fam = schema.find(f => f.name === selFamily);
  if (!fam) {
    if (selFamily !== null) throw new Error(`Selected family not found: ${selFamily}`);
    return;
  }
  for (const op of fam.ops) {
    const d = document.createElement('div');
    d.className = 'item' + (selOp === op.name ? ' active' : '');
    d.textContent = opLabel(op.name);
    d.title = op.multi_input ? `${op.name} - ${t('requiresMultiple')}` : op.name;
    d.onclick = () => showOpForm(fam.name, op, false);
    el.appendChild(d);
  }
}

function showOpForm(family, op, wfMode) {
  selFamily = family; selOp = op.name; selMeta = op;
  formWorkflowMode = wfMode;
  if (mode === 'single') renderOps();
  $('op-panel').style.display = '';
  $('op-title').textContent = `${familyLabel(family)} > ${opLabel(op.name)}`;
  currentRead = renderForm(op.schema || {}, $('form-fields'), { t, fieldLabel, fieldDescription });
  $('btn-single').style.display = wfMode ? 'none' : '';
  $('btn-add-to-wf').style.display = wfMode ? '' : 'none';
  clearError();
}

function toggleWorkflowMode() {
  mode = mode === 'workflow' ? 'single' : 'workflow';
  $('c2-single').style.display = mode === 'workflow' ? 'none' : '';
  $('c2-workflow').style.display = mode === 'workflow' ? '' : 'none';
  $('wf-toggle').classList.toggle('active', mode === 'workflow');
  $('op-panel').style.display = 'none';
  selOp = null;
  selMeta = null;
  formWorkflowMode = false;
  // Drop virtual entries when leaving/entering workflow mode.
  pool = pool.filter(f => !f.virtual);
  wfSteps = [];
  renderFamilies();
  renderChain();
  renderFileList();
}

window.init = init;

// ---- File pool / dropdown ----
function toggleDropdown() { $('file-list').classList.toggle('show'); }

function renderFileList() {
  const el = $('file-list');
  el.replaceChildren();
  if (!pool.length) {
    const empty = document.createElement('div');
    empty.className = 'empty';
    empty.textContent = t('noFiles');
    el.appendChild(empty);
    return;
  }
  pool.forEach((f, idx) => {
    const row = document.createElement('div');
    row.className = 'file-item';
    row.draggable = true;
    row.dataset.idx = idx;

    const grip = document.createElement('span');
    grip.className = 'grip';
    grip.textContent = '::';
    grip.title = t('dragToReorder');

    const cb = document.createElement('input');
    cb.type = 'checkbox';
    cb.checked = f.selected;
    cb.onchange = () => { f.selected = cb.checked; onSelectionChange(); };
    const name = document.createElement('span');
    name.className = 'fname' + (f.virtual ? ' virtual' : '');
    name.textContent = f.name;
    name.title = f.id;
    const del = document.createElement('button');
    del.className = 'fdel';
    del.textContent = '✕';
    del.disabled = f.virtual;
    del.onclick = () => deleteFile(f);

    row.addEventListener('dragstart', e => {
      dragFromIdx = idx;
      row.classList.add('dragging');
      e.dataTransfer.effectAllowed = 'move';
    });
    row.addEventListener('dragend', () => row.classList.remove('dragging'));
    row.addEventListener('dragover', e => { e.preventDefault(); row.classList.add('drag-over'); });
    row.addEventListener('dragleave', () => row.classList.remove('drag-over'));
    row.addEventListener('drop', e => {
      e.preventDefault();
      row.classList.remove('drag-over');
      reorderPool(dragFromIdx, idx);
    });

    row.append(grip, cb, name, del);
    el.appendChild(row);
  });
}

function reorderPool(from, to) {
  if (from == null) throw new Error('Missing drag source index');
  if (from === to) return;
  const [moved] = pool.splice(from, 1);
  pool.splice(to, 0, moved);
  renderFileList();
}

function onSelectionChange() {
  // Preview the most recently relevant selected real file.
  const sel = pool.filter(f => f.selected && !f.virtual);
  if (sel.length) previewFile(sel[sel.length - 1].id).catch(e => showError(e.message));
}

async function deleteFile(f) {
  try {
    const r = await fetch(apiUrl('file/' + encodeURIComponent(f.id)), { method: 'DELETE' });
    if (!r.ok) throw new Error(await r.text());
  } catch (e) { return showError(e.message); }
  pool = pool.filter(x => x !== f);
  renderFileList();
}

function selectedFileIds() { return pool.filter(f => f.selected && !f.virtual).map(f => f.id); }
function selectedInputs() {
  return pool.filter(f => f.selected).map(f =>
    f.virtual ? { kind: 'step', index: f.stepIndex } : { kind: 'file', id: f.id });
}

async function previewFile(id) {
  const url = apiUrl('file/' + encodeURIComponent(id));
  const head = await fetch(url, { method: 'HEAD' });
  if (!head.ok) throw new Error(`${t('previewFailed')}: ${await head.text()}`);
  const ct = head.headers.get('content-type');
  if (!ct) throw new Error(`${t('previewFailed')}: missing content-type`);
  const area = $('preview-area');
  area.classList.remove('is-empty');
  area.replaceChildren();
  if (ct.startsWith('application/pdf')) {
    const embed = document.createElement('embed');
    embed.src = url;
    embed.type = 'application/pdf';
    area.appendChild(embed);
  } else if (ct.startsWith('image/')) {
    const img = document.createElement('img');
    img.src = url;
    img.alt = 'file';
    area.appendChild(img);
  }
  else {
    const r = await fetch(url);
    if (!r.ok) throw new Error(`${t('previewFailed')}: ${await r.text()}`);
    const text = await r.text();
    const pre = document.createElement('pre');
    pre.textContent = text;
    area.appendChild(pre);
  }
}

async function onUpload() {
  try {
    await uploadFiles(this.files);
    this.value = '';
  } catch (e) {
    showError(e.message);
  }
}

function setupDropZone() {
  const area = $('preview-area');
  area.onclick = () => { if (area.classList.contains('is-empty')) $('file-input').click(); };
  area.addEventListener('dragover', e => {
    e.preventDefault();
    area.classList.add('drag-active');
  });
  area.addEventListener('dragleave', e => {
    if (!area.contains(e.relatedTarget)) area.classList.remove('drag-active');
  });
  area.addEventListener('drop', async e => {
    e.preventDefault();
    area.classList.remove('drag-active');
    await uploadFiles(e.dataTransfer.files);
  });
}

async function uploadFiles(files) {
  if (!files.length) throw new Error('No files supplied for upload');
  const fd = new FormData();
  for (const f of files) fd.append('file', f);
  const r = await fetch(apiUrl('upload'), { method: 'POST', body: fd });
  if (!r.ok) throw new Error(`${t('uploadFailed')}: ${await r.text()}`);
  const data = await r.json();
  if (!Array.isArray(data.files)) throw new Error(`${t('uploadFailed')}: malformed response`);
  for (const f of data.files) pool.push({ id: f.id, name: f.filename, virtual: false, selected: true });
  renderFileList();
  onSelectionChange();
}

function readCurrentOptions() {
  if (!currentRead) throw new Error('Operation form has not been rendered');
  return currentRead();
}

// ---- Workflow ----
function openPicker() {
  const body = $('picker-body');
  body.replaceChildren();
  for (const f of schema) {
    const fl = document.createElement('div');
    fl.className = 'fam';
    fl.textContent = familyLabel(f.name);
    fl.title = f.name;
    body.appendChild(fl);
    const grid = document.createElement('div');
    grid.className = 'opgrid';
    for (const op of f.ops) {
      const d = document.createElement('div');
      d.textContent = opLabel(op.name);
      d.title = op.multi_input ? `${op.name} - ${t('requiresMultiple')}` : op.name;
      d.onclick = () => { $('op-picker').classList.remove('show'); showOpForm(f.name, op, true); };
      grid.appendChild(d);
    }
    body.appendChild(grid);
  }
  $('op-picker').classList.add('show');
}

function addStepFromForm() {
  if (!selOp) return showError(t('selectOperation'));
  const inputs = selectedInputs();
  if (!inputs.length) return showError(t('selectInputsForStep'));
  const stepIndex = wfSteps.length;
  wfSteps.push({ family: selFamily, op: selOp, options: readCurrentOptions(), inputs });

  // Consume current selection; add a virtual output for this step and select it.
  pool.forEach(f => f.selected = false);
  pool.push({ id: 'step:' + stepIndex, name: `-> ${t('stepOutput', stepIndex + 1)}`, virtual: true, stepIndex, selected: true });

  $('op-panel').style.display = 'none';
  selOp = null;
  selMeta = null;
  formWorkflowMode = false;
  renderChain();
  renderFileList();
  clearError();
}

function renderChain() {
  const el = $('chain');
  el.replaceChildren();
  el.appendChild(chainNode(t('inputNode'), true));
  wfSteps.forEach((s, i) => {
    el.appendChild(chainArrow());
    const n = document.createElement('div');
    n.className = 'node';
    const label = document.createElement('span');
    label.textContent = `${familyLabel(s.family)} > ${opLabel(s.op)}`;
    const x = document.createElement('button');
    x.className = 'btn-sm';
    x.textContent = '✕';
    x.onclick = () => removeStep(i);
    n.append(label, x);
    el.appendChild(n);
  });
  el.append(chainArrow(), chainNode(t('outputNode'), true));
}

function chainArrow() {
  const arrow = document.createElement('div');
  arrow.className = 'arrow';
  arrow.textContent = '↓';
  return arrow;
}

function chainNode(text, io) {
  const node = document.createElement('div');
  node.className = io ? 'node io' : 'node';
  node.textContent = text;
  return node;
}

function removeStep(i) {
  // Remove the step and any steps after it (their inputs may depend on it),
  // plus their virtual outputs. Keeps references consistent.
  wfSteps = wfSteps.slice(0, i);
  pool = pool.filter(f => !(f.virtual && f.stepIndex >= i));
  renderChain();
  renderFileList();
}

// ---- Execute ----
async function execSingle() {
  clearError();
  const ids = selectedFileIds();
  if (!ids.length) return showError(t('selectInputs'));
  if (!selOp) return showError(t('selectOperation'));
  const btn = $('btn-single');
  btn.disabled = true; btn.textContent = t('running');
  try {
    const r = await fetch(apiUrl('execute/single'), {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ artifact_ids: ids, family: selFamily, op: selOp,
        options_json: JSON.stringify(readCurrentOptions()) }),
    });
    if (!r.ok) throw new Error(await r.text());
    await showResult((await r.json()).result_id);
  } catch (e) { showError(e.message); }
  finally { btn.disabled = false; btn.textContent = t('execute'); }
}

async function execWorkflow() {
  clearError();
  if (!wfSteps.length) return showError(t('addAtLeastOneStep'));
  const btn = $('btn-exec-wf');
  btn.disabled = true; btn.textContent = t('running');
  try {
    const tasks = wfSteps.map(s => ({ family: s.family, op: s.op,
      options_json: JSON.stringify(s.options), inputs: s.inputs }));
    const r = await fetch(apiUrl('execute/workflow'), {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ tasks }),
    });
    if (!r.ok) throw new Error(await r.text());
    await showResult((await r.json()).result_id);
  } catch (e) { showError(e.message); }
  finally { btn.disabled = false; btn.textContent = t('executeWorkflow'); }
}

async function showResult(id) {
  lastResultId = id;
  const dl = $('dl-btn');
  dl.href = apiUrl('file/' + encodeURIComponent(id));
  dl.style.visibility = 'visible';
  const head = await fetch(apiUrl('file/' + encodeURIComponent(id)), { method: 'HEAD' });
  if (!head.ok) throw new Error(await head.text());
  const ct = head.headers.get('content-type');
  if (!ct) throw new Error('Missing result content-type');
  dl.download = ct.startsWith('application/pdf') ? 'result.pdf'
    : ct.startsWith('image/') ? 'result.' + (ct.split('/')[1] || 'png') : 'result.txt';
  await previewFile(id);
}

function showError(msg) { $('error-msg').textContent = msg; }
function clearError() { $('error-msg').textContent = ''; }

init();
