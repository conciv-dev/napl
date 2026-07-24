import {Link} from '@tanstack/react-router'
import {HomeLayout} from 'fumadocs-ui/layouts/home'
import {CodeBlock} from 'fumadocs-ui/components/codeblock'
import {baseOptions} from '@/lib/layout.shared'
import {NaplPlayground} from '@/components/playground/napl-playground'

export const PROMPT_EXAMPLE = `---
module: greeting
targets: [typescript]
tests:
  - name: greets by name
    given: { name: World }
    expect: { message: "Hello, World!" }
---
# Greeting

Expose a \`greet\` function that takes a person's name and
returns a friendly greeting of the form \`Hello, <name>!\`.
Leading and trailing whitespace is trimmed. An empty name
is rejected with an error.`

const PILLARS = [
  {
    title: 'Prompts are the source',
    body: 'You write what you mean in a .napl file. The code is a build artifact — generated, tested, and locked read-only.',
  },
  {
    title: 'Attribution is a hard gate',
    body: 'Every generated line maps back to the sentence that caused it. If the toolchain cannot prove the link, the build fails.',
  },
  {
    title: 'Drift is a compile error',
    body: 'Edit locked code by hand and napl status exits non-zero. All change flows through prompts, with a guided fix attached.',
  },
]

export function LandingPage({sampleHtml}: {sampleHtml: string}) {
  return (
    <HomeLayout {...baseOptions()}>
      <main className="flex flex-col items-center px-6 py-20 gap-16">
        <section className="flex flex-col items-center text-center gap-6 max-w-3xl">
          <span className="text-sm font-mono uppercase tracking-widest text-fd-muted-foreground">
            NAPL Ain&apos;t a Programming Language
          </span>
          <h1 className="text-4xl md:text-6xl font-bold tracking-tight text-balance">
            What if the prompt <em>was</em> the source code?
          </h1>
          <p className="text-lg text-fd-muted-foreground max-w-2xl text-balance">
            Write what you mean in English. A coding agent generates and locks per-target code, attributed line by line
            and gated by tests. The English stays the source of truth.
          </p>
          <div className="flex flex-wrap items-center justify-center gap-3">
            <Link
              to="/docs/$"
              params={{_splat: ''}}
              className="rounded-md bg-fd-primary px-5 py-2.5 text-sm font-medium text-fd-primary-foreground"
            >
              Read the docs
            </Link>
            <Link
              to="/docs/$"
              params={{_splat: 'cli/install'}}
              className="rounded-md border border-fd-border px-5 py-2.5 text-sm font-medium"
            >
              Install napl
            </Link>
          </div>
        </section>

        <section className="w-full max-w-3xl text-left">
          <NaplPlayground
            module="greeting"
            compact
            speed={1.4}
            fallback={
              <CodeBlock title="greeting.napl" className="my-0 [&_pre]:px-4">
                <div dangerouslySetInnerHTML={{__html: sampleHtml}} />
              </CodeBlock>
            }
          />
          <p className="mt-3 text-center text-sm text-fd-muted-foreground">
            <code className="font-mono">napl gen typescript</code> hands this to a coding agent, then proves the
            connection. Press <strong>Run gen</strong> to replay it.
          </p>
        </section>

        <section className="grid w-full max-w-4xl gap-6 md:grid-cols-3">
          {PILLARS.map((pillar) => (
            <div key={pillar.title} className="rounded-lg border border-fd-border bg-fd-card p-5">
              <h2 className="mb-2 text-base font-semibold">{pillar.title}</h2>
              <p className="text-sm text-fd-muted-foreground">{pillar.body}</p>
            </div>
          ))}
        </section>

        <p className="text-sm text-fd-muted-foreground">
          Status: wild experiment. A bet on whether English can be a real programming language when the toolchain
          enforces the discipline compilers used to.
        </p>
      </main>
    </HomeLayout>
  )
}
