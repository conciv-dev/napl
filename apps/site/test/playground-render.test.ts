// @vitest-environment jsdom
import {readFileSync} from 'node:fs'
import {resolve} from 'node:path'
import {createElement} from 'react'
import {afterEach, describe, expect, it} from 'vitest'
import {cleanup, fireEvent, render} from '@testing-library/react'
import {PlaygroundShell, projectState, type PlaygroundFile} from '@napl/editor'
import type {RecordedSession} from '@napl/editor'

interface FixtureModule {
  prompt: {file: string; content: string}
  files: {path: string; content: string}[]
  session: RecordedSession
}

const fixture = JSON.parse(
  readFileSync(
    resolve(__dirname, '../src/fixtures/modules/gen_prompt_diff.json'),
    'utf8',
  ),
) as FixtureModule

afterEach(() => {
  cleanup()
})

describe('playground renders a real fixture module', () => {
  it('shows the prompt and switches to generated Rust from the replayed final state', () => {
    const final = projectState(fixture.session, fixture.session.events.length)
    const files: PlaygroundFile[] = [
      {name: fixture.prompt.file, language: 'napl', content: fixture.prompt.content},
      ...fixture.files.map((file) => ({
        name: file.path,
        language: 'source' as const,
        content: final.files[file.path] ?? file.content,
        readOnly: true,
      })),
    ]

    const {container, getByRole} = render(
      createElement(PlaygroundShell, {files, title: 'gen_prompt_diff.napl'}),
    )
    expect(container.querySelector('.cm-content')?.textContent).toContain('module: gen_prompt_diff')

    const libTab = fixture.files.find((file) => file.path.endsWith('lib.rs'))
    expect(libTab).toBeDefined()
    fireEvent.click(getByRole('tab', {name: new RegExp(libTab!.path.replace(/[/.]/g, '\\$&'))}))
    expect(container.querySelector('.cm-content')?.textContent).toContain('compute_prompt_diff')
  })
})
