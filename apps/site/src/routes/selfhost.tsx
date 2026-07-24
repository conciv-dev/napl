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
      <main className="flex w-full flex-col gap-3 px-4 py-4 lg:px-6">
        <header className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
          <h1 className="text-xl font-semibold tracking-tight">Self-host showcase</h1>
          <p className="text-sm text-fd-muted-foreground">
            36 modules of NAPL&apos;s own toolchain, each generated into Rust from a prompt — hover to
            trace attribution both ways, replay the real generation.
          </p>
          <Link
            to="/docs/$"
            params={{_splat: 'selfhost'}}
            className="text-sm font-medium text-fd-primary underline-offset-4 hover:underline"
          >
            Read the story →
          </Link>
        </header>
        <ShowcaseBrowser />
      </main>
    </HomeLayout>
  )
}
