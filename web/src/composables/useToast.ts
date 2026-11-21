import { ref } from 'vue'

export type ToastType = 'success' | 'error' | 'info' | 'warning'

export interface ToastMessage {
  id: number
  type: ToastType
  title: string
  message?: string
}

const toasts = ref<ToastMessage[]>([])
let nextId = 1

export function useToast() {
  function push(
    type: ToastType,
    title: string,
    message?: string,
    durationMs = 4500,
  ): number {
    const id = nextId++
    toasts.value.push({ id, type, title, message })
    if (durationMs > 0) {
      setTimeout(() => {
        dismiss(id)
      }, durationMs)
    }
    return id
  }

  function dismiss(id: number): void {
    toasts.value = toasts.value.filter((t) => t.id !== id)
  }

  function clearAll(): void {
    toasts.value = []
  }

  return {
    toasts,
    success: (title: string, message?: string) => push('success', title, message),
    error: (title: string, message?: string) => push('error', title, message, 6000),
    info: (title: string, message?: string) => push('info', title, message),
    warning: (title: string, message?: string) => push('warning', title, message, 5000),
    dismiss,
    clearAll,
  }
}
