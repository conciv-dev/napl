export interface BuildOptions {
  log?: (message: string) => void;
}

export async function runBuild(options: BuildOptions): Promise<void> {
  options.log?.(
    'hl build is deprecated. Generation now works directly from prompts — the coding agent writes ' +
      'source, and the IR is derived afterwards. Run "hl gen <target>" instead.',
  );
}
