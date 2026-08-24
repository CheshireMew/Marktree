import { readFile, writeFile, mkdir, readdir } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import path from 'node:path'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const checkOnly = process.argv.includes('--check')
const generatedDirectory = path.join(root, 'src', 'generated')
const typesOutputPath = path.join(generatedDirectory, 'native.ts')
const apiOutputPath = path.join(generatedDirectory, 'nativeApi.ts')

const typeModuleDirectory = path.join(root, 'src-tauri', 'src', 'types')
const typeModulePaths = (await readdir(typeModuleDirectory, { withFileTypes: true }))
  .filter((entry) => entry.isFile() && entry.name.endsWith('.rs'))
  .map((entry) => path.join(typeModuleDirectory, entry.name))
  .sort()
const sourcePaths = [
  path.join(root, 'src-tauri', 'src', 'types.rs'),
  ...typeModulePaths,
  path.join(root, 'src-tauri', 'src', 'error.rs'),
  path.join(root, 'src-tauri', 'src', 'auth.rs'),
]
const commandSourcePaths = ['auth', 'documents', 'git', 'portability', 'workspaces'].map((name) =>
  path.join(root, 'src-tauri', 'src', 'commands', `${name}.rs`),
)
const registryPath = path.join(root, 'src-tauri', 'src', 'ipc_commands.list')
const eventRegistryPath = path.join(root, 'src-tauri', 'src', 'ipc_events.list')

const [sources, commandSources, registrySource, eventRegistrySource] = await Promise.all([
  Promise.all(sourcePaths.map((sourcePath) => readFile(sourcePath, 'utf8'))),
  Promise.all(commandSourcePaths.map((sourcePath) => readFile(sourcePath, 'utf8'))),
  readFile(registryPath, 'utf8'),
  readFile(eventRegistryPath, 'utf8'),
])
const commandsSource = commandSources.join('\n')

const registry = registrySource
  .split(/\r?\n/)
  .map((line) => line.trim())
  .filter((line) => line && !line.startsWith('#'))
const eventRegistry = eventRegistrySource
  .split(/\r?\n/)
  .map((line) => line.trim())
  .filter((line) => line && !line.startsWith('#'))

if (new Set(registry).size !== registry.length) {
  throw new Error('The IPC command registry contains duplicate command names.')
}

function lowerCamel(value) {
  if (value.includes('_')) {
    const [first, ...rest] = value.split('_')
    return first + rest.map((part) => part[0].toUpperCase() + part.slice(1)).join('')
  }
  return value[0].toLowerCase() + value.slice(1)
}

function upperCamel(value) {
  const camel = lowerCamel(value)
  return camel[0].toUpperCase() + camel.slice(1)
}

function matchingDelimiter(source, openingIndex, opening, closing) {
  let depth = 0
  let stringQuote = null
  let escaped = false
  let lineComment = false
  let blockCommentDepth = 0

  for (let index = openingIndex; index < source.length; index += 1) {
    const character = source[index]
    const next = source[index + 1]

    if (lineComment) {
      if (character === '\n') lineComment = false
      continue
    }
    if (blockCommentDepth > 0) {
      if (character === '/' && next === '*') {
        blockCommentDepth += 1
        index += 1
      } else if (character === '*' && next === '/') {
        blockCommentDepth -= 1
        index += 1
      }
      continue
    }
    if (stringQuote) {
      if (escaped) {
        escaped = false
      } else if (character === '\\') {
        escaped = true
      } else if (character === stringQuote) {
        stringQuote = null
      }
      continue
    }
    if (character === '/' && next === '/') {
      lineComment = true
      index += 1
      continue
    }
    if (character === '/' && next === '*') {
      blockCommentDepth = 1
      index += 1
      continue
    }
    if (character === '"') {
      stringQuote = character
      continue
    }
    if (character === opening) depth += 1
    if (character === closing) {
      depth -= 1
      if (depth === 0) return index
    }
  }
  throw new Error(`Unclosed ${opening} delimiter.`)
}

function splitTopLevel(value, delimiter = ',') {
  const parts = []
  let start = 0
  let angle = 0
  let round = 0
  let square = 0
  let curly = 0

  for (let index = 0; index < value.length; index += 1) {
    const character = value[index]
    if (character === '<') angle += 1
    else if (character === '>') angle -= 1
    else if (character === '(') round += 1
    else if (character === ')') round -= 1
    else if (character === '[') square += 1
    else if (character === ']') square -= 1
    else if (character === '{') curly += 1
    else if (character === '}') curly -= 1
    else if (
      character === delimiter &&
      angle === 0 &&
      round === 0 &&
      square === 0 &&
      curly === 0
    ) {
      parts.push(value.slice(start, index).trim())
      start = index + 1
    }
  }
  const tail = value.slice(start).trim()
  if (tail) parts.push(tail)
  return parts
}

