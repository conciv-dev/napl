import {defineConfig, defineDocs} from 'fumadocs-mdx/config'
import {remarkMdxMermaid} from 'fumadocs-core/mdx-plugins'
import {naplLanguage, maplLanguage} from '@napl/grammar'

export const docs = defineDocs({
  dir: 'content/docs',
  docs: {
    postprocess: {
      includeProcessedMarkdown: true,
    },
  },
})

const naplLang = {...naplLanguage, name: 'napl'}
const maplLang = {...maplLanguage, name: 'mapl'}

export default defineConfig({
  mdxOptions: {
    remarkPlugins: [remarkMdxMermaid],
    rehypeCodeOptions: {
      themes: {
        light: 'github-light',
        dark: 'github-dark',
      },
      langs: [naplLang, maplLang, 'yaml', 'markdown'],
    },
  },
})
