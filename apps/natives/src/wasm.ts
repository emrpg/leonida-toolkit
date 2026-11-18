import type { NativeEntry, StubLang } from './types';

function mapCppType(t: string): string {
  const s = t.trim().toLowerCase();
  if (s === 'void') return 'void';
  if (s === 'bool' || s === 'boolean') return 'BOOL';
  if (s === 'float') return 'float';
  if (s === 'int' || s === 'entity' || s === 'ped' || s === 'vehicle' || s === 'player' || s === 'blip')
    return 'int';
  if (s === 'hash') return 'Hash';
  if (s === 'char*' || s === 'const char*') return 'const char*';
  if (s === 'vector3') return 'Vector3';
  return t || 'Any';
}

function mapCsType(t: string): string {
  const s = t.trim().toLowerCase();
  if (s === 'void') return 'void';
  if (s === 'bool' || s === 'boolean') return 'bool';
  if (s === 'float') return 'float';
  if (s === 'char*' || s === 'const char*') return 'string';
  if (s === 'vector3') return 'Vector3';
  if (s === 'hash') return 'uint';
  return 'int';
}

function mapRustType(t: string): string {
  const s = t.trim().toLowerCase();
  if (s === 'void') return '()';
  if (s === 'bool' || s === 'boolean') return 'bool';
  if (s === 'float') return 'f32';
  if (s === 'char*' || s === 'const char*') return '*const c_char';
  if (s === 'hash') return 'u32';
  return 'i32';
}

export function generateShvStub(entry: NativeEntry, lang: StubLang): string {
  const args = entry.params.map((p) => p.name).join(', ');
  if (lang === 'cpp') {
    const ret = mapCppType(entry.return_type);
    const sig = entry.params.map((p) => `${mapCppType(p.param_type)} ${p.name}`).join(', ');
    const call = `invoke<${ret}>(${entry.hash}${args ? `, ${args}` : ''});`;
    return `// ${entry.namespace}::${entry.name} (${entry.hash})\nstatic ${ret} ${entry.name}(${sig}) {\n    ${ret === 'void' ? call : `return ${call}`}\n}`;
  }
  if (lang === 'csharp') {
    const ret = mapCsType(entry.return_type);
    const sig = entry.params.map((p) => `${mapCsType(p.param_type)} ${p.name}`).join(', ');
    const call =
      ret === 'void'
        ? `Function.Call((Hash)${entry.hash}${args ? `, ${args}` : ''});`
        : `return Function.Call<${ret}>((Hash)${entry.hash}${args ? `, ${args}` : ''});`;
    return `// ${entry.namespace}.${entry.name}\npublic static ${ret} ${entry.name}(${sig}) {\n    ${call}\n}`;
  }
  const ret = mapRustType(entry.return_type);
  const sig = entry.params.map((p) => `${p.name}: ${mapRustType(p.param_type)}`).join(', ');
  const pushes = entry.params.map((p) => `    native_push(${p.name});`).join('\n');
  return `// ${entry.namespace}::${entry.name}\npub unsafe fn ${entry.name.toLowerCase()}(${sig}) -> ${ret} {\n    native_init(${entry.hash});\n${pushes ? `${pushes}\n` : ''}    native_call::<${ret}>()\n}`;
}
