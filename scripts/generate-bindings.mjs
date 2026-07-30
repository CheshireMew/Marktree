import { readFile, writeFile, mkdir } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import path from 'node:path'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const checkOnly = process.argv.includes('--check')
const generatedDirectory = path.join(root, 'src', 'generated')
const typesOutputPath = path.join(generatedDirectory, 'native.ts')
const apiOutputPath = path.join(generatedDirectory, 'nativeApi.ts')

const sourcePaths = [
  path.join(root, 'src-tauri', 'src', 'types.rs'),
  path.join(root, 'src-tauri', 'src', 'error.rs'),
  path.join(root, 'src-tauri', 'src', 'auth.rs'),
]
const commandSourcePaths = ['auth', 'documents', 'git', 'workspaces'].map((name) =>
  path.join(root, 'src-tauri', 'src', 'commands', `${name}.rs`),
)
const registryPath = path.join(root, 'src-tauri', 'src', 'ipc_commands.list')

const [sources, commandSources, registrySource] = await Promise.all([
  Promise.all(sourcePaths.map((sourcePath) => readFile(sourcePath, 'utf8'))),
  Promise.all(commandSourcePaths.map((sourcePath) => readFile(sourcePath, 'utf8'))),
  readFile(registryPath, 'utf8'),
])
const commandsSource = commandSources.join('\n')

const registry = registrySource
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

  for (let index = 0; index < value.length; index += 1) {
    const character = value[index]
    if (character === '<') angle += 1
    else if (character === '>') angle -= 1
    else if (character === '(') round += 1
    else if (character === ')') round -= 1
    else if (character === '[') square += 1
    else if (character === ']') square -= 1
    else if (
      character === delimiter &&
      angle === 0 &&
      round === 0 &&
      square === 0
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
  if (/^(?:u|i|f)(?:8|16|32|64|128|size)$/.test(leaf)) return 'number'
  if (leaf === '()') return 'void'
  return leaf
}

function declarationAttributes(source, declarationIndex) {
  const previousClosingBrace = source.lastIndexOf('\n}', declarationIndex)
  return source.slice(previousClosingBrace + 2, declarationIndex)
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
    })
  }
  return declarations
}

function renderDeclaration(declaration) {
  if (declaration.kind === 'enum') {
    const variants = declaration.body
      .split(/\r?\n/)
      .map((line) => line.trim().replace(/,$/, ''))
      .filter((line) => /^[A-Za-z_][A-Za-z0-9_]*$/.test(line))
    if (variants.length === 0) {
      throw new Error(`No serializable variants found for ${declaration.name}.`)
    }
    return `export type ${declaration.name} =\n${variants
      .map((variant) => `  | '${lowerCamel(variant)}'`)
      .join('\n')}`
  }

  const fields = declaration.body
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.startsWith('pub '))
    .map((line) => {
      const match = /^pub\s+([A-Za-z_][A-Za-z0-9_]*)\s*:\s*(.+),$/.exec(line)
      if (!match) throw new Error(`Unsupported field in ${declaration.name}: ${line}`)
      return `  ${lowerCamel(match[1])}: ${mapRustType(match[2])}`
    })
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

const typesOutput = `// This file is generated from Rust serialization types. Do not edit it by hand.
// Run \`npm run generate:bindings\` after changing a native contract.

${declarations.map(renderDeclaration).join('\n\n')}

export type NativeError = ErrorPayload
`

function findCommand(source, command) {
  const pattern = new RegExp(`pub\\s+(?:async\\s+)?fn\\s+${command}\\s*\\(`)
  const match = pattern.exec(source)
  if (!match) throw new Error(`Registered IPC command is missing: ${command}`)
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
        !parameter.rustType.startsWith('State<') && parameter.rustType !== 'AppHandle',
    )
    .map((parameter) => ({ ...parameter, type: mapRustType(parameter.rustType) }))
  return { command, method: lowerCamel(command), parameters, returnType }
}

const commandBindings = registry.map((command) => findCommand(commandsSource, command))
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
