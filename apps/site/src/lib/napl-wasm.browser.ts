import init, {
  attribution_at_file_line,
  attribution_at_prompt_line,
  body_line_to_doc_line,
  doc_line_to_body_line,
  mapl_entries_at_prompt_line,
  mapl_parse,
  parse_frontmatter_diagnostics,
  scan_document_json,
} from '@napl/wasm/pkg'
import wasmUrl from '../../../../packages/napl-wasm/pkg/napl_wasm_bg.wasm?url'
import type {
  AttributionSpan,
  Diagnostic,
  MaplEntry,
  ScanResult,
} from '@napl/wasm'

let ready: Promise<void> | null = null

export function ensureNaplWasm(): Promise<void> {
  if (!ready) {
    ready = init({module_or_path: wasmUrl}).then(() => undefined)
  }
  return ready
}

export function parseFrontmatterDiagnostics(content: string): Diagnostic[] {
  return JSON.parse(parse_frontmatter_diagnostics(content)) as Diagnostic[]
}

export function scanDocument(content: string): ScanResult {
  return JSON.parse(scan_document_json(content)) as ScanResult
}

export function attributionAtPromptLine(
  attribution: string,
  promptLine: number,
): AttributionSpan[] {
  return JSON.parse(
    attribution_at_prompt_line(attribution, promptLine),
  ) as AttributionSpan[]
}

export function attributionAtFileLine(
  attribution: string,
  file: string,
  line: number,
): AttributionSpan[] {
  return JSON.parse(
    attribution_at_file_line(attribution, file, line),
  ) as AttributionSpan[]
}

export function docLineToBodyLine(content: string, docLine: number): number | null {
  return JSON.parse(doc_line_to_body_line(content, docLine)) as number | null
}

export function bodyLineToDocLine(content: string, bodyLine: number): number | null {
  return JSON.parse(body_line_to_doc_line(content, bodyLine)) as number | null
}

export function maplParse(content: string): MaplEntry[] {
  return JSON.parse(mapl_parse(content)) as MaplEntry[]
}

export function maplEntriesAtPromptLine(
  content: string,
  promptLine: number,
): MaplEntry[] {
  return JSON.parse(
    mapl_entries_at_prompt_line(content, promptLine),
  ) as MaplEntry[]
}

export type {AttributionSpan, Diagnostic, MaplEntry, ScanResult}
