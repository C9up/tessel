import { readdir, readFile } from 'node:fs/promises'
import { join, posix } from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, test } from '@c9up/helix'
import ts from 'typescript'

/**
 * The hexagonal rule, taken from nadeo (the "business at the centre" decision): each
 * module has four layers, and each layer imports only what it may.
 *
 *   domain/          its domain, shared/domain, @c9up/atom, @c9up/chronos
 *   application/     its domain and application, shared/domain, the framework's
 *                    injection, another module's events and ports
 *   infrastructure/  everything except its application and its http
 *   http/            everything except its infrastructure and the ORM
 *
 * `start/` composes: it never opens a repository, runs no SQL, defines no RPC point
 * and decides nothing. A module reads only its own tables.
 */

const ROOT = fileURLToPath(new URL('../../../../', import.meta.url))
const MODULES = join(ROOT, 'app/modules')
const START = join(ROOT, 'start')

const LAYERS = ['domain', 'application', 'infrastructure', 'http'] as const
type Layer = (typeof LAYERS)[number]
const isLayer = (v: string | undefined): v is Layer => LAYERS.some((l) => l === v)

const IMPORT_RE = /(?:^|\n)\s*(?:import|export)\s[^'"]*?from\s+['"]([^'"]+)['"]|import\(\s*['"]([^'"]+)['"]\s*\)/g

/** Where an import lands: `{ module, layer }` for one of the app's modules, the package name otherwise. */
function target(file: string, specifier: string): { module: string; layer: string } | { pkg: string } {
  if (specifier.startsWith('.')) {
    const parts = posix.normalize(posix.join(posix.dirname(file), specifier)).split('/')
    return { module: parts[0] ?? '', layer: parts[1] ?? '' }
  }
  const own = /^#modules\/([^/]+)\/?([^/]*)/.exec(specifier)
  if (own) return { module: own[1] ?? '', layer: own[2] ?? '' }
  const pkg = /^(@[^/]+\/[^/]+|[^/]+)/.exec(specifier)
  return { pkg: pkg?.[1] ?? specifier }
}

/** Why an import breaks the rule of its layer, or `null`. */
function breach(module: string, layer: Layer, to: ReturnType<typeof target>, specifier: string): string | null {
  if ('pkg' in to) {
    if (layer === 'domain') return ['@c9up/atom', '@c9up/chronos'].includes(to.pkg) ? null : `the domain imports ${to.pkg}`
    if (layer === 'application') return ['@c9up/atom', '@c9up/chronos', '@c9up/ream'].includes(to.pkg) ? null : `the application imports ${to.pkg}`
    if (layer === 'http') return to.pkg === '@c9up/atlas' ? 'http imports the ORM' : null
    return null
  }
  const sharedDomain = to.module === 'shared' && to.layer === 'domain'
  if (to.module === module) {
    if (layer === 'domain' && to.layer !== 'domain') return `the domain imports its ${to.layer || 'module root'}`
    if (layer === 'application' && to.layer !== 'domain' && to.layer !== 'application') return `the application imports its ${to.layer || 'module root'}`
    if (layer === 'infrastructure' && (to.layer === 'application' || to.layer === 'http')) return `the infrastructure imports its ${to.layer}`
    if (layer === 'http' && to.layer === 'infrastructure') return 'http imports its infrastructure'
    return null
  }
  if (sharedDomain) return null
  if (layer === 'domain') return `the domain imports ${specifier}`
  if (layer === 'application') {
    const contract = /^#modules\/[^/]+\/(?:events\.js|domain\/(?:ports|events)\.js)$/.test(specifier)
    return contract ? null : `the application imports ${specifier}`
  }
  return null
}

/** Who owns each table: the module whose migrations create it. A new table is registered here. */
const OWNER: Record<string, readonly string[]> = {
  shared: ['outbox'],
  identity: ['accounts', 'devices', 'sessions', 'key_packages', 'profiles'],
}
const TABLE_OWNER = new Map(Object.entries(OWNER).flatMap(([module, tables]) => tables.map((t): [string, string] => [t, module])))
const TABLE_RE = /\b(?:FROM|JOIN|INTO|UPDATE|TABLE)\s+([a-z_]+)\b/gi
/** A value import of another module's ORM model: reading or writing its table. A type alone reads nothing. */
const ENTITY_RE = /^\s*import\s+(?!type\b)[^'"]*from\s+['"]#modules\/([a-z]+)\/(?:infrastructure\/)?entities\//gm

/** The tables of other modules a file reads or writes. */
function foreignTables(module: string, text: string): string[] {
  const out = new Set<string>()
  for (const m of text.matchAll(TABLE_RE)) {
    const owner = TABLE_OWNER.get((m[1] ?? '').toLowerCase())
    if (owner && owner !== module) out.add(`reads ${m[1]?.toLowerCase()} of ${owner}`)
  }
  for (const m of text.matchAll(ENTITY_RE)) if (m[1] && m[1] !== module) out.add(`uses the ORM model of ${m[1]}`)
  return [...out].sort()
}

const DB_IN_START = /\b(?:BaseRepository|TenantRepository)\b|\.(?:query|execute)\(\s*[`'"]\s*(?:SELECT|INSERT|UPDATE|DELETE|WITH)\b/i
/** A method registered under a name of its own with its handler written right there. */
const RPC_POINT_IN_START = /\b(?:rpc\.method|method|operatorMethod)\(\s*['"][^'"]+['"]\s*,\s*async\b/
const DB_IN_CORE = /@Inject\(\s*['"]db['"]\s*\)/

/** Operators that compare, combine conditions, fall back or compute: a decision, not a wiring. */
const DECIDING_OPERATORS = new Set<ts.SyntaxKind>([
  ts.SyntaxKind.EqualsEqualsToken,
  ts.SyntaxKind.EqualsEqualsEqualsToken,
  ts.SyntaxKind.ExclamationEqualsToken,
  ts.SyntaxKind.ExclamationEqualsEqualsToken,
  ts.SyntaxKind.LessThanToken,
  ts.SyntaxKind.LessThanEqualsToken,
  ts.SyntaxKind.GreaterThanToken,
  ts.SyntaxKind.GreaterThanEqualsToken,
  ts.SyntaxKind.AmpersandAmpersandToken,
  ts.SyntaxKind.BarBarToken,
  ts.SyntaxKind.QuestionQuestionToken,
  ts.SyntaxKind.PlusToken,
  ts.SyntaxKind.MinusToken,
  ts.SyntaxKind.AsteriskToken,
  ts.SyntaxKind.SlashToken,
  ts.SyntaxKind.PercentToken,
  ts.SyntaxKind.InstanceOfKeyword,
  ts.SyntaxKind.InKeyword,
])

/** What a list or a value is turned into: a selection, a count, a projection, a format. */
const SHAPING_METHODS = new Set([
  'map',
  'flatMap',
  'filter',
  'find',
  'findIndex',
  'some',
  'every',
  'reduce',
  'forEach',
  'sort',
  'includes',
  'startsWith',
  'endsWith',
  'slice',
  'join',
  'split',
  'trim',
  'toFixed',
  'catch',
])

/** A store's `find(id)`, not a list's `find((x) => …)`: a list is searched with a function. */
function storeFind(call: ts.CallExpression): boolean {
  const first = call.arguments[0]
  return (
    ts.isPropertyAccessExpression(call.expression) &&
    call.expression.name.text === 'find' &&
    first !== undefined &&
    !ts.isArrowFunction(first) &&
    !ts.isFunctionExpression(first)
  )
}

/** A loop that only registers points: `for (… of …) method(…)`. */
function registersPoints(loop: ts.ForOfStatement): boolean {
  const body = ts.isBlock(loop.statement) && loop.statement.statements.length === 1 ? loop.statement.statements[0] : loop.statement
  if (!body || !ts.isExpressionStatement(body) || !ts.isCallExpression(body.expression)) return false
  return /^(?:method|operatorMethod|rpc\.method)$/.test(body.expression.expression.getText())
}

/** Why a node decides rather than composes, or `null`. */
function decisionOf(node: ts.Node): string | null {
  if (ts.isIfStatement(node) || ts.isConditionalExpression(node) || ts.isSwitchStatement(node)) return 'a branch'
  if (ts.isBinaryExpression(node) && DECIDING_OPERATORS.has(node.operatorToken.kind)) return `an operator (${node.operatorToken.getText()})`
  if (ts.isPrefixUnaryExpression(node) && node.operator === ts.SyntaxKind.ExclamationToken) return 'a negation'
  if (ts.isPrefixUnaryExpression(node) && node.operator === ts.SyntaxKind.MinusToken && !ts.isNumericLiteral(node.operand)) return 'a computation (-)'
  if (ts.isTypeOfExpression(node)) return 'a type test'
  if (ts.isThrowStatement(node) || ts.isTryStatement(node)) return 'a refusal or a recovery'
  if (ts.isForStatement(node) || ts.isForInStatement(node) || ts.isWhileStatement(node) || ts.isDoStatement(node)) return 'a loop'
  if (ts.isForOfStatement(node) && !registersPoints(node)) return 'a loop'
  if ((ts.isPropertyAccessExpression(node) || ts.isPropertyAccessChain(node)) && node.questionDotToken) return 'a fallback (?.)'
  if (ts.isPropertyAccessExpression(node) && node.name.text === 'length') return 'a count (.length)'
  if (ts.isCallExpression(node) && ts.isPropertyAccessExpression(node.expression) && SHAPING_METHODS.has(node.expression.name.text) && !storeFind(node))
    return `a shaping (.${node.expression.name.text})`
  if (ts.isNewExpression(node) && /^(?:Set|Map)$/.test(node.expression.getText())) return `a table (new ${node.expression.getText()})`
  return null
}

/**
 * What decides rather than composes, read on the syntax of each file of
 * `start/` (25-01, 25-09), whether in a named function or a callback: a
 * branch, a comparison, a condition, a fallback, a computation, a count, a
 * selection or a projection of a list, a table built, a refusal or a
 * recovery, a loop that is not the registration of points. `start/`
 * instantiates, provides the ports, registers the handlers and ties an event
 * to a listener already written; each decision lives in the module that owns it.
 */
function decisionsIn(file: string, text: string): string[] {
  const source = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS)
  const found: string[] = []
  const visit = (node: ts.Node) => {
    const why = decisionOf(node)
    if (why) found.push(`line ${source.getLineAndCharacterOfPosition(node.getStart(source)).line + 1}: ${why}`)
    ts.forEachChild(node, visit)
  }
  visit(source)
  return found
}

async function tsFiles(dir: string): Promise<string[]> {
  const entries = await readdir(dir, { recursive: true })
  return entries
    .map((p) => p.split(/[\\/]/).join('/'))
    .filter((p) => p.endsWith('.ts') && !p.endsWith('.d.ts') && !p.split('/').includes('__tests__'))
    .sort()
}

async function measure(): Promise<{
  unmigrated: string[]
  violations: string[]
}> {
  const unmigrated: string[] = []
  const violations: string[] = []
  for (const file of await tsFiles(MODULES)) {
    const [module, layer] = file.split('/')
    if (!module) continue
    const text = await readFile(join(MODULES, file), 'utf8')
    for (const why of foreignTables(module, text)) violations.push(`app/modules/${file}: ${why}`)
    if (!isLayer(layer)) {
      unmigrated.push(`app/modules/${file}`)
      continue
    }
    for (const m of text.matchAll(IMPORT_RE)) {
      const specifier = m[1] ?? m[2]
      if (!specifier) continue
      const why = breach(module, layer, target(file, specifier), specifier)
      if (why) violations.push(`app/modules/${file}: ${why}`)
    }
    if ((layer === 'domain' || layer === 'application') && DB_IN_CORE.test(text)) violations.push(`app/modules/${file}: the ${layer} reaches the database`)
  }
  for (const file of await tsFiles(START)) {
    const text = await readFile(join(START, file), 'utf8')
    const lines = text.split('\n')
    const hits = lines.filter((line) => !/^\s*(?:\/\/|\*|\/\*)/.test(line) && DB_IN_START.test(line)).length
    if (hits > 0) violations.push(`start/${file}: start reaches the database (${hits})`)
    // A point written in `start/` (a commented-out block aside) instead of its module's http layer.
    if (RPC_POINT_IN_START.test(text.replace(/\/\*[\s\S]*?\*\//g, ''))) violations.push(`start/${file}: start defines an RPC point`)
    for (const why of decisionsIn(file, text)) violations.push(`start/${file}: start decides, ${why}`)
  }
  return {
    unmigrated: [...new Set(unmigrated)].sort(),
    violations: [...new Set(violations)].sort(),
  }
}

describe('the hexagonal rule (nadeo model)', () => {
  test('every file of every module is in one of the four layers', async () => {
    expect((await measure()).unmigrated).toEqual([])
  })

  test('no layer imports what it may not, no module reads the tables of another, start/ only composes', async () => {
    expect((await measure()).violations).toEqual([])
  })
})
