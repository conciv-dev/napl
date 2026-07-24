import type {HighlightRange} from '@napl/editor'
import {
  attributionAtFileLine,
  attributionAtPromptLine,
  bodyLineToDocLine,
  docLineToBodyLine,
} from './napl-wasm.browser'

export interface ResolvedSpan {
  file: string
  lines: [number, number]
  promptLines: [number, number]
  promptDocLines: [number, number]
  note: string
  sentence: string
}

const bodyRangeToDocRange = (
  promptContent: string,
  start: number,
  end: number,
): [number, number] => {
  const docStart = bodyLineToDocLine(promptContent, start)
  const docEnd = bodyLineToDocLine(promptContent, end)
  if (docStart == null || docEnd == null) return [start, end]
  return [docStart + 1, docEnd + 1]
}

const extractSentence = (promptAtGen: string | null, start: number, end: number): string => {
  if (!promptAtGen) return ''
  return promptAtGen
    .split('\n')
    .slice(start - 1, end)
    .join(' ')
    .replace(/^[#\s>-]+/, '')
    .replace(/\s+/g, ' ')
    .trim()
}

const build = (
  promptContent: string,
  promptAtGen: string | null,
  span: {
    file: string
    lines: {start: number; end: number}
    prompt_lines: {start: number; end: number}
    note: string
  },
): ResolvedSpan => ({
  file: span.file,
  lines: [span.lines.start, span.lines.end],
  promptLines: [span.prompt_lines.start, span.prompt_lines.end],
  promptDocLines: bodyRangeToDocRange(promptContent, span.prompt_lines.start, span.prompt_lines.end),
  note: span.note,
  sentence: extractSentence(promptAtGen, span.prompt_lines.start, span.prompt_lines.end),
})

export const resolvePromptHover = (
  promptContent: string,
  attributionYaml: string,
  promptAtGen: string | null,
  docLine0: number,
): ResolvedSpan[] => {
  const bodyLine = docLineToBodyLine(promptContent, docLine0)
  if (bodyLine == null) return []
  return attributionAtPromptLine(attributionYaml, bodyLine).map((span) =>
    build(promptContent, promptAtGen, span),
  )
}

export const resolveGeneratedHover = (
  promptContent: string,
  attributionYaml: string,
  promptAtGen: string | null,
  file: string,
  genLine0: number,
): ResolvedSpan[] => {
  return attributionAtFileLine(attributionYaml, file, genLine0 + 1).map((span) =>
    build(promptContent, promptAtGen, span),
  )
}

export const promptHighlightsFor = (spans: ResolvedSpan[]): HighlightRange[] =>
  spans.map((span) => span.promptDocLines)

export const fileHighlightsFor = (spans: ResolvedSpan[], file: string): HighlightRange[] =>
  spans.filter((span) => span.file === file).map((span) => span.lines)

export const excerptFromContent = (content: string, range: [number, number]): string => {
  const lines = content.split('\n')
  const start = Math.max(0, range[0] - 1)
  const end = Math.min(lines.length, range[1])
  return lines.slice(start, end).join('\n').replace(/\t/g, '  ')
}
