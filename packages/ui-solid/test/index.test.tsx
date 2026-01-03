import { createRoot, createSignal } from 'solid-js'
import { isServer } from 'solid-js/web'
import { describe, expect, it } from 'vitest'
import { Hello, createHello } from '../src'

describe('environment', () => {
  it('runs on client', () => {
    expect(typeof window).toBe('object')
    expect(isServer).toBe(false)
  })
})

describe('createHello', () => {
  it('Returns a Hello World signal', () =>
    createRoot(dispose => {
      const [hello] = createHello()
      expect(hello()).toBe('Hello World!')
      dispose()
    }))

  it('Changes the hello target', () =>
    createRoot(dispose => {
      const [hello, setHello] = createHello()
      setHello('Solid')
      expect(hello()).toBe('Hello Solid!')
      dispose()
    }))
})

describe('Hello', () => {
  it('renders a hello component', () => {
    createRoot(() => {
      const container = (<Hello />) as HTMLDivElement
      expect(container.outerHTML).toBe('<div class="text-4xl text-green-700 text-center py-20">Hello World!</div>')
    })
  })

  it('changes the hello target', () =>
    createRoot(dispose => {
      const [to, setTo] = createSignal('Solid')
      const container = (<Hello to={to()} />) as HTMLDivElement
      expect(container.outerHTML).toBe('<div class="text-4xl text-green-700 text-center py-20">Hello Solid!</div>')
      setTo('Tests')

      // rendering is async
      queueMicrotask(() => {
        expect(container.outerHTML).toBe('<div class="text-4xl text-green-700 text-center py-20">Hello Tests!</div>')
        dispose()
      })
    }))
})
