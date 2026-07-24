import type {ComponentProps} from 'react'
import {cva, type VariantProps} from 'class-variance-authority'
import {Slot} from 'radix-ui'
import {cn} from '@/lib/utils'

const buttonVariants = cva(
  "inline-flex shrink-0 items-center justify-center gap-1.5 rounded-lg border border-transparent text-sm font-medium whitespace-nowrap transition-[color,background-color,border-color,box-shadow,transform] outline-none select-none focus-visible:ring-2 focus-visible:ring-fd-ring/60 active:scale-[0.97] disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
  {
    variants: {
      variant: {
        default: 'bg-fd-primary text-fd-primary-foreground hover:brightness-110',
        outline:
          'border-fd-border bg-fd-card text-fd-foreground hover:border-fd-primary/50 hover:bg-fd-accent/40',
        secondary: 'bg-fd-secondary text-fd-secondary-foreground hover:bg-fd-accent',
        ghost: 'text-fd-muted-foreground hover:bg-fd-accent hover:text-fd-accent-foreground',
      },
      size: {
        default: 'h-8 px-3',
        sm: 'h-7 px-2.5 text-[0.8rem]',
        xs: 'h-6 gap-1 px-2 text-xs [&_svg:not([class*=\'size-\'])]:size-3',
        icon: 'size-8',
      },
    },
    defaultVariants: {
      variant: 'default',
      size: 'default',
    },
  },
)

export type ButtonProps = ComponentProps<'button'> &
  VariantProps<typeof buttonVariants> & {asChild?: boolean}

export function Button({
  className,
  variant,
  size,
  asChild = false,
  ...props
}: ButtonProps) {
  const Comp = asChild ? Slot.Root : 'button'
  return (
    <Comp className={cn(buttonVariants({variant, size, className}))} {...props} />
  )
}

export {buttonVariants}
