import {createRootRoute, HeadContent, Outlet, Scripts} from '@tanstack/react-router'
import appCss from '@/styles/app.css?url'
import {RootProvider} from 'fumadocs-ui/provider/tanstack'

const SITE = 'https://napl.dev'
const TITLE = "NAPL — NAPL Ain't a Programming Language"
const DESCRIPTION =
  'NAPL makes the prompt the source of truth. A coding agent generates and locks per-target code from your English, attributed line by line, gated by tests.'

export const Route = createRootRoute({
  head: () => ({
    meta: [
      {charSet: 'utf-8'},
      {name: 'viewport', content: 'width=device-width, initial-scale=1'},
      {title: TITLE},
      {name: 'description', content: DESCRIPTION},
      {name: 'theme-color', content: '#6d5efc'},

      {property: 'og:type', content: 'website'},
      {property: 'og:site_name', content: 'NAPL'},
      {property: 'og:title', content: TITLE},
      {property: 'og:description', content: DESCRIPTION},
      {property: 'og:url', content: SITE},
      {property: 'og:locale', content: 'en_US'},

      {name: 'twitter:card', content: 'summary_large_image'},
      {name: 'twitter:title', content: TITLE},
      {name: 'twitter:description', content: DESCRIPTION},
    ],
    links: [
      {rel: 'stylesheet', href: appCss},
      {rel: 'canonical', href: `${SITE}/`},
      {rel: 'icon', href: '/favicon.svg', type: 'image/svg+xml'},
      {rel: 'apple-touch-icon', href: '/favicon.svg'},
    ],
  }),
  component: RootComponent,
})

function RootComponent() {
  return (
    <html lang="en" suppressHydrationWarning>
      <head>
        <HeadContent />
      </head>
      <body className="flex flex-col min-h-screen">
        <RootProvider>
          <Outlet />
        </RootProvider>
        <Scripts />
      </body>
    </html>
  )
}
