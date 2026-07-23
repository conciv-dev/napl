export interface PromptBody {
  bodyStartLine: number;
  lines: string[];
}

function splitLines(text: string): string[] {
  return text.split(/\r?\n/);
}

export function promptBodyLines(raw: string): PromptBody {
  const lines = splitLines(raw);
  const hasFrontmatter = lines.length > 0 && lines[0].trimEnd() === '---';
  if (hasFrontmatter) {
    for (let i = 1; i < lines.length; i += 1) {
      if (lines[i].trimEnd() === '---') {
        const bodyStartLine = i + 1;
        return { bodyStartLine, lines: lines.slice(bodyStartLine) };
      }
    }
  }
  return { bodyStartLine: 0, lines };
}

export function bodyLineForDocLine(body: PromptBody, docLine: number): number | null {
  const bodyLine = docLine - body.bodyStartLine + 1;
  if (bodyLine < 1 || bodyLine > body.lines.length) return null;
  return bodyLine;
}

export function numberLines(lines: string[]): string {
  return lines.map((line, index) => `${index + 1}: ${line}`).join('\n');
}
