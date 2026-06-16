// JSON-Schema (draft 2020-12) → HTML form renderer.
// Builds form fields from an op's schema and reads values back as a JS object.

function resolveRef(schema, root) {
  if (schema && schema.$ref) {
    const path = schema.$ref.replace(/^#\//, '').split('/');
    let node = root;
    for (const p of path) node = node[p];
    return node;
  }
  return schema;
}

// Returns {types:[...], enumVals, refSchema} normalizing nullable/anyOf/oneOf forms.
function analyze(prop, root) {
  prop = resolveRef(prop, root);
  let types = [];
  let enumVals = null;
  let nested = null;

  if (Array.isArray(prop.type)) types = prop.type.filter(t => t !== 'null');
  else if (prop.type) types = [prop.type];

  if (prop.enum) enumVals = prop.enum.slice();

  // oneOf/anyOf: collect const strings (enum) or a single $ref (nested object/enum)
  const variants = prop.oneOf || prop.anyOf;
  if (variants) {
    const consts = [];
    for (const v of variants) {
      if (v.const !== undefined) consts.push(v.const);
      else if (v.$ref) {
        const r = resolveRef(v, root);
        if (r.enum) consts.push(...r.enum);
        else if (r.oneOf) for (const o of r.oneOf) if (o.const !== undefined) consts.push(o.const);
        else nested = r;
      }
    }
    if (consts.length) enumVals = consts;
  }

  if (!types.length && (prop.properties || nested)) types = ['object'];
  if (nested && nested.properties) { types = ['object']; prop = nested; }
  return { prop, types, enumVals, nested };
}

// Build one field control. Returns {el, read} where read() returns the value or undefined.
function buildField(key, prop, required, root) {
  const { prop: p, types, enumVals } = analyze(prop, root);
  const wrap = document.createElement('div');
  wrap.className = 'field';

  const label = document.createElement('label');
  label.textContent = key;
  const tag = document.createElement('span');
  tag.className = 'tag ' + (required ? 'req' : 'opt');
  tag.textContent = required ? 'required' : 'optional';
  label.appendChild(tag);
  wrap.appendChild(label);

  if (p.description) {
    const d = document.createElement('div');
    d.className = 'fdesc';
    d.textContent = p.description;
    wrap.appendChild(d);
  }

  let read;

  if (enumVals) {
    const sel = document.createElement('select');
    if (!required) sel.appendChild(new Option('— none —', ''));
    for (const v of enumVals) sel.appendChild(new Option(v, v));
    if (p.default !== undefined) sel.value = p.default;
    wrap.appendChild(sel);
    read = () => sel.value === '' ? undefined : sel.value;
  } else if (types.includes('boolean')) {
    const cb = document.createElement('input');
    cb.type = 'checkbox';
    if (p.default === true) cb.checked = true;
    cb.classList.add('cb');
    wrap.appendChild(cb);
    read = () => cb.checked;
  } else if (types.includes('integer') || types.includes('number')) {
    const inp = document.createElement('input');
    inp.type = 'number';
    if (types.includes('integer')) inp.step = '1';
    if (p.minimum !== undefined) inp.min = p.minimum;
    if (p.maximum !== undefined) inp.max = p.maximum;
    if (p.default !== undefined) inp.value = p.default;
    inp.placeholder = required ? '' : '(default)';
    wrap.appendChild(inp);
    read = () => {
      if (inp.value === '') return undefined;
      return types.includes('integer') ? parseInt(inp.value, 10) : parseFloat(inp.value);
    };
  } else if (types.includes('object') && p.properties) {
    const fs = document.createElement('div');
    fs.className = 'nested';
    const subReads = [];
    const subReq = p.required || [];
    for (const [sk, sp] of Object.entries(p.properties)) {
      const f = buildField(sk, sp, subReq.includes(sk), root);
      fs.appendChild(f.el);
      subReads.push([sk, f.read]);
    }
    wrap.appendChild(fs);
    read = () => {
      const obj = {};
      for (const [sk, r] of subReads) { const v = r(); if (v !== undefined) obj[sk] = v; }
      return Object.keys(obj).length ? obj : undefined;
    };
  } else if (types.includes('array')) {
    // Simple comma-separated array for primitive items; JSON for complex.
    const inp = document.createElement('input');
    inp.type = 'text';
    inp.placeholder = 'comma,separated';
    wrap.appendChild(inp);
    const itemSchema = analyze(p.items || {}, root);
    read = () => {
      if (inp.value.trim() === '') return undefined;
      return inp.value.split(',').map(s => {
        s = s.trim();
        if (itemSchema.types.includes('integer')) return parseInt(s, 10);
        if (itemSchema.types.includes('number')) return parseFloat(s, 10);
        return s;
      });
    };
  } else {
    const inp = document.createElement('input');
    inp.type = 'text';
    if (p.default !== undefined) inp.value = p.default;
    inp.placeholder = required ? '' : '(default)';
    wrap.appendChild(inp);
    read = () => inp.value === '' ? undefined : inp.value;
  }

  return { el: wrap, read };
}

// Render a full schema into `container`. Returns a read() that yields the options object.
function renderForm(schema, container) {
  container.innerHTML = '';
  const root = schema;
  const props = schema.properties || {};
  const required = schema.required || [];
  const reads = [];

  if (Object.keys(props).length === 0) {
    const p = document.createElement('div');
    p.className = 'placeholder';
    p.textContent = 'This operation has no parameters.';
    container.appendChild(p);
    return () => ({});
  }

  for (const [key, prop] of Object.entries(props)) {
    const f = buildField(key, prop, required.includes(key), root);
    container.appendChild(f.el);
    reads.push([key, f.read]);
  }

  return () => {
    const obj = {};
    for (const [key, r] of reads) { const v = r(); if (v !== undefined) obj[key] = v; }
    return obj;
  };
}

window.renderForm = renderForm;
