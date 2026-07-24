import {useEffect, useState, type ReactElement} from 'react'
import {loadShowcaseModule} from '@/lib/fixtures'
import type {ShowcaseModule} from '@/lib/fixtures'
import {NaplPlaygroundClient} from '@/components/playground/napl-playground-impl'

export interface InlineFile {
  name: string
  content: string
}

export interface NaplExampleImplProps {
  module?: string
  files?: InlineFile[]
  compact?: boolean
  readOnly?: boolean
  showGen?: boolean
}

const synthModule = (files: InlineFile[]): ShowcaseModule => {
  const prompt = files.find((f) => f.name.endsWith('.napl') || f.name.endsWith('.🧑')) ?? files[0]
  const promptName = prompt?.name ?? 'example.napl'
  const promptContent = prompt?.content ?? ''
  const generated = files.filter((f) => f !== prompt)
  return {
    module: promptName.replace(/\.(napl|🧑)$/, ''),
    collection: 'example',
    target: '',
    targets: [],
    prompt: {file: promptName, path: promptName, content: promptContent},
    promptAtGen: null,
    summary: '',
    files: generated.map((f) => ({
      path: f.name,
      journalPath: f.name,
      content: f.content,
      language: 'text',
    })),
    attribution: [],
    attributionYaml: '',
    mapl: [],
    journal: [],
    lock: [],
    session: {task: '', files: {[promptName]: promptContent}, events: []},
  }
}

export function NaplExampleClient({
  module: moduleName,
  files,
  compact = true,
  readOnly = false,
  showGen = false,
}: NaplExampleImplProps): ReactElement {
  const [data, setData] = useState<ShowcaseModule | null>(files ? synthModule(files) : null)

  useEffect(() => {
    if (!moduleName) return undefined
    let active = true
    loadShowcaseModule(moduleName)
      .then((loaded) => {
        if (active) setData(loaded)
      })
      .catch(() => {})
    return () => {
      active = false
    }
  }, [moduleName])

  if (!data) {
    return <div className="napl-example__placeholder">Loading example…</div>
  }

  return (
    <div className="napl-example" data-testid="napl-example">
      <NaplPlaygroundClient
        module={data}
        compact={compact}
        readOnly={readOnly}
        showGen={showGen}
      />
    </div>
  )
}
