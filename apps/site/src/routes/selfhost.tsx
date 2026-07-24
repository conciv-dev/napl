import {createFileRoute, Link} from '@tanstack/react-router'
import {HomeLayout} from 'fumadocs-ui/layouts/home'
import {baseOptions} from '@/lib/layout.shared'
import {ShowcaseBrowser} from '@/components/showcase/showcase-browser'

export const Route = createFileRoute('/selfhost')({
  component: Selfhost,
})

function Selfhost() {
  return (
    <HomeLayout {...baseOptions()}>
      <main className="flex flex-col px-6 py-12 gap-8 max-w-6xl mx-auto w-full">
        <header className="flex flex-col gap-3">
          <span className="text-sm font-mono uppercase tracking-widest text-fd-muted-foreground">
            Self-host showcase
          </span>
          <h1 className="text-3xl md:text-4xl font-bold tracking-tight">
            NAPL generating its own toolchain
          </h1>
          <p className="text-fd-muted-foreground max-w-3xl">
            Every module below is a real self-hosted NAPL prompt that generated a Rust crate. The
            prompt is on the left, the generated code on the right. Hover a prompt sentence to see the
            lines it produced, hover generated code to jump back to its prompt, and replay the exact
            generation the toolchain recorded. All data is derived from the repository&apos;s journal,
            attribution, and machine-layer state.{' '}
            <Link to="/docs/$" params={{_splat: 'selfhost'}} className="underline">
              Read the self-host story
            </Link>
            .
          </p>
        </header>
        <ShowcaseBrowser />
      </main>
    </HomeLayout>
  )
}
