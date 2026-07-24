import {describe, expect, it} from 'vitest'
import {maplLanguage, naplLanguage} from '@napl/grammar'

describe('napl grammar wiring', () => {
  it('exposes the napl language with its scope and emoji alias', () => {
    expect(naplLanguage.scopeName).toBe('source.napl')
    expect(naplLanguage.aliases).toContain('🧑')
  })

  it('exposes the mapl language with its scope and emoji alias', () => {
    expect(maplLanguage.scopeName).toBe('source.mapl')
    expect(maplLanguage.aliases).toContain('🤖')
  })
})