function genericType(value) {
  const opening = value.indexOf('<')
  if (opening < 0 || !value.endsWith('>')) return null
  return {
    name: value.slice(0, opening).trim().split('::').at(-1),
    arguments: splitTopLevel(value.slice(opening + 1, -1)),
  }
}

function mapRustType(rawType) {
  let value = rawType
    .trim()
    .replace(/^&(?:'[A-Za-z_][A-Za-z0-9_]*\s+)?/, '')
    .replace(/\s+/g, ' ')
  const generic = genericType(value)
  if (generic) {
    const mapped = generic.arguments.map(mapRustType)
    if (generic.name === 'AppResult' || generic.name === 'Result') return mapped[0]
    if (generic.name === 'Option') return `${mapped[0]} | null`
    if (generic.name === 'Vec') return `${mapped[0]}[]`
    if (generic.name === 'BTreeMap' || generic.name === 'HashMap') {
      return `Record<${mapped[0]}, ${mapped[1]}>`
    }
    throw new Error(`Unsupported Rust generic type: ${rawType}`)
  }

  const leaf = value.split('::').at(-1)
  if (leaf === 'String' || leaf === 'str') return 'string'
  if (leaf === 'bool') return 'boolean'
  if (/^(?:u|i)(?:8|16|32)$/.test(leaf) || /^(?:f)(?:32|64)$/.test(leaf)) {
    return 'number'
  }
  if (/^(?:u64|i64|usize|isize)$/.test(leaf)) return 'SafeInteger'
  if (/^(?:u128|i128)$/.test(leaf)) {
    throw new Error(
      `Rust type ${rawType} cannot be represented exactly by the JavaScript IPC wire format.`,
    )
  }
  if (leaf === '()') return 'void'
  return leaf
}

function declarationAttributes(source, declarationIndex) {
  const prefix = source.slice(0, declarationIndex)
  return /(?:#\[[^\]]*\]\s*)+$/s.exec(prefix)?.[0] ?? ''
}

function leadingAttributes(value) {
  const attributes = []
  let rest = value.trim()
  while (rest.startsWith('#[')) {
    const opening = rest.indexOf('[')
    const closing = matchingDelimiter(rest, opening, '[', ']')
    attributes.push(rest.slice(opening + 1, closing).trim())
    rest = rest.slice(closing + 1).trim()
  }
  return { attributes, rest }
}

function serdeOptions(attributes, context) {
  const options = new Map()
  for (const attribute of attributes) {
    if (attribute === 'serde') continue
    if (!attribute.startsWith('serde(') || !attribute.endsWith(')')) continue
    for (const item of splitTopLevel(attribute.slice('serde('.length, -1))) {
      const assignment = /^([A-Za-z_][A-Za-z0-9_]*)\s*=\s*"([^"]+)"$/.exec(item)
      const key = assignment?.[1] ?? item
      const value = assignment?.[2] ?? true
      const supported = [
        'rename_all',
        'rename',
        'default',
        'skip',
        'skip_serializing_if',
      ]
      if (!supported.includes(key)) {
        throw new Error(`Unsupported serde option '${item}' on ${context}.`)
      }
      options.set(key, value)
    }
  }
  return options
}

function serializedDeclarations(source) {
  const declarations = []
  const pattern = /pub\s+(struct|enum)\s+([A-Za-z_][A-Za-z0-9_]*)\s*\{/g
  for (const match of source.matchAll(pattern)) {
    const attributes = declarationAttributes(source, match.index)
    if (!/#\[derive\([^\]]*\bSerialize\b[^\]]*\)\]/s.test(attributes)) continue
    const opening = source.indexOf('{', match.index)
    const closing = matchingDelimiter(source, opening, '{', '}')
    declarations.push({
      kind: match[1],
      name: match[2],
      body: source.slice(opening + 1, closing),
      attributes: leadingAttributes(attributes).attributes,
    })
  }
  return declarations
}

function renderDeclaration(declaration) {
  const containerOptions = serdeOptions(
    declaration.attributes,
    declaration.name,
  )
  const renameAll = containerOptions.get('rename_all')
  if (renameAll && renameAll !== 'camelCase') {
    throw new Error(
      `Unsupported serde rename_all '${renameAll}' on ${declaration.name}.`,
    )
  }
  if (declaration.kind === 'enum') {
    const variants = splitTopLevel(declaration.body)
      .map((variant) => leadingAttributes(variant))
      .filter(({ rest }) => rest.length > 0)
      .map(({ attributes, rest }) => {
        const match = /^([A-Za-z_][A-Za-z0-9_]*)$/.exec(rest)
        if (!match) {
          throw new Error(
            `Payload enum variant '${rest}' in ${declaration.name} requires an explicit wire-contract representation.`,
          )
        }
        const options = serdeOptions(attributes, `${declaration.name}::${match[1]}`)
        if (options.has('skip')) return null
        return options.get('rename') ?? (renameAll === 'camelCase' ? lowerCamel(match[1]) : match[1])
      })
      .filter(Boolean)
    if (variants.length === 0) {
      throw new Error(`No serializable variants found for ${declaration.name}.`)
    }
    return `export type ${declaration.name} =\n${variants
      .map((variant) => `  | '${variant}'`)
      .join('\n')}`
  }

  const fields = splitTopLevel(declaration.body)
    .map((field) => leadingAttributes(field))
    .filter(({ rest }) => rest.length > 0)
    .map(({ attributes, rest }) => {
      const match = /^pub\s+([A-Za-z_][A-Za-z0-9_]*)\s*:\s*([\s\S]+)$/.exec(rest)
      if (!match) throw new Error(`Unsupported field in ${declaration.name}: ${rest}`)
      const options = serdeOptions(attributes, `${declaration.name}.${match[1]}`)
      if (options.has('skip')) return null
      const type = mapRustType(match[2])
      const optional = options.has('skip_serializing_if')
      const fieldName = options.get('rename') ?? (
        renameAll === 'camelCase' ? lowerCamel(match[1]) : match[1]
      )
      return `  ${fieldName}${optional ? '?' : ''}: ${type}`
    })
    .filter(Boolean)
  return `export interface ${declaration.name} {\n${fields.join('\n')}\n}`
}

const declarations = sources.flatMap(serializedDeclarations)
const duplicateType = declarations.find(
  (declaration, index) =>
    declarations.findIndex((candidate) => candidate.name === declaration.name) !== index,
)
if (duplicateType) {
  throw new Error(`Duplicate serialized Rust type: ${duplicateType.name}`)
}

function findCommand(source, command) {
  const pattern = new RegExp(`pub\\s+(?:async\\s+)?fn\\s+${command}\\s*\\(`)
  const match = pattern.exec(source)
  if (!match) throw new Error(`Registered IPC command is missing: ${command}`)
  const attributes = declarationAttributes(source, match.index)
  if (!attributes.includes('tauri::command')) {
    throw new Error(`Registered function is not a Tauri command: ${command}`)
  }
  const opening = source.indexOf('(', match.index)
  const closing = matchingDelimiter(source, opening, '(', ')')
  const argumentsSource = source.slice(opening + 1, closing)
  const bodyOpening = source.indexOf('{', closing)
  const returnSource = source.slice(closing + 1, bodyOpening)
  const returnMatch = /->\s*([\s\S]+)$/.exec(returnSource)
  const returnType = mapRustType(returnMatch ? returnMatch[1] : '()')
  const parameters = splitTopLevel(argumentsSource)
    .map((parameter) => {
      const separator = parameter.indexOf(':')
      if (separator < 0) throw new Error(`Unsupported parameter in ${command}: ${parameter}`)
      const name = parameter.slice(0, separator).trim()
      const rustType = parameter.slice(separator + 1).trim()
      return { name, rustType }
    })
    .filter(
      (parameter) =>
        !parameter.rustType.startsWith('State<') &&
        parameter.rustType !== 'AppHandle' &&
        parameter.rustType !== 'WebviewWindow',
    )
    .map((parameter) => ({ ...parameter, type: mapRustType(parameter.rustType) }))
  return { command, method: lowerCamel(command), parameters, returnType }
}

const commandBindings = registry.map((command) => findCommand(commandsSource, command))
const declaredCommands = [...commandsSource.matchAll(/pub\s+(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(/g)]
  .filter((match) => declarationAttributes(commandsSource, match.index).includes('tauri::command'))
  .map((match) => match[1])
const missingFromRegistry = declaredCommands.filter((command) => !registry.includes(command))
if (missingFromRegistry.length) {
  throw new Error(
    `Tauri commands are missing from the IPC registry: ${missingFromRegistry.join(', ')}`,
  )
}

const declarationNames = new Set(declarations.map((declaration) => declaration.name))
const reachableNames = new Set(['ErrorPayload'])
for (const eventName of eventRegistry) {
  const declarationName = upperCamel(eventName)
  if (!declarationNames.has(declarationName)) {
    throw new Error(`Registered IPC event type is missing: ${declarationName}`)
  }
  reachableNames.add(declarationName)
}
for (const binding of commandBindings) {
  for (const type of [binding.returnType, ...binding.parameters.map((parameter) => parameter.type)]) {
    for (const name of type.match(/[A-Za-z_][A-Za-z0-9_]*/g) ?? []) {
      if (declarationNames.has(name)) reachableNames.add(name)
    }
  }
}
let foundReachableType = true
while (foundReachableType) {
  foundReachableType = false
  for (const declaration of declarations) {
    if (!reachableNames.has(declaration.name)) continue
    for (const name of declaration.body.match(/[A-Za-z_][A-Za-z0-9_]*/g) ?? []) {
      if (declarationNames.has(name) && !reachableNames.has(name)) {
        reachableNames.add(name)
        foundReachableType = true
      }
    }
  }
}
const wireDeclarations = declarations.filter((declaration) =>
  reachableNames.has(declaration.name),
)

const typesOutput = `// This file is generated from the reachable Rust IPC serialization types. Do not edit it by hand.
// Run \`npm run generate:bindings\` after changing a native contract.

/** A Rust 64-bit or pointer-sized integer carried as a JSON number; IPC values must remain within JavaScript's safe-integer range. */
export type SafeInteger = number

${wireDeclarations.map(renderDeclaration).join('\n\n')}

export type NativeError = ErrorPayload
`
const argumentTypes = commandBindings
  .filter((binding) => binding.parameters.length > 0)
  .map((binding) => {
    const name = `${upperCamel(binding.method)}Args`
    const fields = binding.parameters
      .map((parameter) => `  ${lowerCamel(parameter.name)}: ${parameter.type}`)
      .join('\n')
    return `export interface ${name} extends Record<string, unknown> {\n${fields}\n}`
  })
  .join('\n\n')

const methods = commandBindings
  .map((binding) => {
    const call = binding.parameters.length
      ? `invoke<${binding.returnType}>('${binding.command}', args)`
      : `invoke<${binding.returnType}>('${binding.command}')`
    const signature = binding.parameters.length
      ? `(args: ${upperCamel(binding.method)}Args) => ${call}`
      : `() => ${call}`
    return `  ${binding.method}: ${signature},`
  })
  .join('\n')

const referencedNativeTypes = [
  ...new Set(
    commandBindings
      .flatMap((binding) => [
        binding.returnType,
        ...binding.parameters.map((parameter) => parameter.type),
      ])
      .flatMap((type) => type.match(/[A-Za-z_][A-Za-z0-9_]*/g) ?? []),
  ),
]
  .filter(
    (name) =>
      !['void', 'string', 'number', 'boolean', 'null', 'Record'].includes(name),
  )
  .sort()

const apiOutput = `// This file is generated from the Rust command registry and signatures.
// Do not edit it by hand. Run \`npm run generate:bindings\` instead.

import { invoke } from '@tauri-apps/api/core'

import type {
${referencedNativeTypes.map((name) => `  ${name},`).join('\n')}
} from './native'

${argumentTypes}

export const isTauri = () => '__TAURI_INTERNALS__' in window

export const nativeApi = {
${methods}
}
`

async function writeOrCheck(outputPath, content) {
  let existing = null
  try {
    existing = await readFile(outputPath, 'utf8')
  } catch {
    // A missing generated file is reported as drift in check mode.
  }
  if (existing === content) return
  if (checkOnly) {
    throw new Error(
      `${path.relative(root, outputPath)} is stale. Run npm run generate:bindings.`,
    )
  }
  await mkdir(path.dirname(outputPath), { recursive: true })
  await writeFile(outputPath, content, 'utf8')
}

await Promise.all([
  writeOrCheck(typesOutputPath, typesOutput),
  writeOrCheck(apiOutputPath, apiOutput),
])
