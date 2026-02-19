"use client"

import * as React from "react"
import { invoke } from "@tauri-apps/api/core"

// --- TYPES ---
interface MyData {
  id: string
  name: string
  status: "active" | "inactive"
}

// --- UTILS (No dependency on clsx/tailwind-merge) ---
function cn(...classes: (string | undefined | null | false)[]) {
  return classes.filter(Boolean).join(" ")
}

// --- STYLES (Pure Object/Function Pattern instead of cva) ---
const styles = {
  container: (variant: "default" | "ghost" = "default") => cn(
    "w-full max-w-md mx-auto rounded-xl border overflow-hidden transition-all",
    variant === "default" && "bg-white dark:bg-neutral-800 border-neutral-200 dark:border-neutral-700 shadow-sm",
    variant === "ghost" && "bg-transparent border-transparent shadow-none"
  ),
  item: (status: "active" | "inactive") => cn(
    "p-3 rounded-lg border text-sm flex justify-between items-center transition-colors",
    status === "active" && "bg-emerald-50 dark:bg-emerald-900/10 border-emerald-100 dark:border-emerald-800 text-emerald-700 dark:text-emerald-300",
    status === "inactive" && "bg-neutral-50 dark:bg-neutral-900 border-neutral-100 dark:border-neutral-800 text-neutral-500"
  ),
  button: (isLoading: boolean) => cn(
    "w-full mt-4 py-2 px-4 rounded-lg font-medium text-sm transition-all focus:ring-2 focus:ring-offset-2",
    isLoading
      ? "bg-neutral-100 text-neutral-400 cursor-wait"
      : "bg-indigo-600 text-white hover:bg-indigo-700 active:scale-[0.98]"
  )
}

// --- SUB-COMPONENTS ---
const LoadingState = () => (
  <div className="flex items-center justify-center p-8 text-neutral-500 animate-pulse">
    <div className="h-4 w-4 bg-current rounded-full mr-2" />
    Loading data...
  </div>
)

const ErrorState = ({ message }: { message: string }) => (
  <div className="p-4 bg-red-50 dark:bg-red-900/10 text-red-600 dark:text-red-400 rounded-lg text-sm font-medium">
    Error: {message}
  </div>
)

const EmptyState = () => (
  <p className="text-center text-neutral-500 py-8 text-sm italic">
    No items found. Add one below.
  </p>
)

// --- MAIN COMPONENT ---
interface MyComponentProps {
  variant?: "default" | "ghost"
}

export function MyComponent({ variant = "default" }: MyComponentProps) {
  const [data, setData] = React.useState<MyData[]>([])
  const [status, setStatus] = React.useState<"idle" | "loading" | "error">("idle")
  const [errorMessage, setErrorMessage] = React.useState<string | null>(null)
  const [isMutating, setIsMutating] = React.useState(false)

  // Data Fetching (Replaces useQuery)
  const fetchData = React.useCallback(async () => {
    setStatus("loading")
    try {
      const result = await invoke<MyData[]>("get_my_data")
      setData(result)
      setStatus("idle")
    } catch (err) {
      setErrorMessage(String(err))
      setStatus("error")
    }
  }, [])

  React.useEffect(() => {
    fetchData()
  }, [fetchData])

  // Mutation (Replaces useMutation)
  const handleCreate = async () => {
    setIsMutating(true)
    try {
      await invoke("create_item", { name: "New Item" })
      await fetchData() // Refresh data
      // Simple Toast Replacement (Console or Custom Implementation)
      console.log("Item created successfully")
    } catch (err) {
      console.error(`Failed to create item: ${err}`)
    } finally {
      setIsMutating(false)
    }
  }

  // Render Logic
  if (status === "loading") return <LoadingState />
  if (status === "error" && errorMessage) return <ErrorState message={errorMessage} />

  return (
    <div className={styles.container(variant)}>
      <div className="px-6 py-4 border-b border-inherit">
        <h3 className="text-lg font-semibold tracking-tight">My Data List (No Deps)</h3>
      </div>

      <div className="p-6 space-y-3">
        {data.length === 0 ? (
          <EmptyState />
        ) : (
          data.map((item) => (
            <div key={item.id} className={styles.item(item.status)}>
              <span className="font-medium">{item.name}</span>
              <span className="text-xs uppercase tracking-wider opacity-70">{item.status}</span>
            </div>
          ))
        )}

        <button
          onClick={handleCreate}
          disabled={isMutating}
          className={styles.button(isMutating)}
        >
          {isMutating ? "Saving..." : "Add New Item"}
        </button>
      </div>
    </div>
  )
}
