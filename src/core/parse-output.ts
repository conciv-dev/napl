export interface GeneratedFile {
  path: string;
  content: string;
}

export function extractYaml(text: string): string {
  const fence = text.match(/```(?:ya?ml)?[^\n]*\n([\s\S]*?)```/i);
  if (fence) return fence[1].trim();
  return text.trim();
}

const FILE_BLOCK_RE = /===\s*FILE:\s*(.+?)\s*===\s*\r?\n```[^\n]*\r?\n([\s\S]*?)```/g;

export function extractCodeFiles(text: string): GeneratedFile[] {
  const files: GeneratedFile[] = [];
  FILE_BLOCK_RE.lastIndex = 0;
  let match: RegExpExecArray | null = FILE_BLOCK_RE.exec(text);
  while (match !== null) {
    files.push({ path: match[1].trim(), content: match[2].replace(/\r?\n$/, '') });
    match = FILE_BLOCK_RE.exec(text);
  }
  return files;
}
